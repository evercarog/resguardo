# La ventana del agente («Resguardo en el equipo»)

El agente (`resguardo-agente.exe`) ya hace las copias como servicio y tiene un
icono en la bandeja. Esto añade, **sin cargar el equipo**, lo que tenía la app
de escritorio que más se echa de menos:

1. una **ventana pequeña** con gráficas en vivo (al estilo de GlassWire) mientras
   copia, restaura, verifica o sube a la copia externa, el espejo o la nube;
2. **avisos bonitos** (notificaciones nativas de Windows) al empezar, terminar,
   fallar o recuperarse;
3. que las dos cosas se **enciendan y apaguen desde la consola y desde el
   propio equipo** (aquí, solo con la clave de administración);
4. un **modo sin consola** completo: el agente se usa solo, con una clave de
   administración local, y se vincula más tarde sin perder nada.

La app de escritorio (Tauri, `src-tauri/`) sigue congelada: no se toca.

## 1. Piezas y quién hace qué

```
Servicio (SYSTEM)                          Sesión del usuario
───────────────────                        ─────────────────────────────────────────
one_tick / --agent-run / --agent-tasks     resguardo-agente --tray   (siempre, ~5 MB)
  state.json, tasks.json (ya existían)       · lee gestionado-bandeja.json (5 s; 1 s con algo en marcha)
  + read_bps, upload_bps, files_per_s        · icono, menú, avisos (WinRT o globo)
hilo_bandeja                                 · «Abrir Resguardo», pulsar el icono o un aviso
  gestionado-bandeja.json  (30 s; 2 s con      → lanza la ventana
    algo en marcha y ventana o «todo»)
  gestionado-ventana.json  (solo si la     resguardo-agente --ventana  (solo mientras está abierta)
    ventana no está apagada)                 · WebView2 del sistema (wry + tao)
ipc_local (tubería con nombre)               · lee los dos archivos cada segundo (si cambian, a la página)
  · hola, ajustes, modo local…               · «Ajustes» y modo local → ipc_local, con la prueba de la clave
registro en memoria (escritorio::en_marcha)
  · restauraciones, espejo, nube
```

- **Ver no da autoridad nueva.** La ventana y la bandeja corren como el usuario
  y solo leen dos archivos de `ProgramData\ResguardoAgente`: sin rutas, sin
  mensajes de restic, sin secretos. «Copiar ahora» sigue usando `solicitudes\`.
- **Lo único privilegiado y nuevo** es `ipc_local` (§4): cambiar los ajustes del
  escritorio y, en modo local, administrar el equipo. Siempre con la clave.
- **Con todo apagado nada cambia**: con `ventana: off` el servicio no escribe
  `gestionado-ventana.json` ni muestrea, y con `avisos` distinto de `todo` no
  escribe más a menudo que antes.

## 2. Ajustes del escritorio (`escritorio`)

En la configuración gestionada (`Configuracion` v1, orden `config`):

```json
"escritorio": { "ventana": "al_trabajar", "avisos": "errores" }
```

| Campo | Valores | Qué hace |
|---|---|---|
| `ventana` | `off` | Sin ventana: la bandeja de siempre. |
| | `siempre_disponible` | «Abrir Resguardo» en el menú; pulsar el icono la abre. No se abre sola. |
| | `al_trabajar` | Además se abre sola al empezar una copia, restauración, verificación o subida (una vez por tarea; cerrada, no vuelve hasta la siguiente). |
| `avisos` | `off` | Ninguno. |
| | `errores` | Al fallar y al recuperarse (lo que hacía `bandeja.avisos: true`). |
| | `todo` | También al empezar y al terminar bien. |

- **Compatibilidad.** Sin `escritorio` (consola anterior) se deduce de `bandeja`
  (`avisos` → `errores` u `off`; `visible` → `siempre_disponible` u `off`). Una
  `config` sin el campo no borra el que haya. La consola nueva manda también
  `bandeja.avisos = (avisos != off)` y solo manda `escritorio` si el resumen trae
  `admite: ["escritorio"]`.
- **Desde la consola**: «Añadir o cambiar copias» → tarjeta «En el equipo»; la ficha
  del equipo enseña lo que tiene (del resumen en claro) y si se cambió allí.
- **Desde el equipo**: «Ajustes» en la ventana pide la clave; el servicio pone
  `escritorio`, `cambiado_en_equipo` y sube la configuración como cualquier
  cambio (`subir_config`). La consola lo ve al descifrarla y en el resumen
  (`escritorio_cambiado_en_equipo`).
- **Carreras con el canal**: un cambio local sube `cambio_local`, que el canal
  trata como «cambiado fuera» (`cambiado_fuera`, una línea).

## 3. Lo que muestra la ventana

460×720 (se puede cambiar, mínimo 380×520), tema claro u oscuro según Windows.

- **Ahora**: cada tarea en marcha (tipo, nombre, fase, %, archivos, bytes,
  «quedan») y las **ondas en vivo** (`ui/componentes/GraficaOndas.svelte`,
  compartida con la consola): áreas translúcidas con degradado y brillo que
  fluyen con el tiempo; **lectura** (azul) y **escritura/subida** (naranja) en
  bytes/s, y **archivos por segundo** (aguamarina) en otra gráfica (otra unidad:
  nunca dos ejes). Leyenda con el valor de ahora y el pico, cruz con los valores
  al pasar el ratón y un resumen para lectores de pantalla. Con poca historia se
  ve de cerca y se va alejando hasta 5 minutos; al terminar, la onda baja a cero.
  Sin nada en marcha, el estado tranquilo (última copia, próxima, «Copiar ahora»).
- **Copias**: cada copia con su resultado, cuándo y la próxima; «Copiar ahora».
- **Historial**: barras de 14 días (correctas, con avisos, fallidas; con icono y
  texto, no solo color) y lo último de cada tarea.
- **Ajustes**: con la clave, la ventana y los avisos; en modo local, todo (§5).

Qué se mide (de verdad, no estimado):

| Tarea | Lectura | Escritura / subida | Archivos/s |
|---|---|---|---|
| Copia | E/S de lectura del proceso de restic | E/S de escritura + «otra» (red) del proceso | restic |
| Restauración | — | bytes restaurados (`restore --json`) | — |
| Verificación | bytes leídos (si la tarea los da) | — | — |
| Copia externa, espejo, nube | — | bytes subidos (si la tarea los da) | — |

La copia manda además `lectura`, `subida` y `archivos_s` en el progreso de la
consola (v1.36): la consola pinta las mismas ondas en la fila de la copia.

### `gestionado-ventana.json`

```json
{ "v": 1, "escrito": "…",
  "serie": [[1759666800, 652000000, 431000000, 7, "copia"]],
  "historial": [{ "dia": "2026-10-05", "ok": 3, "aviso": 0, "fallo": 1 }] }
```

`serie`: `[segundo, lectura B/s, escritura B/s, archivos/s, tipo]`, como mucho
150 puntos y 5 minutos. Lo que está en marcha (`actividades`) y lo último
terminado (`hechas`) van en `gestionado-bandeja.json` (también los usa la
bandeja para avisar). Solo nombres (de la consola o del administrador local) y
cifras.

## 4. `ipc_local`: el único camino privilegiado nuevo

**Transporte.** Windows: tubería `\\.\pipe\ResguardoAgente` del servicio, con
DACL `D:P(A;;GA;;;SY)(A;;GA;;;BA)(A;;GA;;;OW)(A;;0x0012008B;;;IU)`: los usuarios
con sesión interactiva leen y escriben datos pero **no pueden crear instancias**
(sin `FILE_CREATE_PIPE_INSTANCE`), así que nadie puede suplantar la tubería; el
servicio siempre tiene una instancia esperando. `PIPE_REJECT_REMOTE_CLIENTS`. El
cliente comprueba que la tubería es de SYSTEM o de los administradores y abre
con `SECURITY_IDENTIFICATION` (el servidor no puede suplantarlo). Cada conexión,
una petición (una línea JSON, como mucho 64 KiB; respuestas hasta 4 MiB) y como
mucho 4 a la vez; la petición entera tiene que llegar en 10 s (plazo total, no
por lectura) o se cierra. **El servicio sabe quién pide**: tras leer la
petición identifica al cliente con su token (`ImpersonateNamedPipeClient` a
nivel de identificación y `RevertToSelf` enseguida; si no pudiera volver,
aborta): su SID y si es administrador (SYSTEM, o del grupo Administradores
aunque UAC lo deje «solo para denegar»). Linux:
`/run/resguardo-agente/ipc.sock` (`0666` en un directorio de root, que tiene que
ser de root; lo que autoriza es la prueba; el cliente exige que el socket sea de
root; quién pide, por `SO_PEERCRED`; como mucho 4 a la vez). En pruebas
(`RESGUARDO_AGENT_DIR`, solo compilaciones de desarrollo), otro nombre: nunca el
del servicio instalado.

**Prueba de administración**, la de la consola: `prueba = Argon2id(clave_NFC,
sal_equipo)`; el equipo guarda `verificador = SHA-256(prueba)`. La ventana la
calcula en su proceso (el servicio nunca ve la clave) y la guarda en memoria
mientras está desbloqueada (10 minutos sin usarla). Cada petición va atada a un
**reto de un solo uso**:

1. `{"op":"hola"}` → `{ "v":1, "reto", "sal_equipo", "modo": "sin_clave"|"local"|"gestionado"|"web"|"pendiente"|"desvinculado" }`
   (retos: 60 s; cada reto es **de la cuenta que lo pidió** y solo ella lo
   gasta; como mucho 16 vivos por cuenta y 256 en total: otra cuenta que pide
   muchos no echa los de las demás).
2. `{"op":"…","reto":"…","prueba":"<b64>", …}`.

El servicio: gasta el reto (siempre, aunque falle: **sin repetición**), mira el
límite, y compara la prueba en **tiempo constante**. **Límites, por cuenta del
equipo**: 5 fallos seguidos bloquean 1 minuto, cada bloqueo siguiente el doble
(hasta 1 hora); tras un fallo, como mucho un intento por segundo; un acierto lo
reinicia. Un usuario que prueba claves solo se bloquea a sí mismo: el
administrador, con su cuenta, sigue entrando (como mucho 64 cuentas en memoria).
Al registro solo va «clave de administración incorrecta desde la cuenta
S-1-5-21-…» o qué se cambió, nunca la petición.

Lo que cambia el vínculo (`crear_clave`, `ajustes`, `config`,
`crear_repositorio`, `retencion`, `copia_externa`, `cambiar_clave`) se hace
bajo el mismo cerrojo que las órdenes de las consolas, leyendo el vínculo ya
dentro: una orden que llegue a la vez no se pierde (su `seq`, sus `nonce`, una
clave nueva o una consola quitada).

¿Por qué la prueba y no un HMAC? El equipo solo guarda `SHA-256(prueba)`: para
comprobar un HMAC tendría que guardar la prueba. Viaja por una tubería local con
ACL, atada a un reto de un uso: lo mismo que ya viaja (cifrado) en cada orden.

| Op | Cuándo | Qué hace |
|---|---|---|
| `hola` | siempre | reto, sal y modo |
| `crear_clave` | sin vínculo ni consola web, **solo un administrador del equipo** | modo local: verificador y `K_cfg` (de la ventana) |
| `comprobar` | con clave | «Desbloquear» |
| `ajustes` | con clave (no en la consola web) | `escritorio`; si hay consola, se sube |
| `estado_local` | local | repositorios, destinos, configuración, nubes, resumen |
| `crear_repositorio`, `config`, `retencion`, `copia_externa`, `pausar`, `reanudar` | local | lo mismo que las órdenes de la consola (`gestion_v2`) |
| `carpetas`, `explorar` | local | las sesiones `elegir_carpetas` y `explorar` (versiones, listar, buscar, Qué cambió…) |
| `restaurar` | local | junto al original, en su sitio o en otra carpeta (§5) |
| `guarda_copias`, `conectar_nube`, `quitar_nube` | local | Servidor de copias, espejo y nubes |
| `historial`, `kit`, `cambiar_clave`, `vincular` | local | |
| `adoptar_repositorio`, `copiar_historial`, `historial_traido` | local | «Usar uno que ya existe» (o solo probarlo) y «Traer historial» (en segundo plano; `historial_traido { repo }` dice cómo va: `{ estado, mensaje }` o `null`) |

En un equipo gestionado la ventana solo cambia `escritorio`.

## 5. Modo sin consola

Equipo recién instalado, sin vincular: la ventana ofrece **«Usar sin consola»**.

1. Clave de administración (12 caracteres o más, dos veces). La ventana genera
   `sal_equipo` y `sal_cliente` y calcula `verificador` y `K_cfg` como la consola;
   el servicio crea el vínculo en modo local (sin servidor) con claves propias.
   Solo si no hay vínculo ni consola web, y **solo un administrador del
   equipo** (su cuenta, sin «Ejecutar como administrador»: la ventana nunca va
   elevada; el servicio mira el grupo Administradores en su token). Si no, el
   primer usuario sin privilegios que abriera la ventana se quedaría con la
   clave y, con ella, con lo que el servicio hace como SYSTEM. En Linux, root.
   Quien tiene la clave del modo local decide qué se copia (cualquier carpeta)
   y tiene las contraseñas de los repositorios: trátala como la de un
   administrador del equipo.
2. Después, todo con la clave (lo mismo que la consola, con su lógica y sus
   componentes: `EditorHorario`, `EditorRetencion`, `lib/horario`, `lib/ganchos`,
   `lib/verificacion`…):
   - **Dónde**: carpeta o disco del equipo (USB), rest-server (con su
     certificado), S3, B2 o SFTP; contraseña generada y **kit de recuperación**
     para imprimir; **retención** con los preajustes («Programas contables»…) y
     «aplicar ya»; **copia externa** a otro destino; **«Usar uno que ya existe»**
     (p. ej. el de la app de escritorio, con todo su historial) y **«Traer
     historial»** de otro repositorio a uno de aquí, con los formularios de la consola.
   - **Copias**: carpetas con un explorador del equipo (o escribiendo la ruta),
     exclusiones, reglas de horario (horas, cada N minutos, cada N días, un día
     al mes), «solo si hay cambios», «Antes de copiar» (volcado de SQL Server,
     vigilar las copias de una aplicación) y **verificación automática**.
   - **Restaurar**: versiones, recorrerlas, «Qué cambió», restaurar archivos o
     carpetas junto al original, en su sitio (reemplazando o no) o en otra carpeta.
     Restaura el servicio (SYSTEM), así que: «otra carpeta» tiene que existir en
     un disco del equipo, sin enlaces en su camino, sin `..`, nombres que Windows
     recorta (`a.`, `a `) ni flujos (`a:b`), sin `\\?\` ni red, y no ser del
     sistema, de los programas ni de Resguardo, tampoco por su ruta real
     (`C:\PROGRA~1`); lo restaurado va siempre a una carpeta **nueva**
     «Restaurado …» que crea el agente («… (2)» si ya hay algo con ese nombre:
     nunca una unión que alguien dejara antes adivinando la hora) y que, desde
     la ventana, nace solo para SYSTEM, Administradores y quien la pide (nadie
     más puede poner un enlace dentro mientras se escribe). «En su sitio», no si
     la carpeta es ahora un enlace.
   - **Más**: pausar, historial completo, kit, cambiar la clave, «Guardar copias
     de otros» (Servidor de copias) con su espejo a una carpeta o a Dropbox /
     Google Drive (`rclone authorize` lo abre la ventana **como el usuario**, en
     su navegador; el token va al servicio con la clave), y **vincular a una
     consola**.
3. **Vincular** conserva todo: el equipo tiene clave, así que queda pendiente
   del alta (`adopcion`) y la consola lo adopta con la **misma clave** (camino 3
   de api-servidor.md §3.5). Se enseña el código de comprobación.

## 6. Avisos

- **WinRT** con identidad propia: la bandeja registra para el usuario
  `HKCU\Software\Classes\AppUserModelId\Resguardo.Agente` (nombre «Resguardo» y
  un PNG del escudo en `%LOCALAPPDATA%\Resguardo\Agente`), sin instalador ni
  accesos directos. Si Windows no los admite, el globo de siempre.
- **Agrupados**: `Group` por tipo (copias, restauraciones, verificaciones,
  subidas) y `Tag` por tarea: «terminó» reemplaza a «empezó».
- **No molestar**: Windows los guarda en silencio. Con una presentación o un
  juego a pantalla completa (`SHQueryUserNotificationState`) solo pasan los de
  error. Los que no son de error van sin sonido.
- **Pulsar** un aviso abre la ventana (si no está apagada).
- Qué se avisa: `escritorio::avisos` (probado). Lo que ya había terminado al
  arrancar la bandeja no avisa.

## 7. Ligereza (medido en Windows 11, copia de prueba de 31 GB a un disco local)

- **Servicio**: con todo apagado, como antes. Con algo en marcha y la ventana o
  «todo», escribe los dos archivos cada 2 s (unos KB). Sin nada en marcha mira
  cada 2 s si empezó algo (leer tres JSON pequeños).
- **Bandeja**: la misma (~5 MB privados; lee un archivo pequeño cada 5 s).
- **Ventana**: otro proceso que solo existe abierta; al cerrarla se libera todo.
  El proceso de la ventana, ~6 MB; WebView2, el resto: ~210–250 MB privados
  entre sus procesos. CPU: ~0,3 % de un núcleo en reposo y ~2 % con las ondas
  fluyendo (30 fps como mucho, solo con algo en marcha y la ventana a la vista;
  con «reducir movimiento», solo al llegar datos).
- **Ejecutable**: 6,98 MB en release (5,89 MB antes; +~0,47 MB comprimido en el
  instalador): wry, tao, las piezas de WinRT de `windows` (ya estaban en el
  `Cargo.lock` por la app) y la página.
- **Página**: ~41 KB comprimidos lo que se abre siempre; el modo local se carga
  al abrirlo (en total ~90 KB comprimidos). Sin fuentes propias (la del sistema),
  sin red: un protocolo propio (`http://resguardo.localhost/`) con CSP estricta.
- Sin WebView2, «Abrir Resguardo» lo dice en el registro y la bandeja sigue igual.
- Avisos apagados por el usuario (para Resguardo o para todo Windows): se respeta.

## 8. Compilar y probar

- `cd consola && npm run build:ventana` deja la página en `crates/agente/ventana`
  (va en el repositorio; `build.rs` la mete en el ejecutable). `npm run
  check:ventana` la comprueba.
- `RESGUARDO_AGENT_DIR=<carpeta> resguardo-agente --primer-plano` y, en otra
  consola, `resguardo-agente --ventana` (compilación de desarrollo): ventana
  contra una carpeta de pruebas, con otra tubería. `RESGUARDO_VENTANA_TEMA=claro|oscuro`
  fuerza el tema (solo en desarrollo).

## 9. Lo que queda

- Ventana en Linux (WebKitGTK): el `ipc_local` ya funciona allí, falta la ventana.
- En modo local: adoptar un repositorio que ya existe y traer su historial
  (las órdenes existen; falta la pantalla), los equipos cliente del Servidor de
  copias (sus contraseñas se sellan para una consola) y la etiqueta y el
  nombre del equipo.
- Medir la lectura y la subida de verificaciones, copias externas y el espejo
  por E/S del proceso (hoy, por los bytes que dicen las tareas).
