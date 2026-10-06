# Destinos y repositorios

Nota de diseño de las fases 1 y 2. Las dos están implementadas: el modelo (fase 1) y qué hay dentro de un destino (fase 2).

## Vocabulario

| Palabra | Qué es | Ejemplo |
|---|---|---|
| **Destino** | Un lugar donde se guardan copias. | Un bucket (o un prefijo) con su clave, un rest-server (URL base y credenciales), un servidor SFTP, una carpeta local o de red. |
| **Repositorio** | Una caja cifrada de restic dentro de un destino, con su propia contraseña. | `copias-ana/portatil` en Backblaze; `E:\Copias\restic`. |
| **Copia** | Un trabajo de copia: qué carpetas, cuándo, con qué exclusiones. | «Laboral», «Domingo». |
| **Versión** | Una instantánea (snapshot) que guardó una copia. | «Versión del 1 oct, 19:31». |

Hasta la versión 0.6.3, la app llamaba «destino» a lo que ahora es un repositorio. Un destino puede tener varios repositorios: uno por equipo, por cliente o por tipo de datos.

```
Backblaze B2 · copias-ana          ← destino (bucket y su clave)
├── portatil/        (contraseña propia)   ← repositorio
├── disco-externo/   (contraseña propia)
└── servidor/        (contraseña propia)
```

## Fase 1: el modelo

### Datos

- `Repo` (en `repos.json`, como hasta ahora) sigue siendo la unidad con identidad estable. Su `id` no cambia, así que las contraseñas del almacén de credenciales (indexadas por `id`), la configuración y el estado del agente, el historial y lo que ya tiene la web siguen enganchados.
- Nuevo campo `Repo.place_id` (opcional en el JSON): el destino al que pertenece.
- Nuevo archivo `places.json` (carpeta de configuración de la app, por usuario): la lista de destinos.

  ```json
  [{ "id": "8f1c…uuid", "name": "Backblaze B2 · copias-ana", "key": "s3:s3.us-west-004.backblazeb2.com/copias-ana" }]
  ```

  - `key` es la clave de agrupación, calculada a partir de la ubicación del repositorio (ver la tabla siguiente). Es local y nunca se envía a la web.
  - `name` es el nombre que ve el usuario; se puede cambiar.

| Tipo | Ubicación del repositorio | Clave del destino | Nombre por defecto |
|---|---|---|---|
| S3 compatible | `s3:https://host/bucket/prefijo` | `s3:host/bucket` (servidor en minúsculas, sin usuario) | `Backblaze B2 · bucket` (según el servidor) |
| B2 nativo | `b2:bucket:ruta` | `b2:bucket` | `Backblaze B2 · bucket` |
| Azure / GCS | `azure:contenedor:/ruta` | `azure:contenedor` | `Azure · contenedor` |
| rest-server | `rest:https://usuario:clave@host:8000/ruta/` | `rest:https://host:8000` (sin credenciales) | `Servidor host` |
| SFTP | `sftp:usuario@host:/ruta` | `sftp:usuario@host` | `SFTP host` |
| rclone | `rclone:remoto:ruta` | `rclone:remoto` | `rclone remoto` |
| Carpeta local | `E:\Copias\restic` | carpeta padre, en minúsculas: `e:/copias` | `Unidad E:` |
| Carpeta de red | `\\nas\copias\portatil` | carpeta padre: `//nas/copias` | `nas · copias` |

Los nombres por defecto nunca incluyen una ruta local completa, porque se envían a la web.

### Migración (sin intervención del usuario)

1. Al leer `repos.json`, cada repositorio sin `place_id`, o con un `place_id` que no existe, se asigna a un destino:
   - el que tenga su misma clave;
   - si no hay ninguno, uno nuevo con el nombre por defecto.
2. Se escriben `places.json` y `repos.json`. Si escribir falla, se sigue con la asignación en memoria y se reintenta en la siguiente lectura.
3. No se toca el almacén de credenciales ni la configuración del agente. Es idempotente: una segunda lectura no cambia nada.
4. Al añadir un repositorio se asigna igual. Al quitar el último repositorio de un destino, el destino se borra.

### Agente y web

- `AgentRepo` gana tres campos opcionales, que llegan con `sync_meta` y al programar: `place_id`, `place_name` y `place_kind` (`s3`, `b2`, `rest`, `sftp`, `local`…).
- **Contrato para la web.** Cada elemento de `repos` del informe (`device_report`) gana un campo `place`. Es opcional: un agente antiguo no lo envía y la web debe tratarlo como un destino propio por repositorio.

  ```json
  "place": { "id": "uuid", "name": "Backblaze B2 · copias-ana", "kind": "s3" }
  ```

  - `id`: estable, igual para todos los repositorios del mismo destino de este equipo. No es global entre equipos: dos equipos con el mismo bucket tienen destinos distintos hasta la fase 3.
  - `name`: el nombre visible (ya sin rutas locales).
  - `kind`: el mismo vocabulario que el `kind` del repositorio.
  - El resto del informe no cambia: `repos[].id`, `name`, `kind`, `host`… siguen igual.
  - **Sugerencia para la web:** agrupar por `place.id` bajo cada equipo y mostrar «Destino › Repositorio».

### Interfaz

- **Barra lateral.** «Destinos» lista los destinos; cada uno se despliega y muestra sus repositorios. Un destino con un solo repositorio también se despliega, para que el modelo se entienda. Se recuerda qué destinos están plegados.
- **Página de un repositorio** (la antigua «página de destino»):
  - una miga «Destino «X» ›» encima del nombre;
  - las acciones dicen «repositorio» («Renombrar repositorio», «Quitar repositorio»).
- **Renombrar un destino** solo cambia su nombre: no pide contraseña porque no toca ningún repositorio.
- El resto de textos se irán pasando a «repositorio» donde se refieren a la caja cifrada. Ver «Pendiente».

## Fase 2: dentro de un destino

### Página del destino

Al pulsar un destino en la barra lateral se abre su página:

- el tipo, la ubicación base y el nombre (que se puede renombrar);
- sus repositorios: los conocidos y los encontrados al listarlo, cada uno con su estado.

**Listar repositorios.** Un repositorio es una carpeta o prefijo que contiene el objeto `config`:

- **Carpeta local o de red:** se recorren dos niveles bajo la carpeta del destino, buscando `config` junto a `keys/`. Va por el sistema de archivos, sin restic.
- **S3 compatible:** `ListObjectsV2` con `delimiter=/` hasta dos niveles (como mucho 60 prefijos), firmado con SigV4 (`s3list.rs`, con `sha2` + `hmac` de RustCrypto; probado con la clave de firma de ejemplo de AWS). Si la clave no puede listar (403), se dice y se ofrece escribir la ruta.
- **B2 nativo, Azure y GCS:** sin listado en la fase 2. Se ofrece «Usar uno existente» escribiendo la ruta.
- **rest-server y SFTP:** el rest-server no lista repositorios. Se recuerdan las rutas conocidas (las de los repositorios que la app ya usa o usó en ese destino, en `places.json` → `known_paths`). Con SFTP también se usan las rutas conocidas.

**Cada repositorio encontrado** muestra su ruta y si este equipo ya lo usa, con dos acciones:

- **«Usar este»:** pide su contraseña, la comprueba con `restic cat config`, crea el `Repo` local y lo guarda en el almacén. Avisa: «si otro equipo también copia aquí, los dos escribís en el mismo repositorio y quien tenga la contraseña ve todas las versiones».
- **«Crear uno nuevo aquí»:**
  - ruta sugerida `<equipo>/<nombre>`, que se puede editar;
  - contraseña aleatoria de 32 caracteres (`restic init`);
  - al terminar se abre el flujo del kit de recuperación, porque esa contraseña solo está en este equipo.

**Clonar a otro destino.** `restic copy` de un repositorio a otro nuevo:

- primero se crea el de llegada con `init --from-repo … --copy-chunker-params`, para que la deduplicación siga funcionando entre los dos;
- el progreso se muestra con `--json`, en una tarea cancelable como las copias;
- al terminar se ofrece pasar las copias (planes) al nuevo repositorio con «Cambiar destino», que ya existe.

Mover copias entre repositorios ya existe («Cambiar destino») y no cambia.

### Seguridad

- Listar un bucket o una carpeta solo usa las credenciales del destino, que este equipo ya tiene. No se envía nada a la web.
- Las contraseñas de los repositorios nunca salen de este equipo. «Usar este» exige escribirla.
- La contraseña generada se guarda en el almacén de credenciales y solo se muestra en el kit.

## Copia externa a un repositorio que ya existe

En la consola, «Copia externa…» de un repositorio tiene dos caminos (`cambiar_copia_externa`, [api-servidor.md](api-servidor.md) §5, v1.46):

- **Crear uno nuevo** (como siempre): en un destino del equipo o en uno nuevo, en una carpeta con el id del repositorio. Al guardarla, el agente lo crea allí con los parámetros de troceado del origen (`init --from-repo … --copy-chunker-params`) o, si ya hay uno que la contraseña abre, sigue con él.
- **Usar uno que ya existe**: la dirección completa (bucket y carpeta en B2/S3, servidor y carpeta en un rest-server, carpeta, SFTP), sus credenciales y **la contraseña de ese repositorio**. Es el caso de la subida a la nube de la app de escritorio: el repositorio antiguo se movió a un almacén (uno nuevo con su troceado y todo su historial traído, `copiar_historial`) y la copia externa del nuevo sigue yendo al repositorio de la nube de siempre. «Probar» (`solo_probar`) lo abre (`restic cat config`), cuenta sus versiones y compara su troceado con el del origen. El agente nunca lo crea: si un día no aparece, la subida falla y lo dice, en vez de crear uno vacío que obligaría a subirlo todo otra vez.

**Qué sube `restic copy`** (comprobado con restic 0.19.1; pruebas `copia_externa_a_un_repositorio_que_ya_existe` y `retencion_con_bloqueo_de_objetos` de `tasks.rs`):

- Los bloques se identifican por el hash de su contenido, no por la clave del repositorio: `restic copy` solo lee del origen y sube al destino los bloques que el destino aún no tiene. Para que un mismo archivo dé los mismos bloques, los dos repositorios tienen que trocear igual (mismo `chunker_polynomial`): por eso el nuevo se crea con `--copy-chunker-params` y «Probar» avisa si no es así («este repositorio trocea distinto: la primera subida ocupará como una copia completa»; se puede usar igual).
- Una versión que ya está en el destino no se copia otra vez: restic busca en el destino una versión con el mismo `original` (la versión de la que es copia) y los mismos datos (hora, equipo, carpetas, árbol). Una versión traída al almacén desde el repositorio antiguo conserva su `original`, así que las que la app de escritorio ya subió desde el antiguo cuentan como subidas. El agente las descuenta igual antes de empezar: el progreso y el mensaje («1 copia subida (213 ya estaban en el destino).») hablan solo de lo que falta.
- En la prueba: un repositorio con 3 MiB ya subido a «la nube» y movido a un almacén con su historial; tras 1 MiB nuevo, la copia externa del almacén a la misma «nube» añadió ≈1 MiB (y una sola versión), no los 4 MiB.
- `restic copy` no tiene `--dry-run` (0.19.1): no hay una cifra exacta de lo que falta antes de empezar. El agente estima los bytes con lo que añadió cada versión pendiente (`data_added_packed`, restic ≥ 0.17) y la consola enseña el ritmo real de subida del proceso de restic (como hasta ahora).

### Bloqueo de objetos (Object Lock) en el destino

Con un bucket con bloqueo de objetos (B2 u otro S3 compatible), nada de lo subido se puede borrar ni sobrescribir durante N días: es lo que protege la copia de alguien que tenga la clave (un ransomware). Para restic significa:

- **`prune` no sirve y sale caro**: reescribe y sube paquetes nuevos para quitar los viejos, que no se pueden borrar (con la API S3 se ocultan con una marca de borrado pero se siguen guardando y cobrando hasta que vence el bloqueo; con la API nativa de B2 el borrado falla). Se pagaría dos veces sin liberar nada.
- **`forget` sí** para versiones más antiguas que el bloqueo (sus archivos de versión ya no están bloqueados).

Por eso, con «El destino tiene bloqueo de objetos: N días» (`bloqueo_dias`), la retención propia de la copia externa solo hace `forget` (nunca `--prune`) y siempre guarda lo de los últimos N+1 días (`--keep-within`), aunque la regla diga menos. Si restic no puede quitar alguna versión porque aún está bloqueada (p. ej. versiones antiguas subidas hace poco), lo dice en el resultado («… siguen bloqueadas: se quitarán cuando venza su bloqueo») y vuelve a intentarlo en la siguiente subida. Sin retención propia, allí no se borra nada. **El espacio no se libera solo**: el bucket guarda todo lo subido. Si hace falta liberarlo, hay que hacerlo a mano y a sabiendas (un `restic prune` desde un equipo con la clave, cuando lo que se quiere quitar ya no esté bloqueado; y en B2, una regla de ciclo de vida que borre las versiones ocultas cuando venza su bloqueo).

**Gobernanza o cumplimiento.** En modo *cumplimiento* (compliance) nadie, ni la cuenta dueña, puede borrar ni acortar el bloqueo. En modo *gobernanza* (governance) una clave con permiso para saltarse el bloqueo puede borrar; restic nunca lo pide, así que para Resguardo los dos se comportan igual, pero en gobernanza el dueño de la cuenta puede arreglar un error (p. ej. un periodo demasiado largo) y en cumplimiento no. Para copias, gobernanza con una clave de aplicación sin ese permiso suele bastar; cumplimiento solo si un tercero lo exige. Los archivos de bloqueo de restic (`locks/`) también quedan bloqueados: si no se pueden borrar al terminar, restic los da por viejos a los 30 minutos y no estorban.

Un **rest-server de solo añadir** como destino de la copia externa se detecta al guardarla (`solo_anadir`): allí no se aplica la retención desde el equipo (la aplica el propio servidor).

## Espejo del almacén

Horario por destino, «después de cada copia nueva», selección de repositorios, verificación sin contraseñas, retención con freno, más destinos por rclone y restaurar desde el espejo: ver [espejo.md](espejo.md).

## Espejo en Dropbox: conectar desde la consola y renovar el token

**Conectar.** La consola hace OAuth 2 con PKCE contra la app «Resguardo» de Dropbox (app key pública `beobf3c13cvlrup`, la que da `GET /api/servidor`; permiso «App folder»; sin app secret, que ni se pide ni se guarda), cambia el código por el token en el navegador y lo manda sellado al equipo en `conectar_nube` (ver [api-servidor.md](api-servidor.md) §12). El agente guarda el refresh token protegido (DPAPI) junto con la app key, en el formato de token de rclone: `{ access_token, token_type: "bearer", refresh_token, expiry }`.

**Renovar: lo hace el agente, y rclone como respaldo.** Decisión:

- Antes de cada vuelta del espejo, si el access token caduca en menos de 30 min, el **agente** lo renueva él mismo: `POST https://api.dropboxapi.com/oauth2/token` con `grant_type=refresh_token`, `refresh_token` y `client_id` (sin secreto: la forma que Dropbox pide a un cliente público con PKCE). Guarda el nuevo y se lo pasa a rclone ya fresco (dura 4 h). Así la petición que importa es nuestra, conocida y probada, y no depende de cómo una versión de rclone hable con Dropbox.
- rclone recibe también el refresh token y `RCLONE_CONFIG_RNUBE_CLIENT_ID=<app key>` (sin `CLIENT_SECRET`). Si una vuelta dura más de 4 h (la primera subida de un almacén grande), renueva él. Comprobado con rclone 1.75.1 contra un servidor de tokens falso (`RCLONE_CONFIG_RNUBE_TOKEN_URL`): con un `client_id` propio, rclone borra su secreto de serie; primero prueba `Authorization: Basic <app key>:` y, si falla, repite con `client_id` en el cuerpo y sin `client_secret`, que es la forma válida para PKCE. Si rclone guardara el token renovado en su archivo de configuración (en la carpeta privada), el agente lo recoge y lo protege como siempre.
- Si Dropbox no responde, se usa el access token que haya mientras valga; si ya caducó, ese destino falla en esta vuelta y se reintenta en la siguiente (con `--immutable`, retoma donde quedó). `invalid_grant` (permiso retirado) o `invalid_client` (app key que no es): error claro, «vuelve a conectar la nube desde la consola».
- Las nubes conectadas con `resguardo-agente nube conectar` (rclone authorize, con la app de rclone) no cambian: renueva rclone como antes.

## Hecho en la fase 2

- `discover.rs` (`place_scan`, `clone_repo`, `cancel_clone`), `s3list.rs`, `Place.known`.
- `add_repo` acepta `rest_from` (el usuario del rest-server de otro repositorio del mismo destino) además de `cloud_from`.
- Interfaz: página del destino (`PlaceView`), «Usar este» / «Crear uno nuevo aquí» (`PlaceRepoDialog`, contraseña generada de 6×4 caracteres sin ambigüedades, ~124 bits) y «Clonar…» (`CloneDialog`, con progreso y «Detener»).
- Clonar entre dos nubes con claves distintas no se permite (restic usa un solo juego de variables `AWS_*`): se explica y se propone pasar por un disco o un servidor.
- Los textos de la app ya usan «destino» y «repositorio» con este sentido en todas partes.
