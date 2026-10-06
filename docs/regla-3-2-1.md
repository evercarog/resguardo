# La regla 3-2-1-1-0 como guía (tarea 8)

Diseño de la tarea 8 de [plan-mejoras.md](plan-mejoras.md). Escrito el 2026-10-06 con el responsable ausente: como pidió para las tareas grandes, la propuesta va aquí y el trabajo sigue sin esperar el visto bueno. Lo dudoso está en [registro-ia.md](registro-ia.md).

Documentos relacionados: [copias-en-cadena.md](copias-en-cadena.md) (destinos, zonas y cadenas; la parte B, con las cadenas y las copias derivadas, la hace otra sesión a la vez), [espejo.md](espejo.md), [almacen-inmutable.md](almacen-inmutable.md) (la guía de 8e), [api-servidor.md](api-servidor.md) («Cambios»).

## Qué es la regla

La forma moderna de la regla 3-2-1, **3-2-1-1-0**, para **cada copia** (las carpetas de un equipo que se copian juntas):

| Parte | Qué pide | Cómo se cuenta |
|---|---|---|
| **3** | Tres copias de los datos, contando los originales. | Los originales (1) más cada destino **al día** al que llegan esos datos. |
| **2** | En dos soportes distintos. | Soportes distintos entre los originales y los destinos al día. Soporte = **equipo + disco** (o la nube, o el servidor de fuera). |
| **1** | Una fuera de la oficina. | Destinos al día en **otra sede** o en la **nube**. |
| **1** | Una inmutable o fuera del alcance de los equipos. | Destinos al día de **solo añadir** (desde el equipo), con **bloqueo de objetos**, con **instantáneas fuera de su alcance** o **desconectados**. |
| **0** | Cero errores al verificar y al probar la restauración. | Verificación y prueba de restauración programadas, la última de cada una correcta y reciente (45 días como mucho), y ningún destino con datos dañados en su comprobación. |

**Guía, nunca obligación.** Se puede guardar cualquier configuración. La consola dice cómo queda cada copia y qué hacer para cumplir; nunca bloquea un botón ni resta en otra parte por no cumplirla.

## 8a. Lo que se sabe de cada destino

Cada destino lleva tres datos para la regla:

- **Dónde está** (`lugar`): `este_equipo`, `oficina` (otro equipo de la oficina), `otra_sede`, `nube`.
- **Si es inmutable** (`inmutable`): `solo_anadir` (un rest-server de solo añadir: desde el equipo no se puede borrar), `object_lock` (bloqueo de objetos), `instantaneas` («con instantáneas inmutables fuera de su alcance»: las hace el anfitrión, el almacén no las ve), `desconectado` (un disco USB que se desconecta y se rota) o `no`.
- **Soporte** (`soporte`): qué equipo y qué disco. Dos destinos con el mismo soporte cuentan una vez en el «2».

### Valores por defecto (deducidos del tipo)

| Destino | Dónde | Inmutable | Soporte |
|---|---|---|---|
| Zona de un almacén del cliente (otro equipo) | `oficina` | `solo_anadir` (las zonas siempre lo son) | el almacén + el disco de la zona («D:»; sin letra, la zona) |
| …el almacén del propio equipo | `este_equipo` | `solo_anadir` | igual |
| Servidor de copias de fuera (rest) | `otra_sede` | `solo_anadir` si se comprobó (`solo_anadir` del repositorio o `inmutable` del destino); si no, `no` | el servidor |
| B2, S3 | `nube` | `object_lock` si se marcó (bloqueo de días de la copia externa o `inmutable`); si no, `no` | la nube + el bucket |
| SFTP | `otra_sede` | `no` | el servidor |
| Carpeta del propio equipo | `este_equipo` | `no` | **el mismo soporte que los originales** (no se sabe en qué disco están: lo prudente es no contarla aparte) |
| Disco extraíble del propio equipo | `este_equipo` | `no` (márcalo «desconectado» si lo rotas) | el equipo + la unidad |
| Carpeta de otra máquina de la red | `oficina` | `no` | esa carpeta |
| Espejo del almacén en una carpeta | como el almacén | `no` | el almacén + la unidad de la carpeta |
| Espejo en Dropbox, Drive, WebDAV | `nube` | `no` (no son inmutables) | esa nube |
| Espejo en B2, S3 | `nube` | `object_lock` si el destino tiene bloqueo | esa nube |
| Espejo en SMB | `oficina` | `no` | esa nube (el NAS) |
| Espejo en SFTP | `otra_sede` | `no` | esa nube |

### Editables, en el catálogo de destinos (parte A)

Los tres datos se pueden cambiar en «Repositorios y destinos» → el destino → «Para la regla 3-2-1». Se guardan en el **catálogo del cliente** (`atributos` de la entrada, junto al nombre; en claro: no son secretos). Lo que no se cambia sigue deduciéndose del tipo. Ejemplos: «el servidor de copias de fuera está en la oficina» (`lugar: oficina`), «este almacén tiene instantáneas en el anfitrión» (`inmutable: instantaneas`), «estos dos discos USB son el mismo soporte que se rota» (`soporte: "USB rotado"`).

Las claves del catálogo son las de la parte A: `zona:<almacén>:<zona>`, el id de un destino de los equipos, `nube:<almacén>:<nombre>`. Las **carpetas del espejo** no estaban en el catálogo: su clave es `espejo:<almacén>:<8 cifras hexadecimales>` (un resumen de su ruta: la ruta no va al servidor).

**El almacén no puede comprobar** lo que hace su anfitrión (instantáneas de Proxmox) ni si un disco se desconecta: lo dice la persona y la consola lo cree. Lo que sí se comprueba (solo añadir, en el agente) manda sobre lo marcado: si un rest-server de fuera **no** es de solo añadir, marcarlo no lo arregla, pero la regla lo cuenta como lo marques (es una guía) y la «Salud de la protección» sigue diciendo lo que comprobó el agente.

**Sistema de archivos y entorno (8e).** El agente dice en su resumen el **sistema operativo** (ya lo decía), el **sistema de archivos** de la carpeta de cada destino local, de cada zona y de cada carpeta del espejo (`NTFS`, `ReFS`, `ext4`, `xfs`, `zfs`, `btrfs`…) y si corre en un **contenedor** o una **máquina virtual**. La consola lo enseña **como dato** en la ficha del destino. **Nunca** resta en la regla ni en la salud de la protección por usar Windows o un sistema de archivos sin instantáneas.

## 8b. El cálculo, por copia

### Los caminos de hoy

Para una copia (carpetas de un equipo → un repositorio), los datos llegan a:

1. **El destino del repositorio** (paso `copia`): una zona de un almacén, un servidor de fuera, una nube, una carpeta o un disco del equipo. Al día si la copia salió bien dentro de su horario más un margen.
2. **Los destinos del espejo del almacén** (paso `espejo`), si el repositorio está en la **zona principal** de un almacén (el espejo de hoy copia la principal) y el destino del espejo lo incluye (`repos` sin selección o con ese `<usuario>/<repo>`).
3. **La copia externa** del repositorio (paso `externa`).
4. **(Parte B)** Los pasos de una cadena: espejos por repositorio (también desde otras zonas), copias derivadas (`derivadas[]`) y copias nuevas «después de la anterior». Entran como **más pasos** en la misma lista, con los mismos campos: el cálculo no cambia.

### Modelo de entrada (pequeño, a propósito)

Las reglas están **en un solo sitio**: `crates/agente/src/protection.rs` (`regla_321`, y `evaluate` la calcula cuando los `Facts` traen la entrada). La consola, que es la que ve todos los equipos (el almacén, su espejo, el catálogo), arma la entrada y la evalúa con la misma regla en TypeScript (`consola/src/lib/regla321.ts`). Las dos pasan **los mismos vectores** (`crates/protocolo/vectors/regla-321.json`): si una cambia, la otra falla.

```text
EntradaRegla {
  origen: { equipo, soporte },              // los originales: cuentan como una copia
  pasos: [PasoRegla],                        // cada destino al que llegan los datos
  verificacion: PruebaRegla,                 // la del repositorio de la copia
  prueba_restauracion: PruebaRegla,
}
PasoRegla {
  id, nombre,
  tipo: "copia" | "espejo" | "externa" | "derivada" | "paso",
  lugar, inmutable, soporte,
  equipo?: string,                           // el equipo que lo guarda (nada: nube o servidor de fuera)
  ultima_ok?: RFC 3339,                      // la última vez que se puso al día bien
  cada_horas?: número,                       // cada cuánto le toca (sin él, 24)
  verificacion_mal?: bool,                   // su comprobación encontró datos dañados
}
PruebaRegla { configurada, ultima_ok?, fallo }
```

**Al día**: `ultima_ok` existe y `ahora − ultima_ok ≤ cada_horas × 1,5 + 12 h` (cada día: 48 h; cada hora: 13,5 h; cada semana: 11 días). Un paso que lleva tiempo fallando deja de estar al día cuando pasa ese plazo, no al primer fallo.

### Salida

```text
Regla321 {
  cumple,              // las cinco partes, con lo que está al día
  cumple_config,       // las cinco partes si todos los pasos estuvieran al día y lo programado hubiera salido bien
  dejo_de_cumplir,     // cumple_config && !cumple: la configuración está bien, algo no está al día
  partes: [{ id: "copias" | "soportes" | "fuera" | "inmutable" | "errores",
             meta, valor, valor_config, cumple, cumple_config, accion, detalle }],
  atrasados: [id del paso],
  avisos: ["mismo_equipo" | "inmutable_local"],
}
```

- `accion` es un código (`anadir_destino`, `poner_al_dia`, `otro_soporte`, `anadir_fuera`, `anadir_inmutable`, `programar_verificacion`, `programar_prueba`, `revisar_verificacion`, `revisar_prueba`, `revisar_destino`, o vacío si cumple): cada lado lo pone en palabras (la consola, con los nombres de los destinos y un enlace).
- `mismo_equipo`: se llega a 2 soportes solo con discos del mismo equipo (dos zonas del mismo almacén): «un fallo del equipo, un robo o un incendio se los lleva a la vez».
- `inmutable_local`: lo inmutable está todo en la oficina (instantáneas del anfitrión, un disco desconectado, una zona de solo añadir): no protege de un incendio o un robo de la oficina.

**«Dejó de cumplir» sin guardar historia.** Una copia «deja de cumplir» cuando su **configuración** cumple pero hoy algo no está al día o falló (p. ej. el espejo en la nube lleva 3 días fallando). Es un cálculo, no un recuerdo: no hace falta guardar el estado anterior en ningún sitio.

### Lo que no cuenta (a propósito)

- Que el sistema sea Windows o el sistema de archivos no tenga instantáneas (8e).
- La retención: un espejo **con retención** sigue a la del original (con retraso y freno). Cuenta como un destino más; la recomendación de copias-en-cadena.md («al menos un destino fuera del alcance de la retención») la da la parte B al crear los pasos.
- Los repositorios de solo lectura (importados) y las copias desactivadas: no se evalúan.

## 8c. En la consola

- **En cada copia** (su página): una tira **«3 · 2 · 1 · 1 · 0»** con cada parte cumplida o no, su cifra («2 de 3») y **qué hacer** («Añade un destino fuera de la oficina: Dropbox o B2 → Copia externa»), con enlace al sitio donde se hace hoy (copia externa, espejo del almacén, verificación, prueba de restauración, ficha del destino). Debajo, los destinos que cuentan, con su lugar, si es inmutable y si está al día.
- **Al añadir o cambiar una copia** («Cambiar las copias»): un resumen por copia («Cumple la regla 3-2-1-1-0» o «Le falta: 1 fuera de la oficina…») con el paso que propone. Se calcula con el repositorio elegido (lo que ya tiene ese repositorio: espejo, copia externa, verificación).
- **En Estado** (página del cliente): «N de M copias cumplen la regla 3-2-1-1-0», con la lista de las que no y lo que les falta. Las que **dejaron de cumplir** salen como **aviso no urgente** (ámbar suave, no en «Necesita atención»).
- **En Informes**: la misma cuenta por cliente, para enseñársela al cliente, con una fila por copia.
- **El aviso por correo / canales** (notificaciones del servidor) **no** se hace en esta tarea: el servidor tendría que tener la regla y el catálogo junto a los avisos de cada equipo (`notificaciones/problemas.rs` mira un equipo cada vez). Queda anotado en el plan.

## 8d. Plantilla «3-2-1 recomendada»

En «Cambiar las copias», junto a «Añadir una copia»: **«Añadir con la plantilla 3-2-1 recomendada»**. Hace lo que se puede hoy y deja escrito lo que falta:

1. **La copia** al almacén (la zona principal, «Disco D»), con un repositorio que ya esté en un almacén del cliente si lo hay.
2. **Verificación automática** semanal (10 %) en ese repositorio, y la propuesta de **prueba de restauración mensual** (enlace a la página del repositorio, donde se programa).
3. **Espejo a otro disco** (zona E) **después de la anterior**: hoy, con el espejo flexible del almacén (un destino de carpeta en otro disco con «después de cada copia nueva»); el paso por copia es de la parte B.
4. **Repositorio a partir del anterior en la nube** (B2 con bloqueo de objetos, o Dropbox) **después de la anterior**: hoy, la **copia externa** del repositorio (a B2 con bloqueo de objetos); la copia derivada «después de la anterior» es de la parte B.

Los pasos 3 y 4 se enseñan como una lista con su estado («Ya lo tiene», «Hazlo en…», «Pendiente: copias en cadena») y no se mandan solos (cada uno es una orden con su clave y su espera, donde ya está hoy). Cuando la parte B esté en `main`, la plantilla creará la cadena entera.

## 8e. Inmutabilidad local y el almacén recomendado

- **Guía**: [almacen-inmutable.md](almacen-inmutable.md) (Proxmox con ZFS e instantáneas del anfitrión, Linux endurecido, discos USB rotados, almacén en Windows).
- **En la consola**: «Con instantáneas inmutables fuera de su alcance» y «Desconectado» se marcan en el destino (8a). La regla los cuenta como inmutables y avisa de que son locales (`inmutable_local`).
- **Detectar y enseñar, sin castigar**: el sistema de archivos de cada carpeta y si el almacén es un contenedor o una máquina virtual, como dato en la ficha del destino (y del equipo). Un enlace discreto «Cómo añadir instantáneas que el almacén no pueda borrar», que se puede ocultar (se recuerda en el navegador).

## Contrato (compatible hacia atrás)

En [api-servidor.md](api-servidor.md) → «Cambios», como «v1.4x (pendiente de numerar al unir)»:

- Catálogo de destinos: `atributos?: { lugar?, inmutable?, soporte? }` en `GET` y `PUT`. Un `PUT` **sin** `atributos` deja los que había (una consola anterior que solo renombra no los borra); con `atributos: null` o `{}` se quitan. El nombre puede ir vacío solo si van `atributos` (marcar un destino sin ponerle nombre propio): el destino sigue con su nombre de siempre.
- Resumen del agente: `sistema_archivos?` en `destinos[]` (locales), en `guarda_copias` (la principal), en `guarda_copias.zonas[]` y en `guarda_copias.espejo.destinos[]` (carpetas); y `entorno?: { virtual?: "kvm" | "vmware" | "hyperv" | "virtualbox" | "xen" | "otra", contenedor?: "lxc" | "docker" | "podman" | "wsl" | "otro" }` en la raíz del resumen. Solo nombres: nada de rutas. Una consola anterior los ignora.
- Ninguna orden nueva y ningún cambio en los agentes instalados: la regla la calcula la consola con lo que ya mandan; los datos nuevos solo se enseñan.

## Seguridad

- El catálogo sigue sin admitir secretos: `atributos` rechaza cualquier campo de más y valores fuera de la lista.
- La clave de las carpetas del espejo es un resumen (no la ruta).
- Lo que dice la persona («instantáneas», «desconectado») solo cambia la guía: no cambia ninguna orden, ninguna espera ni lo que comprueba el agente.
- Detectar contenedor o máquina virtual lee archivos del sistema (`/proc`, `/sys`, el registro de Windows) sin ejecutar programas.
