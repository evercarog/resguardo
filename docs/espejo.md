# El espejo del almacén (tarea 3 del plan)

Diseño de la tarea 3 de [plan-mejoras.md](plan-mejoras.md) («Espejo más flexible»). Lo implementa `crates/agente/src/espejo.rs` (el motor) con `nube.rs` (los destinos por rclone); la consola lo enseña en la ficha del equipo que guarda copias. Pensado para que la tarea 7 («espejo» como paso de una cadena, 7d) reutilice el mismo motor.

## Hasta ahora

- Una vez al día, a una hora (`espejo.hora`), el almacén copia **todo** lo que guarda (`<carpeta del almacén>/<usuario>/<repo>/…`) a cada destino: carpetas de discos del equipo o Dropbox/Drive (`rclone copy --immutable`).
- Nunca borra; no copia `locks/` ni lo modificado hace menos de 10 min; no reescribe lo que ya está (otro tamaño = error).
- Nadie comprueba que lo copiado esté bien.

## Lo nuevo, de un vistazo

Cada **destino** del espejo lleva su propia configuración. Todo es opcional: un destino sin nada nuevo se comporta exactamente como antes.

| Campo (en `guarda_copias.espejo.destinos[]`) | Qué es | Sin él |
|---|---|---|
| `horario` | `{ dias, horas, reglas? }`, el mismo de las copias (§3a) | cada día a `espejo.hora` |
| `tras_copia` | `true`: también «después de cada copia nueva» (§3a) | no |
| `repos` | `["<usuario>/<repo>", …]`: solo esos repositorios (§3f) | todos |
| `retencion_dias` | borra lo que ya no está en el origen pasados N días (7 a 3650; §3b) | nunca borra |
| `bloqueo` | `true`: destino con bloqueo de objetos (Object Lock) o que no se debe tocar nunca: no se borra nada (§3b) | no |
| `verificar_pct` | % de los archivos del destino que se comprueban en cada vuelta (0 a 100; §3d) | 5 en carpetas, 0 en nubes |

Y en `espejo`: `hora` (sigue valiendo: es el horario de los destinos sin `horario`) y `limite_kib`, como antes.

El agente que entiende todo esto lo dice en `resumen.admite`: **`espejo_flexible`** (3a, 3b, 3d, 3f) y **`espejo_destinos`** (3c, más tipos de nube). La consola solo enseña y manda los campos nuevos a un agente que los admite; uno anterior los ignoraría (y haría el espejo de siempre), así que con él la consola ofrece solo lo de siempre.

## El motor: una vuelta a un destino

Una vuelta (`espejo::vuelta`) recibe la carpeta del almacén, el destino y su **alcance** (todos los repositorios o una lista). Es la pieza que reutilizará la tarea 7 (un «espejo» de un solo repositorio como paso de una cadena: alcance de un repositorio, empezando tras su copia).

1. **Lista el origen** dentro del alcance: archivos con su tamaño y fecha, sin `locks/`, sin temporales del espejo y sin enlaces. Lo modificado hace menos de 10 min se deja para la próxima vuelta.
2. **Lista el destino** (carpeta: recorriéndola; nube: `rclone lsjson -R --files-only`).
3. **Copia lo que falta**, comprobando antes cada archivo (§3d). En una carpeta, a un temporal que se renombra; en una nube, `rclone copy --files-from-raw <lista> --immutable` (solo esos archivos, nunca `sync`).
4. **Comprueba una parte del destino** (§3d).
5. **Borra lo vencido**, si el destino tiene retención y no salta el freno (§3b).
6. Anota el resultado del destino y su estado (archivo `espejo/<id>.json` en la carpeta del agente; `<id>`: SHA-256 corto de tipo, nube y carpeta). No hay secretos en ese estado.

Como hoy, no necesita ninguna contraseña de repositorio: no abre nada, solo copia y compara archivos cifrados.

## 3a. Cuándo

- **Horario por destino**: el mismo editor y las mismas reglas que las copias (`crates/motor/src/plans.rs` a través de `gestion_v2::Horario`: días con horas, cada N minutos en una franja, cada N días, cada mes). Toca cuando ha pasado un hueco del horario desde la última vuelta de ese destino (`PlanSchedule::is_due`): si el almacén estaba apagado, una vez al encenderse.
- **Compatibilidad**: un destino sin `horario` usa `espejo.hora` («cada día a esa hora», como antes). Al guardar un espejo nuevo, la consola manda el horario de cada destino y también `hora` (la primera hora del primer destino), para que una consola anterior siga enseñando algo razonable.
- **«Después de cada copia nueva»** (`tras_copia`): en cada vuelta del servicio (como mucho una vez por minuto) el almacén mira la fecha del archivo más reciente de `snapshots/` de cada repositorio del alcance. Si hay alguno posterior al comienzo de la última vuelta de ese destino, el espejo empieza cuando lleva **12 min** sin aparecer otro (así se agrupan varias copias seguidas y la última ya no cuenta como «reciente»), o a los **60 min** de la primera sin copiar, aunque sigan llegando. Se puede combinar con el horario.
- Entre dos vueltas del mismo destino pasan al menos 5 min. Los destinos que tocan a la vez se hacen uno detrás de otro (un solo espejo en marcha).
- El resumen dice la próxima vuelta por horario de cada destino (`proxima`).

## 3f. Qué repositorios van a cada destino

- `repos` ausente: **todos** (como hoy, también los que se creen después).
- `repos: ["ana/contabilidad", "srv/sql"]`: solo esos. En el destino cada uno sigue en su carpeta (`<destino>/<usuario>/<repo>`), así que un destino con selección y otro con todos tienen la misma forma.
- Un nombre se valida (`<usuario>/<repo>`, letras, números, `-`, `_`, `.`, sin `..`). Uno que ya no está en el almacén se salta (y lo dice el resultado).
- **Repositorios nuevos**: el agente guarda, al guardar la selección, los repositorios que había entonces (`vistos`). El resumen da `vistos` de cada destino con selección; la consola compara con `guarda_copias.repositorios` y, si hay alguno nuevo, pregunta en la ficha del almacén: «`srv/nuevo` no entra en 2 destinos con selección. **Añadirlo** / **Dejarlo fuera**». Las dos respuestas vuelven a mandar el espejo (con o sin él) y así se actualiza `vistos`. Por defecto (sin responder), un repositorio nuevo solo va a los destinos de «todos».
- Fundir varios repositorios en uno no se puede con el espejo (cada uno tiene su clave): eso es una copia derivada (tarea 4).

## 3d. Verificación sin contraseñas

Los archivos de restic en `data/`, `index/`, `snapshots/` y `keys/` se llaman como el **SHA-256 de su contenido** (64 cifras hexadecimales; comprobado con restic 0.19.1 en la prueba `nombres_de_restic_son_su_sha256`). `config` no: ese se copia sin comprobar.

- **Antes de copiar** un archivo con nombre de hash, se calcula su SHA-256 (en una carpeta, mientras se copia: se escribe en el temporal y solo se renombra si cuadra). Si no cuadra, **no se copia** (no se propaga un archivo dañado) y la vuelta termina con error: «N archivos dañados en el almacén no se han copiado: revisa el disco del almacén y comprueba esos repositorios».
- **Mismo nombre, otro tamaño** en el destino: se mira el contenido de los dos. Si el del origen está bien y el del destino no, en una carpeta **se repara** (se reescribe con el bueno, comprobado); en una nube se avisa. Si el malo es el del origen, se avisa y el del destino no se toca.
- **Por rotación en el destino**: en cada vuelta se comprueba el `verificar_pct` % de los archivos con nombre de hash del destino (al menos uno si hay), siguiendo por orden donde lo dejó la anterior, como la verificación automática de los repositorios. En una carpeta se leen; en una nube, `rclone hashsum sha256 --download` (descarga: por eso en las nubes va a 0 si no se elige otra cosa). Un archivo malo en una carpeta se repara si el del origen está bien; si no, error.
- Si algo no cuadra, el resultado empieza por `ERROR` y el servidor crea el aviso `espejo_fallido` (ya existe). El resumen de cada destino lleva `verificacion: { ultima, archivos, mal }` y `danados_origen`.

## 3b. Retención del espejo (borrado diferido)

Sin `retencion_dias`, como hoy: **nunca borra**.

Con `retencion_dias: N` (30 por defecto en la consola; mínimo 7, máximo 3650):

- En cada vuelta se miran los archivos del destino (del alcance) que **ya no están en el origen** (los quitó la poda de la retención del almacén). La primera vez que falta uno se anota la fecha; si vuelve a aparecer, se olvida; si sigue faltando **N días después**, se borra del destino (carpeta: el archivo; nube: `rclone delete --files-from-raw`, solo esos archivos).
- **Freno.** Si de una vez (archivos que faltan por primera vez en esta vuelta) falta en el origen el **10 % o más de los archivos del destino** (y al menos 20 archivos), o **un repositorio entero** (su `config`), esa vuelta **no anota ni borra nada** y termina con error (aviso `espejo_fallido`): «Falta de golpe en el almacén el 35 % de lo que hay en el espejo (o el repositorio X entero): no se borra nada. Si fue a propósito (una poda grande, un repositorio quitado), confírmalo en la consola». Es lo que protege de un almacén vaciado o dañado: el espejo no lo sigue.
- **Confirmar**: `guarda_copias { espejo_freno: { tipo, carpeta, nube? } }` (clave de administración; **espera**, como lo que reduce la protección). El agente anota con la fecha de hoy lo que falta en ese destino, y se borrará pasados N días.
- `config` de un repositorio solo se borra cuando falta el repositorio entero y se confirmó.
- **Bloqueo de objetos** (`bloqueo: true`): nunca se borra nada allí, aunque haya retención (la consola no deja elegir las dos cosas y el agente lo rechaza). Igual que en la copia externa: con Object Lock, borrar o no se puede o sale caro.
- Lo que espera para borrarse se ve en el resumen: `por_borrar: { archivos, bytes, desde }` (cuántos, cuánto y cuándo se borra el primero).

## 3c. Más destinos (por rclone, credenciales selladas)

Además de Dropbox y Google Drive, `conectar_nube` admite (agente con `admite: "espejo_destinos"`):

| `tipo` | Parámetros (`parametros`) | Carpeta del destino |
|---|---|---|
| `b2` | `cuenta` (keyID), `clave` | `bucket/ruta` |
| `s3` | `proveedor` (Other, AWS, Wasabi, Minio, Cloudflare…), `endpoint` (https), `region`, `id_clave`, `clave` | `bucket/ruta` |
| `sftp` | `host`, `puerto`, `usuario`, `contrasena`, `clave_host` (la clave pública del servidor, obligatoria) | ruta en el servidor |
| `smb` | `host`, `puerto`, `usuario`, `contrasena`, `dominio` | `recurso/ruta` |
| `webdav` | `url` (https), `proveedor` (other, nextcloud, owncloud), `usuario`, `contrasena` | ruta |

- La orden va **sellada para el equipo** (como la de Dropbox): el servidor nunca ve las claves. El agente las guarda solo en su carpeta privada, protegidas (DPAPI en Windows), en el mismo `nubes.bin`.
- A rclone le llegan por variables de entorno (`RCLONE_CONFIG_RNUBE_*`), nunca en la línea de órdenes ni en un `rclone.conf`. Las contraseñas de SFTP, SMB y WebDAV, rclone las quiere «ofuscadas»: se ofuscan con `rclone obscure -` (por la entrada estándar) al conectar.
- **SFTP con la clave del servidor fijada**: sin ella rclone no comprueba con quién habla. Se escribe un `known_hosts` temporal en la carpeta privada en cada vuelta y se borra al terminar.
- Al conectar se prueba (`rclone lsjson --max-depth 1` en la raíz o en la carpeta de prueba): si no entra, la orden falla con el motivo y no se guarda nada.
- **Carpetas de red**: el espejo a una carpeta UNC (`\\nas\…`) sigue prohibido (SYSTEM no sigue rutas de red, v1.10). Un NAS se usa como `smb` (o SFTP/WebDAV), con su propio usuario.
- **Varios discos del almacén**: ya se podía (varios destinos «carpeta»); la consola los enseña cada uno con su unidad («Disco E:»).
- Dropbox, Drive, SMB, SFTP y WebDAV no son inmutables: la consola lo dice al elegirlos. B2 y S3 pueden serlo con Object Lock (marcar `bloqueo`).

## 3e. Restaurar desde el espejo

Si el almacén se pierde, cada repositorio está entero en el espejo, en `<destino>/<usuario>/<repo>`, con la **misma contraseña** (la del kit de recuperación del equipo dueño). La consola tiene un paso guiado, «Restaurar desde el espejo…», en la ficha del almacén y en la ayuda:

1. Elegir el destino del espejo y el repositorio (de la lista de `guarda_copias.repositorios`, o escrito).
2. La consola dice **dónde está** ese repositorio en ese destino, en la forma que entiende el equipo que lo va a abrir: carpeta (`E:\Espejo\ana\contabilidad`, conectando el disco a ese equipo), `b2:bucket/ruta/ana/contabilidad`, `s3:https://endpoint/bucket/ruta/ana/contabilidad`, `sftp:usuario@host:/ruta/ana/contabilidad`. En Dropbox, Drive, SMB y WebDAV, primero se descarga la carpeta a un disco (el equipo no abre esas nubes directamente).
3. Abre «Restaurar en otro equipo» / «Usar un repositorio que ya existe» con esa dirección: se pide la contraseña del kit, se abre en el equipo elegido y desde ahí se restaura como siempre.

La prueba `restaurar_desde_el_espejo` (Rust) hace una copia de verdad a un repositorio en un «almacén» (carpeta), el espejo a otra carpeta y a un remoto `local` de rclone, borra el almacén y restaura desde cada espejo con la contraseña, comparando los archivos.

## Qué reduce la protección (espera)

Además de quitar un destino o el espejo entero (como antes), espera la orden que:

- quita repositorios de la selección de un destino, o pasa un destino de «todos» a una selección;
- pone o acorta la retención de un destino;
- quita el bloqueo de un destino;
- confirma el freno (`espejo_freno`).

Cambiar el horario, `tras_copia`, `verificar_pct` o el límite no espera.

## Hacia la tarea 7

El espejo por copia de 7d («Espejo: el mismo repositorio en otro destino, con o sin la retención del original») es este mismo motor con alcance de un solo repositorio:

- **con retención** = `retencion_dias` (con su freno); **sin retención** = sin él;
- «después de la anterior» = el disparo por `snapshots/` de §3a, limitado a ese repositorio;
- la verificación, la reparación y los destinos por rclone son los mismos.

Lo que 7 añade es dónde se configura (en la copia, como un paso de su cadena) y los destinos con nombre (7a). Los destinos del espejo de hoy (con `repos` de un solo elemento) se podrán pasar a pasos de una cadena sin mover archivos (7e).
