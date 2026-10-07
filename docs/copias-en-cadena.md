# Destinos, zonas del almacén y copias en cadena (tareas 7 y 4 del plan)

Diseño de la tarea 7 de [plan-mejoras.md](plan-mejoras.md) («Destinos independientes, zonas del almacén y copias en cadena»), con la tarea 4 («Copias derivadas») dentro. Escrito el 2026-10-06 con el responsable ausente: como pidió para las tareas grandes, la propuesta va aquí y el trabajo sigue sin esperar el visto bueno. Lo dudoso está anotado en [registro-ia.md](registro-ia.md).

Se hace en dos partes:

- **Parte A** (rama `ia/destinos-y-zonas`, hecha): 7a (destinos de primera clase en la consola) y 7b = 4f (varias zonas en un almacén).
- **Parte B** (pendiente, para la siguiente sesión): 7c (copias ordenadas y en cadena), 7d (los tres tipos de paso), 7f (el flujo «destino primero, repositorio después»), 7e (convivencia con el espejo de hoy) y 4a–4d. Lo que falta, con detalle, está en [«Lo que queda para la parte B»](#lo-que-queda-para-la-parte-b).

Documentos relacionados: [espejo.md](espejo.md) (el motor del espejo, que reutilizan los pasos «espejo»), [compartir.md](compartir.md) (el Servidor de copias, el solo añadir y la retención en el almacén), [destinos.md](destinos.md) (el modelo destino/repositorio de la app de escritorio, congelada), [api-servidor.md](api-servidor.md) (contrato; «Cambios»).

## Vocabulario

| Palabra | Qué es | Ejemplo |
|---|---|---|
| **Destino** | Un lugar donde se guardan repositorios. | «Almacén · Disco D», «Backblaze B2 · copias-sur», «Dropbox Oficina». |
| **Zona** | Una carpeta que sirve un almacén, con su propio rest-server y su puerto. Cada zona es un destino. | `D:\Resguardo` en el puerto 8000 (la principal), `E:\Resguardo` en el 8002. |
| **Repositorio** | Una caja cifrada de restic en un destino, con su contraseña. | `RECEPCION/documentos` en «Almacén · Disco D». |
| **Copia** | Un trabajo: qué se copia, a qué repositorio y cuándo. | «Documentos cada hora». |
| **Paso** | Una copia dentro de una cadena: empieza con horario o «después de la anterior». | «Después → Dropbox (espejo)». |
| **Cadena** | Las copias de un mismo origen, en orden. | Documentos → Disco D → Disco E → Dropbox. |

## 7a. Destinos de primera clase

### Qué cambia para la persona

En «Repositorios y destinos» hay una lista de **destinos** que no depende de que haya repositorios: se puede **crear uno sin crear un repositorio**, ponerle **nombre** y cambiarlo cuando quieras. Al crear un repositorio se elige un destino de la lista (o se crea uno ahí mismo, como hasta ahora).

### Dónde vive cada cosa (seguridad primero)

Un destino tiene dos partes que se guardan en sitios distintos:

1. **Lo que lo describe** (nombre, tipo, servidor o bucket): en el **catálogo del cliente**, en el servidor, en claro. Es lo mismo que ya ve hoy el servidor en los resúmenes de los equipos (`destinos[].donde` de un destino que no es local), más un nombre.
2. **Las credenciales** (clave del bucket, usuario y contraseña del rest-server, token de Dropbox): **solo en los equipos que lo usan**, selladas para cada uno en la orden que lo necesita (`crear_repositorio`, `cambiar_copia_externa`, `conectar_nube`, `guarda_copias { anadir }`), como hasta ahora. **Nunca** en el catálogo, ni cifradas: así una clave de administración adivinada (o el servidor) no da acceso directo a la nube; hoy ni con la clave se pueden sacar las credenciales de un equipo.

Consecuencia: usar en un equipo un destino que ese equipo aún no tiene pide sus credenciales en ese momento (los datos que no son secretos ya vienen puestos). En un almacén no hace falta escribir nada: el almacén da el acceso sellado para el navegador, como «Copiar en …».

El catálogo **no** guarda rutas de carpetas locales (los agentes no las mandan nunca en su resumen: v1.41) ni el id de la clave de un bucket.

### De dónde salen los destinos que se ven

La consola junta tres fuentes (`consola/src/lib/destinos.ts`, con vectores):

| Fuente | Qué da | Clave del destino |
|---|---|---|
| Las **zonas** de cada almacén del cliente (resumen `guarda_copias`, `guarda_copias.zonas`) | «Almacén ALMACEN-01 · Disco D» (la principal) y una por zona | `zona:<equipo>:principal`, `zona:<equipo>:<zona>` |
| Los **destinos de los equipos** (resumen `destinos[]`, agrupados por su id) | Los que ya usan los repositorios: rest, s3, b2, sftp, carpetas | su id (`destino-1a2b3c4d`, `almacen-…`) |
| El **catálogo** (`GET /api/clientes/{c}/destinos`) | Los creados sin repositorio y los **nombres** puestos a cualquiera de los anteriores | la misma clave |

- Un destino de un equipo que es una zona de un almacén (`equipo_almacen` y el puerto de su dirección) se enseña **dentro de esa zona**, no aparte.
- **Nombre:** el del catálogo si lo hay; si no, el de siempre (el que da el equipo o «Almacén X · Disco D» para una zona, con la unidad de su carpeta).
- **Renombrar** solo toca el catálogo: no manda órdenes ni pide la clave de administración (no cambia nada en los equipos), sí el rol de administrador; queda en la auditoría. Quitar un nombre vuelve al de siempre. Las copias, los repositorios y los kits no cambian.
- **Quitar** un destino del catálogo solo lo quita de la lista (si tiene repositorios o es una zona, sigue apareciendo con su nombre de siempre). Nunca toca ningún equipo ni lo que hay en el destino.

### El catálogo en el servidor

`GET /api/clientes/{c}/destinos` (cualquier miembro): `[{ id, nombre, tipo, donde?, actualizado, por }]`.
`PUT /api/clientes/{c}/destinos/{id}` (administrador): `{ nombre, tipo, donde? }`, crea o sustituye.
`DELETE /api/clientes/{c}/destinos/{id}` (administrador).

- `id`: `[a-z0-9:_.-]`, hasta 120. `nombre`: 1–80 caracteres, sin controles. `tipo`: `zona`, `rest`, `s3`, `b2`, `sftp`, `nube`, `local`. `donde`: hasta 300, solo en `rest`, `s3`, `b2`, `sftp`; se rechaza con credenciales (`usuario:clave@`) o con forma de ruta local (`C:\…`, `\\nas\…`, `/srv/…`).
- Se rechaza cualquier campo de más (`secreto`, `contrasena`, `clave`…): el catálogo no admite secretos ni por error.
- Como mucho 200 por cliente. Cada cambio va a la auditoría (`guardar_destino`, `borrar_destino`) con el id.
- Se guarda en la base de datos del cliente (tabla `destinos` de `clientes/<id>.db`), así que entra en la **copia de la consola**. El paquete de exportación `.resguardo-cliente` (mover un cliente a otro servidor) **aún no** lo lleva: queda para la parte B (punto 8); sin él, en el servidor nuevo los destinos vuelven a sus nombres de siempre y los sueltos hay que crearlos otra vez.

### La página de un destino

Cada destino tiene su página, como los repositorios: `/c/<cliente>/destinos/<clave>` (la clave del catálogo, codificada: `zona:<almacén>:<zona>`, `nube:<almacén>:<nombre>` o el id del destino). Vale para las zonas de los almacenes, las carpetas y discos de los equipos, los servidores y nubes de los repositorios, las nubes conectadas en un almacén y los destinos sueltos del catálogo (`lib/fichaDestino.ts`, con vectores en `scripts/vectores-destinos.ts`).

- **Cabecera**: el nombre (el del catálogo si lo tiene; «antes «…»» si se renombró), qué es y dónde está, y las acciones: «Usar en una copia», «Nombre», «Regla 3-2-1» y «Notas».
- **Cifras**: repositorios, lo protegido, el espacio libre (el de la zona, o el que midió el espejo en esa nube; «el proveedor no dice un límite» en B2/S3) y lo último que pasó.
- **Dónde está**: el almacén y su puerto, los equipos que lo tienen, el sistema de archivos y el entorno (solo un dato) y los chips de siempre (solo añadir, solo red local, en el mismo equipo, extraíble, no inmutable).
- **Para la regla 3-2-1-1-0**: dónde está, si es inmutable y el soporte, diciendo si es lo marcado o lo deducido del tipo.
- **Repositorios aquí**, **Espejos y copias que lo usan** (copias externas y derivadas que llegan, el espejo de un almacén que llega o, en una zona, lo que sale de ella hacia otra zona o una nube, con enlace a la página de ese otro destino), **Usar en una copia** y **Lo último que pasó** (la última vez de cada copia que guarda aquí y del espejo que llega o sale).
- **Usar en una copia** (`usarEnCopia`, que decide con `usosPosibles` de `lib/cadenas.ts`, el mismo que «Añadir paso» del editor de copias): copias nuevas (a una zona, desde cualquier equipo; a otro destino, desde quien lo tiene), el **espejo** de un repositorio que está en el almacén de este destino (lo hace el almacén, sin contraseñas) y un **repositorio nuevo a partir de** otro en su equipo (también cuando antes hay que conectar la nube en él: lo dice y el diálogo lo ofrece); en uno suelto, «Nuevo repositorio». Los enlaces llevan `destino=<clave>`: el paso «espejo» (`?paso_espejo=`) y la copia derivada (`?derivada=`) abren con ese destino ya elegido, y una copia nueva del editor (`?nueva=1`) toma un repositorio del equipo en ese destino si lo tiene.
- **Quitar este destino** (si no lo usa nada), con la clave de administración.

Llevan a ella: el nombre de cada tarjeta de «Repositorios y destinos» (y su «Usar en una copia»), las tarjetas de destino, copia externa y espejo (nube o zona) del mapa de la protección, «Se guarda en» de la página de la copia y del repositorio, el «Camino» de la copia y la lista «Dónde llegan los datos» de su tira 3·2·1·1·0. Una clave que ya no está (una zona quitada, una nube desconectada) enseña «Este destino ya no está».

### Crear un destino sin repositorio

«Nuevo destino» en «Repositorios y destinos» (administrador):

- **Otra zona de un almacén** (otro disco): ver 7b. Es una orden al almacén con la clave de administración.
- **Nube o servidor para los repositorios** (Backblaze B2, S3, un rest-server de otra sede): nombre y dirección (bucket, servidor). SFTP se admite en el catálogo, pero la consola aún no lo ofrece aquí: un repositorio SFTP usa la llave SSH del equipo, que la consola no sabe preparar todavía. Solo va al catálogo; las credenciales se piden al crear el primer repositorio en él, selladas para ese equipo.
- **Dropbox, Google Drive, NAS (SMB), WebDAV** (y B2/S3/SFTP para el espejo): se conectan **en un almacén** con «Conectar Dropbox» / «Conectar otro destino», que ya existen (`conectar_nube`, sellada para el almacén). Desde la parte A sirven para el espejo; en la parte B, también para los pasos de una cadena (4a: conectarlas en el equipo dueño).

### Elegir un destino al crear un repositorio

«Nuevo repositorio» lista, para el equipo elegido:

1. las zonas de los almacenes del cliente a las que aún no copia («Almacén ALMACEN-01 · Disco E»): hace lo de «Copiar en …» en esa zona (`guarda_copias { anadir, zona }`), sin escribir nada;
2. sus propios destinos (con el nombre del catálogo si lo tiene);
3. los destinos del catálogo que aún no tiene (B2, S3, rest-server): con el tipo y la dirección ya puestos, pide solo las credenciales; el destino se crea en el equipo con el **mismo id** que en el catálogo, así se agrupa con los demás equipos que lo usan;
4. «Un destino nuevo…», como hasta ahora (y queda en la lista con el nombre que se le dé).

## 7b. Varias zonas en un almacén (= 4f)

### Por qué otra instancia de rest-server

Un rest-server sirve **una sola carpeta** (`--path`), y el agente no admite enlaces dentro de la del almacén (un enlace a `E:` sería una puerta para que un usuario del equipo cambie dónde escribe SYSTEM). Así que `E:\Resguardo` necesita **otro rest-server**, con **su puerto**:

- el **mismo certificado** (la misma autoridad del almacén, que los equipos ya fijan, y el mismo certificado del servidor, que cubre sus nombres e IP; el puerto no cuenta en TLS);
- siempre `--append-only --private-repos`, como la principal;
- **sus propios usuarios** (`.htpasswd` propio en la carpeta privada: `servidor-zona-<id>.htpasswd`). Un equipo que copia en las dos zonas tiene un usuario en cada una, con contraseñas distintas.

### En el agente (`crates/agente/src/server.rs`)

`servidor.json` gana `zonas: [{ id, nombre, path, port, users, creada }]` (sin él, como siempre: solo la principal).

- **La principal** sigue siendo `path`/`port`/`users` de siempre: nada cambia para los equipos que ya copian.
- **Crear una zona** (`guarda_copias { zona: { nombre?, carpeta, puerto } }`, clave de administración): la carpeta se valida como la principal (un disco del equipo, no la raíz ni de red, sin enlaces, fuera de Windows/programas/Resguardo, solo para SYSTEM y administradores) y además **no puede solaparse** con la principal, con otra zona ni con una carpeta del espejo (ni al revés: el espejo tampoco puede ir dentro de una zona). El puerto: 1024–65535, distinto de los demás del almacén y **libre ahora** (si no, «El puerto N ya está en uso…», como al activar). El id lo pone el agente (`z` + 6 cifras hexadecimales al azar). Hasta 8 zonas.
- **Cambiar el nombre** de una zona (`{ zona: { id, nombre } }`): no espera (solo es lo que se ve). Cambiar su carpeta o su puerto **no** se puede: se quita y se crea otra (los equipos tendrían que cambiar su destino).
- **Quitar una zona** (`{ quitar_zona: "<id>" }`): **espera** (reduce la protección: las copias que van allí dejan de poder entrar). Para su rest-server, quita su puerto del cortafuegos y olvida sus usuarios. **Lo guardado se queda** en su carpeta.
- **Añadir un equipo a una zona** (`{ anadir: "<equipo>", zona: "<id>" }`, con `responder_a`): como siempre, pero el usuario es de esa zona y `donde` lleva su puerto. La respuesta sellada trae además `zona: "<id>"`. Con `local: true`, por `localhost` (su propio almacén en otra zona).
- **Quitar un equipo de una zona** (`{ quitar: "<usuario>", zona: "<id>" }`): espera, como `quitar` en la principal.
- **Los nombres de usuario son únicos en todo el almacén** (todas las zonas): si «recepcion» ya está en la principal, en la zona E será «recepcion-2». Así la **retención en el almacén** (`retencion_almacen { usuario, repo }`, v1.22) sigue sin cambios: el agente busca en qué zona está ese usuario y poda allí. Lo mismo `aplicar_retencion_almacen`.
- **Arranque**: el proceso que mantiene el rest-server (`--server-run`, la tarea de SYSTEM o el servicio de systemd) pasa a **vigilar varias instancias**: cada 5 s mira `servidor.json`, arranca las que faltan (con la misma espera creciente si se caen), para las de zonas quitadas o cambiadas y comprueba la huella del binario antes de cada arranque. Cada instancia guarda su PID (`servidor.pid`, `servidor-zona-<id>.pid`) y solo se para por su PID si sigue siendo el rest-server. Si cambia la IP, el certificado se renueva una vez y se reinician todas.
- **Cortafuegos**: una regla con **todos los puertos** del almacén (`localport=8000,8002`), con la misma opción «solo redes internas» de la principal (una sola para todo el almacén). En Linux, la tabla `inet resguardo` con todos los puertos.
- **Espacio libre** por zona: `espacio` (el volumen de su carpeta), como el de la principal.
- **Lo que no cambia en la parte A:** el **espejo** de hoy copia la zona principal (`<carpeta>/<usuario>/<repo>`). Copiar una zona a otra o una zona a la nube es un paso «espejo» de la parte B (7d.2), que hará el almacén en local con el mismo motor (`espejo_motor::vuelta` con otra carpeta de origen).

### Resumen

`guarda_copias.zonas: [{ id, nombre, carpeta, puerto, usuarios, escucha, espacio, repositorios: [{ usuario, repos }] }]` (solo si hay zonas; `escucha`: si su rest-server responde ahora en su puerto). La principal sigue en los campos de siempre (`carpeta`, `puerto`, `usuarios`, `espacio`, `repositorios`). `admite` lleva **`zonas_almacen`**.

### Compatibilidad con los agentes instalados

- Un agente anterior no conoce `zona` ni `quitar_zona`: `{ zona: {…} }` falla («Falta activo, anadir o quitar») y no hace nada; pero `{ anadir, zona }` **añadiría el usuario a la principal sin decirlo**. Por eso la consola **solo** ofrece zonas a un almacén con `admite: "zonas_almacen"`, y además comprueba que la respuesta sellada trae `zona` igual a la pedida (si no, no crea el repositorio y lo dice).
- `servidor.json` con `zonas` leído por un agente anterior (volver a una versión vieja): ignora el campo; las zonas dejan de servirse hasta actualizar otra vez. Lo guardado no se toca.
- El resumen nuevo es un campo más: una consola anterior lo ignora.

## 7c. Copias ordenadas y en cadena (parte B)

- En «Cambiar las copias», la lista de copias de un equipo se puede **ordenar** (el orden de `config.copias[]`, que ya es una lista).
- Cada copia puede llevar **`tras: "<id de otra copia>"`** («después de la anterior»): no tiene horario propio (o lo tiene además); empieza cuando la otra termina **bien**. Si la otra falla, esta no empieza y la cadena se para con un aviso nuevo **`cadena_parada`** (importante: «Documentos → Disco E no se hizo porque la copia anterior falló»).
- Sin choques: las copias de una cadena van una detrás de otra en el mismo equipo (el agente ya no lanza dos restic a la vez sobre el mismo repositorio; la cola de `jobs.rs` sirve).
- La consola enseña la cadena como una línea: «Documentos → Almacén · Disco D → después → Almacén · Disco E → después → Dropbox».
- Compatible: un agente sin `admite: "cadenas"` ignoraría `tras` y haría la copia solo con su horario; la consola no lo ofrece.

## 7d. Los tres tipos de paso (parte B)

Al añadir un paso debajo de una copia:

1. **Copia nueva**: carpetas del equipo a un repositorio (como hoy). Con `tras`, después de la anterior.
2. **Espejo**: el **mismo** repositorio en otro destino (mismos archivos, misma contraseña; en el destino, `<destino>/<usuario>/<repo>`).
   - **Lo hace el almacén donde está el repositorio**, en local y sin contraseñas, con el motor de [espejo.md](espejo.md) (`espejo_motor::vuelta`) y alcance de **un** repositorio: es un destino de su espejo con `repos: ["<usuario>/<repo>"]` y `tras_copia: true` (la detección de versiones nuevas en `snapshots/`, §3a, limitada a ese repositorio). Lo nuevo es el **origen por zona** (`zona` en el destino del espejo: copiar desde la carpeta de esa zona) y que el destino pueda ser **otra zona** del mismo almacén (`tipo: "zona"`, sin rest-server de por medio: carpeta a carpeta).
   - **Con la retención del original** (`retencion_dias`, con su freno de §3b) o **sin retención** (nunca borra).
   - Si el repositorio no está en un almacén (p. ej. B2 directamente desde el equipo), un espejo no tiene sentido sin contraseña: se ofrece la opción 3.
3. **Repositorio nuevo a partir del anterior**: lo hace el **equipo dueño**, que tiene las dos contraseñas.
   - Se crea en el destino elegido con `init --from-repo … --copy-chunker-params` (deduplica con el anterior), se le **traen las versiones** (`copiar_historial`, que ya existe, con filtros nuevos: `etiquetas`, `equipos` (host), `carpetas` (path) y fechas, que se resuelven a ids: «desde el 1 de enero», «los últimos 90 días»; `restic copy` acepta `--tag`, `--host`, `--path` e ids).
   - **Contraseña**: la misma que el anterior u otra (escrita o generada); siempre al kit. La consola recomienda otra para destinos fuera de la oficina, sin impedir la misma.
   - **Desde entonces**, una de dos: **traer las versiones nuevas** del anterior cada vez (`restic copy` con los mismos filtros, después de la anterior o con horario: es una **copia derivada**, la generalización de la copia externa de hoy), o **copiar las carpetas directamente** (una copia nueva que apunta a este repositorio).
   - Su propia retención (desde el equipo; en un almacén, la retención en el almacén) y su verificación.

## 7f. Crear una copia: destino primero, repositorio después (parte B)

«Añadir una copia» en el equipo:

1. **Qué**: carpetas del equipo (copia nueva) o el repositorio de otra copia (un paso de la cadena: espejo o repositorio a partir del anterior).
2. **Cuándo**: horario (el editor de siempre) o «después de la anterior».
3. **Destino**: de la lista de 7a, o crear uno ahí mismo («Nuevo destino», sin salir del flujo).
4. **Repositorio en ese destino**:
   - espejo: nada que elegir (el mismo repositorio, en `<destino>/<usuario>/<repo>`);
   - copia nueva: uno que ya existe en ese destino (lista; «Usar uno que ya existe») o uno nuevo (nombre y contraseña escrita o generada, al kit);
   - a partir del anterior: nombre, contraseña (la misma u otra) y qué versiones traer.
5. **Traer versiones**: `copiar_historial` con los filtros.
6. **Resumen** con el camino completo («Documentos (RECEPCION) → Almacén · Disco D → espejo Disco E y Dropbox; copia a B2») y la clave de administración. Con la tarea 8, el resumen dice cómo queda la regla 3-2-1-1-0 y propone el paso que falta.

## 7e. Convivencia con el espejo de hoy (parte B)

- El espejo global del almacén («todo lo que guarda», 3) sigue funcionando igual.
- La consola explica que los espejos por copia (7d.2) son la forma nueva y, para un destino del espejo con `repos` de un solo repositorio, ofrece **pasarlo a un paso de la cadena** de esa copia: no se mueve ningún archivo (es el mismo destino del espejo, solo cambia dónde se enseña y se edita).
- Un destino «todos» se queda en el almacén (no es de ninguna copia).

## Seguridad de la tarea 7

Lo que se acordó en el plan, y cómo se cumple:

- **El almacén nunca tiene contraseñas de repositorios** (salvo la clave propia de la retención en el almacén, opcional, v1.22). Los espejos los hace en local copiando archivos cifrados; «traer las versiones» lo hace el equipo dueño.
- **Los equipos no necesitan acceso a la zona de un espejo.** Si se les da (una copia independiente en esa zona), es en **solo añadir** y con `--private-repos`, como la principal: cada zona es un rest-server con esos dos argumentos siempre (prueba `cada_zona_es_de_solo_anadir`).
- **Recomendar un destino fuera del alcance de la retención**: el espejo con retención sigue a la del original (con retraso y freno, §3b). Si todos los destinos de una cadena la siguen, un equipo comprometido que llene el repositorio de versiones basura (lo que la retención del almacén limita con su margen de 48 h) acabaría desplazando las buenas en todos. La consola recomienda (sin obligar) al menos un espejo sin retención, una copia independiente con su retención o una nube con bloqueo de objetos.
- **Dropbox, Drive, SMB, SFTP y WebDAV no son inmutables**: se dice al elegirlos.
- **Credenciales**: siempre selladas para el equipo que las usa; el catálogo de destinos no las admite (se rechaza cualquier campo de más).
- **Todo lo que reduce la protección espera**: quitar una zona, quitar un usuario de una zona, quitar un paso de una cadena o pasar un espejo a «con retención».
- **Zonas**: sin solapes entre carpetas (zonas, principal y espejo), sin enlaces, solo SYSTEM y administradores; un puerto ocupado no se toma; el PID de otra cosa no se para.

## Cómo encaja la tarea 4

| Tarea 4 | En el modelo de la 7 | Parte |
|---|---|---|
| **4a.** Dropbox y las nubes de rclone como destino de la copia externa | Un destino `nube` conectado en el **equipo dueño** (`conectar_nube` también fuera de un almacén) y el repositorio con `rclone:` y el rclone incluido (el agente ya pone `-o rclone.program=`). Sirve para los pasos «repositorio a partir del anterior» y «copia nueva». | B |
| **4b.** Varias copias externas por repositorio | Varios pasos «repositorio a partir del anterior» (7d.3) colgando del mismo repositorio, cada uno con su destino, su contraseña, su retención, su horario o «después de la anterior», y su verificación. `externa` de hoy pasa a ser el primero de esa lista (`derivadas[]`), con convivencia: un agente anterior sigue leyendo `externa`. | B |
| **4c.** Filtros | Los filtros de 7d.3 (etiquetas, equipo, carpetas, fechas), en `copiar_historial` y en cada copia derivada. | B |
| **4d.** El flujo de cada copia en la consola | La línea de la cadena (7c) y el resumen de 7f. | B |
| **4e.** (descartada) | — | — |
| **4f.** Más de una carpeta servida por el almacén | **= 7b** (zonas). | **A** |

## Contrato (parte A)

En [api-servidor.md](api-servidor.md) → «Cambios», como «v1.53»:

- `guarda_copias { zona: { nombre?, carpeta, puerto } }`, `{ zona: { id, nombre } }`, `{ quitar_zona }`*, `{ anadir, zona }`, `{ quitar, zona }`*; `admite: "zonas_almacen"`; resumen `guarda_copias.zonas[]`.
- `GET/PUT/DELETE /api/clientes/{c}/destinos[/{id}]` (el catálogo).

## Lo que queda para la parte B

Para la sesión que lo retome, en este orden (cada punto con sus pruebas, como en la parte A):

1. **Pasos «espejo» por repositorio desde una zona** (7d.2): `Destino` del espejo (`crates/agente/src/espejo.rs`) con `zona?: "<id>"` (origen: la carpeta de esa zona; sin él, la principal) y el tipo de destino `zona` (otra zona del mismo almacén, carpeta a carpeta). `poner_espejo` valida que no se copie una zona en sí misma. `espejo::repos_en` por zona. El resumen de cada destino dice su zona. Prueba: espejo de un repositorio de la zona E a la zona D y a una carpeta, con y sin retención.
2. **Cadenas** (7c) en el agente: `config.copias[].tras`, la cola que lanza la siguiente al terminar bien, el aviso `cadena_parada` (servidor: `notificaciones/problemas.rs`, desde el resumen o el historial) y `admite: "cadenas"`. Prueba de que una copia fallida no lanza la siguiente.
3. **Copias derivadas** (4b, 7d.3): `derivadas[]` por repositorio en el vínculo (generaliza `RepoV2.externa` y `tasks::Offsite`, que pasa a ser una lista), órdenes `cambiar_derivada { repo, id, … }` / `quitar_derivada`* con el contrato de `cambiar_copia_externa` (destino, `existente`, `bloqueo_dias`, retención, verificación, `solo_probar`) más `tras`, `contrasena_destino` (la misma u otra) y filtros. `externa` sigue para los agentes y las consolas anteriores.
4. **Filtros** (4c): en `copiar_historial` y en las derivadas: `etiquetas`, `equipos`, `carpetas`, `desde`, `ultimos_dias` (se resuelven a ids con `restic snapshots --json`). `adoptar_v2::Filtro` ya tiene equipos y etiquetas.
5. **Nubes en el equipo dueño** (4a): `conectar_nube` sin exigir `guarda_copias` (hoy `nube.rs` lo exige), y `ubicacion()` de `gestion_v2.rs` con el tipo `nube` → `rclone:rnube:<carpeta>/<repo>` con las variables de entorno de `nube.rs`. Avisar de que Dropbox no es inmutable.
6. **Consola**: «Cambiar las copias» con orden y «después de la anterior»; el flujo de 7f (componente nuevo, sustituye a «Nuevo repositorio» + «Copia externa» como camino principal); la línea de la cadena en la ficha de la copia y en el mapa (4d); 7e (pasar un destino del espejo a un paso); el aviso de «al menos un destino fuera del alcance de la retención». Simulador (`dev:mock`) y vectores.
7. **e2e**: un paso que haga Documentos → zona D → (después) espejo a zona E → (después) repositorio a partir del anterior en una carpeta con otra contraseña y filtro de fechas.
8. **Exportar el catálogo de destinos** en el paquete `.resguardo-cliente` (al mover un cliente a otro servidor) y en «Recibir un cliente».

## Parte B: lo que se hizo (rama `ia/copias-en-cadena`)

Contrato en [api-servidor.md](api-servidor.md) → «Cambios», v1.55 (copias en cadena). Lo dudoso y lo que no se pudo probar, en [registro-ia.md](registro-ia.md).

1. **Pasos «espejo» por zona** (7d.2): hecho. `Destino.zona` del espejo (de qué zona copia) y el tipo `zona` (a otra zona del almacén, carpeta a carpeta). El almacén lo hace en local con `espejo_motor::vuelta`; cada origen lleva su archivo de estado. `poner_espejo` rechaza copiar una zona en sí misma y `quitar_zona` se niega si el espejo la usa. «Después de cada copia» mira solo los repositorios del destino en su zona. `admite: "espejo_zonas"`. Un destino «zona» o «carpeta» con `repos` de un solo repositorio, `zona` y `tras_copia` es el paso «espejo» de una copia (la consola lo crea con «Añadir un paso «espejo»…»).
2. **Cadenas** (7c): hecho. `config.copias[].tras`; el plan del agente lleva `after` (`<repo>#<copia>`); en una misma vuelta, hasta 8 pasos seguidos. Si la anterior falla: no empieza, se anota una vez por fallo (`state.chains`, historial `chain`) y el informe lleva `cadenas[]` → aviso `cadena_parada`. «Bien» incluye «sin cambios» y «con algún archivo sin leer» (`warning`). `admite: "cadenas"`.
3. **Copias derivadas** (4b, 7d.3): hecho. `cambiar_derivada` / `quitar_derivada`, `AgentRepo.derived` y `Secret.derived` en el agente, `RepoV2.derivadas` en el vínculo. La copia externa de siempre sigue igual y es la primera (`externa`, `offsite`): así un agente o una consola anteriores siguen viéndola, y la «Salud de la protección» (`protection.rs`, no se tocó) la sigue contando. Las derivadas usan la misma tarea (`offsite_repo`, `verify_repo`) con su clave propia en `tasks.json` (`derivada:<repo>:<id>`), respetan la pausa y el freno ante cambios inusuales y van al informe (aviso `externa_fallida`, sujeto `<repo>--<id>`).
4. **Filtros** (4c): hecho. `adoptar_v2::Filtro` con `carpetas`, `desde` y `ultimos_dias`, resuelto a ids (en `copiar_historial`, que ahora pasa ids por tandas de 100, y en cada derivada). `admite: "filtros"`.
5. **Nubes en el equipo dueño** (4a): hecho para las copias derivadas. `conectar_nube` en cualquier equipo; destino `nube` (`rclone:rnube:<carpeta>/<repo>`) con las credenciales puestas al día en cada uso (`nube::entorno_restic`; nunca se guardan con la copia). **Después (rama `ia/repos-en-la-nube`): hecho también copiar las carpetas directamente a una nube.** Con la copia solo se guarda una marca con el nombre de la nube; cada proceso de restic (la vuelta del agente, la verificación, explorar, restaurar…) recibe la nube al día y un archivo de configuración propio, donde rclone deja el token si lo renueva a mitad; al terminar se vuelve a sellar (detalle y límites en [destinos.md](destinos.md), «Repositorios directamente en una nube»). `crear_repositorio` admite el destino `nube` con `admite: "repo_en_nube"`. SFTP conectado por rclone ya tiene su clave del servidor en un archivo de la vuelta (sin probar).
6. **Consola**: hecho. «Cambiar las copias» con orden (subir/bajar) y «Empieza: después de «X»» (con o sin horario propio); la línea de cada copia (`lib/cadenas.ts`, con vectores) en «Cambiar las copias» con la recomendación de un destino fuera del alcance de la retención; copias derivadas (`CopiaDerivada.svelte`: destino del equipo, una nube conectada en él o uno nuevo; cuándo; la misma contraseña u otra, generada y con su kit; retención, bloqueo, filtro y verificación; «Probar»; aviso de que Dropbox y similares no son inmutables) y su lista en la ficha del repositorio con «Cambiar», «Subir ahora» y «Quitar»; el paso «espejo» (`PasoEspejo.svelte`); «Añadir una copia» (7f, `AnadirCopia.svelte`: qué primero y después el diálogo de cada tipo); 7e: en el almacén, un destino del espejo de un solo repositorio dice de qué copia es paso (no se mueve nada). «Traer el historial» con «Solo algunas versiones» (etiqueta, últimos días, desde). **Pendiente (no hecho):** la línea de la cadena en el mapa de protección (el mapa ya enseña las zonas por su nombre) y un asistente 7f de una sola pantalla que cree también un repositorio nuevo en el destino (hoy «carpetas» lleva a «Cambiar las copias» y el repositorio se crea con «Nuevo repositorio», como siempre).
7. **e2e**: paso 3c (Documentos → zona D; después → zona E por la cadena; espejo de la zona E a la principal en el almacén; copia derivada de la zona E a una «nube» por rclone —en las pruebas, una carpeta con el tipo `alias`, solo en compilaciones de desarrollo con `RESGUARDO_AGENT_DIR`— con otra contraseña y filtro de fechas).
8. **Catálogo en el paquete `.resguardo-cliente`**: hecho (lo junta y lo pone la consola; sin cambios en el servidor).

**Después (rama `ia/editor-de-copias`):** «Añadir una copia» (`AnadirCopia.svelte`) se quitó: todo se añade, se cambia y se ordena en el editor de copias, con «Añadir paso» en cada copia. Ver [editor-de-copias.md](editor-de-copias.md).

**Para la tarea 8 (3-2-1-1-0):** `lib/cadenas.ts::lineaCadena(equipo, copia, equipos)` da los pasos de cada copia con `clase` («origen», «copia», «espejo», «derivada»), `destinoId` (o `zona:<almacén>:<zona>`), `tipoDestino`, `inmutable` (bloqueo de objetos o solo añadir; `null` si no se sabe), `fueraRetencion` y `despues`. En el agente, `RepoV2.derivadas` y `AgentRepo.derived` (con `offsite.dest.object_lock_days` y `retention`) y `guarda_copias.espejo.destinos[]` (con `zona`, `repos`, `retencion_dias`, `bloqueo`).
