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
4. un **modo sin consola**: el agente se puede usar solo, con una clave de
   administración local, y vincularse más tarde sin perder nada.

La app de escritorio (Tauri, `src-tauri/`) sigue congelada: no se toca.

## 1. Piezas y quién hace qué

```
Servicio (SYSTEM)                        Sesión del usuario
──────────────────                       ─────────────────────────────────────
one_tick / --agent-run / --agent-tasks   resguardo-agente --tray  (siempre, ~3 MB)
  state.json, tasks.json (ya existían)     · lee gestionado-bandeja.json cada 5 s
hilo_bandeja                               · icono, menú, avisos (WinRT o globo)
  gestionado-bandeja.json  (cada 30 s)     · lanza la ventana (al pulsar o al empezar)
  gestionado-ventana.json  (cada 2 s       
     solo si hay algo en marcha y la     resguardo-agente --ventana  (solo abierta)
     ventana no está apagada)              · WebView2 del sistema (wry + tao)
ipc_local (tubería con nombre)             · lee gestionado-ventana.json cada 1 s
  · `hola`, `ajustes`, modo local…         · «Ajustes» y modo local → ipc_local
```

- **Ver no da autoridad nueva.** La ventana y la bandeja corren como el usuario
  y solo leen dos archivos que escribe el servicio en `ProgramData\ResguardoAgente`,
  sin rutas, sin mensajes del agente y sin secretos (los mismos criterios que
  `gestionado-bandeja.json`). «Copiar ahora» sigue usando `solicitudes\`.
- **Lo único privilegiado y nuevo** es `ipc_local`: cambiar los ajustes del
  escritorio y, en modo local, configurar las copias. Siempre con la clave de
  administración (§4).
- **Sin ventana, nada cambia**: si la consola la apaga (`ventana: off`), el
  servicio no escribe `gestionado-ventana.json` ni muestrea nada; la bandeja es
  la de siempre.

## 2. Ajustes del escritorio (`escritorio`)

En la configuración gestionada (`Configuracion` v1, orden `config`, clave de
administración):

```json
"escritorio": { "ventana": "al_trabajar", "avisos": "errores" }
```

| Campo | Valores | Qué hace |
|---|---|---|
| `ventana` | `off` | Sin ventana: la bandeja de siempre. |
| | `siempre_disponible` | «Abrir Resguardo» en el menú del icono (y al pulsarlo). No se abre sola. |
| | `al_trabajar` | Lo anterior y, además, se abre sola al empezar una copia, restauración, verificación o subida (una vez por tarea; si el usuario la cierra, no vuelve hasta la siguiente). |
| `avisos` | `off` | Ninguno. |
| | `errores` | Al fallar y al recuperarse (lo que hacía `bandeja.avisos: true`). |
| | `todo` | También al empezar y al terminar bien. |

**Compatibilidad.** `bandeja: { visible, avisos }` sigue igual y manda sobre el
icono. Sin `escritorio` (consola anterior), se deduce: `avisos` = `errores` si
`bandeja.avisos`, si no `off`; `ventana` = `siempre_disponible` si el icono es
visible, si no `off`. Una `config` **sin** el campo `escritorio` no borra el que
haya (igual que `verificaciones`): una consola anterior no deshace lo que se
cambió en el equipo. La consola nueva manda los dos (`bandeja.avisos` =
`avisos != off`) para que un agente anterior haga lo que puede.

**Cambiar en el equipo.** «Ajustes» en la ventana pide la clave de
administración; el servicio comprueba la prueba (§4), aplica `escritorio` sobre
la configuración que tiene, marca `cambiado_en_equipo` (fecha) y la sube
cifrada a la consola como cualquier cambio de configuración (`subir_config`).
La consola, al descifrarla, ve el valor nuevo y la marca («cambiado en el
equipo»); el resumen en claro lleva también `escritorio` (no es secreto) para
la tarjeta «En el equipo» sin descifrar nada.

**Carreras con el canal.** El canal con el servidor tiene su copia del vínculo
en memoria y la guarda tras cada informe. Un cambio local sube un contador
(`cambio_local`) que el canal mira como «cambiado fuera» (`cambiado_fuera`, una
línea): cierra y vuelve a abrir con lo del disco, igual que tras `vincular`.

## 3. Lo que muestra la ventana

Tamaño fijo 420×640 (se puede redimensionar), sin marco del sistema propio: la
barra de título de Windows de siempre, tema claro u oscuro según Windows.

- **Ahora**: la tarea en marcha (tipo, nombre, fase, %, archivos, bytes, ritmo y
  «quedan»), con una **gráfica de ritmo en vivo** (los últimos 5 minutos, área
  con degradado y brillo, el color según el tipo: copia en el acento, restaurar
  en azul, verificar en violeta, subir en ámbar). Si no hay nada en marcha: el
  estado tranquilo del icono («Tus archivos están protegidos») y la próxima copia.
- **Copias**: cada copia con su último resultado, cuándo y la próxima;
  «Copiar ahora» (la solicitud de siempre).
- **Historial**: barras de los últimos 14 días (copias correctas, con avisos,
  fallidas) y la lista de lo último (sin rutas ni mensajes).
- **Ajustes** (con la clave): ventana y avisos; en modo local, las copias.

### `gestionado-ventana.json`

```json
{
  "v": 1,
  "escrito": "2026-10-05T14:20:02+02:00",
  "actividades": [{
    "id": "copia:r1#k1", "tipo": "copia", "nombre": "Documentos", "fase": "subiendo",
    "porcentaje": 0.42, "archivos": 120, "archivos_total": 300,
    "bytes": 4000, "bytes_total": 10000, "velocidad": 2000, "quedan_s": 90,
    "empezo": "2026-10-05T14:15:00+02:00"
  }],
  "serie": [[1759666800, 1850000, "copia"], [1759666802, 2010000, "copia"]],
  "hechas": [{ "clave": "copia:r1#k1", "tipo": "copia", "nombre": "Documentos",
               "resultado": "ok", "cuando": "…", "bytes": 4000 }],
  "historial": [{ "dia": "2026-10-05", "ok": 3, "aviso": 0, "fallo": 1 }]
}
```

- `tipo`: `copia`, `restauracion`, `verificacion`, `copia_externa`, `espejo`, `nube`.
- `serie`: como mucho **150 puntos** (5 minutos a uno cada 2 s); se descartan
  los de más de 5 minutos. Ritmo en bytes/s: el de restic en las copias; en el
  resto, la diferencia de bytes entre dos muestras (si la tarea los da).
- `hechas`: lo último terminado de cada tipo (para los avisos de «terminó»).
- Sin rutas, sin mensajes de restic, sin nombres de archivo: solo los nombres
  que da la consola (o el usuario en modo local) y cifras.

## 4. `ipc_local`: el único camino privilegiado nuevo

**Transporte.** Windows: tubería con nombre `\\.\pipe\ResguardoAgente` creada
por el servicio con una DACL explícita (`D:P(A;;GA;;;SY)(A;;GA;;;BA)(A;;GRGW;;;IU)`:
SYSTEM y administradores, y lectura/escritura para los usuarios con sesión
interactiva; nadie por red: `PIPE_REJECT_REMOTE_CLIENTS`), una instancia cada
vez. Linux: `/run/resguardo-agente/ipc.sock`, `0666` en un directorio `0755`
de root (cualquier usuario local puede *hablar*; lo que autoriza es la prueba,
igual que en Windows). Mensajes: una línea JSON por petición, como mucho 64 KiB.

**Prueba de administración.** La misma que la consola: `prueba =
Argon2id(clave_NFC, sal_equipo)` y el equipo guarda `verificador =
SHA-256(prueba)`. La ventana calcula la prueba (Argon2id, 64 MiB, en su propio
proceso: el servicio nunca ve la clave) y la manda **atada a un reto de un
solo uso**:

1. `{"op":"hola"}` → `{"v":1,"reto":"<32 bytes b64>","sal_equipo":"…","modo":"gestionado"|"local"|"sin_clave"|"web"}`.
   El reto vale para **una** petición y caduca a los 60 s.
2. `{"op":"ajustes","reto":"…","prueba":"<b64>","escritorio":{…}}`.

El servicio comprueba, por este orden: que el reto exista, no esté usado ni
caducado (se consume aunque falle: **sin repetición**); el bloqueo por
intentos; y la prueba contra el verificador en **tiempo constante**
(`derivaciones::comprueba_prueba`). **Límites**: 5 fallos seguidos bloquean 1
minuto, y cada bloqueo siguiente dura el doble (hasta 1 hora); un acierto lo
reinicia. Hay además como mucho un intento por segundo (los Argon2id caros los
hace el cliente, pero un atacante podría probar verificadores a mano). Nada de
la petición se escribe en el registro: solo «Ajustes cambiados en el equipo» o
«Clave de administración incorrecta en el equipo (n)».

¿Por qué mandar la prueba y no un HMAC? El equipo solo guarda `SHA-256(prueba)`:
para comprobar un HMAC tendría que guardar la prueba (equivalente a la clave
para las órdenes). La prueba viaja por una tubería local con ACL, dentro de una
conexión que no sale del equipo, y atada a un reto de un uso; es lo mismo que ya
viaja (cifrado) en cada orden de la consola.

**Operaciones** (todas menos `hola` con reto y prueba):

| Op | Cuándo | Qué hace |
|---|---|---|
| `hola` | siempre | reto, sal y modo |
| `ajustes` | con clave | cambia `escritorio`; si está vinculado, lo sube |
| `crear_clave` | **solo sin clave y sin vincular** | modo local: guarda verificador y `K_cfg` (§5) |
| `estado_local` | modo local | repositorios, destinos y copias (sin contraseñas) |
| `crear_repositorio` | modo local | `gestion_v2::crear_repositorio` (carpeta, disco USB o rest-server) |
| `config` | modo local | `gestion_v2::aplicar_config` (copias, carpetas, exclusiones, horario) |
| `carpetas` | modo local | elegir carpetas del equipo (como la sesión `carpetas`) |
| `versiones`, `listar` | modo local | explorar un repositorio |
| `restaurar` | modo local | `sesiones_v2::restaurar`, siempre «junto al original» |

En un equipo gestionado, la ventana solo cambia `escritorio`: las copias las
decide la consola.

## 5. Modo sin consola

Al abrir la ventana en un equipo sin vincular y sin clave, se ofrece **«Usar
sin consola»**:

1. El usuario elige una clave de administración (12 caracteres o más, dos veces).
   La ventana genera `sal_equipo` y `sal_cliente` (16 bytes aleatorios cada una)
   y calcula `verificador` y `K_cfg` como la consola.
2. `crear_clave` → el servicio crea el vínculo en **modo local** (`modo:
   "local"`, sin servidor), con claves propias del equipo (caja y firma) y esos
   valores. Solo se acepta si no hay vínculo, ni emparejamiento web, ni clave:
   quien llega primero a un equipo recién instalado la pone (lo mismo que el
   código de vinculación). Después, cambiarla pide la clave actual.
3. Con la clave: un repositorio (carpeta del equipo, disco USB o rest-server,
   con su contraseña, que se recuerda que hay que guardar), copias con
   carpetas, exclusiones y horario (el mismo `Configuracion` v1 que la
   consola: mismo validador, mismo motor), «Copiar ahora» y restaurar un
   archivo desde un explorador sencillo.
4. **«Vincular a una consola»** más tarde: `resguardo-agente vincular CÓDIGO`
   (o el instalador «listo») ya conserva todo si el equipo tiene clave: queda
   pendiente del alta (`adopcion`) y la consola lo adopta con la **misma clave**
   (y toma su `K_cfg`). Es el camino 3 de api-servidor.md §3.5, sin nada nuevo.

## 6. Avisos

- **WinRT** (`ToastNotificationManager`) con identidad propia: la bandeja
  registra, para el usuario (`HKCU\Software\Classes\AppUserModelId\Resguardo.Agente`),
  el nombre «Resguardo» y el icono (un PNG pintado con el mismo escudo). Sin
  instalador ni accesos directos nuevos. Si falla (Windows antiguo, política),
  el globo de siempre.
- **Agrupados**: `Group` por tipo (`copias`, `restauraciones`…) y `Tag` por
  tarea: «terminó» reemplaza a «empezó» de la misma tarea en el centro de
  actividades; nunca más de uno por tarea y vuelta.
- **No molestar**: Windows ya los guarda en silencio con «No molestar» /
  Asistente de concentración. Además, si `SHQueryUserNotificationState` dice que
  hay una presentación o un juego a pantalla completa, solo pasan los de error.
- **Pulsar** un aviso abre la ventana (si no está apagada).
- Qué se avisa (`escritorio::avisos`, probado): ver la tabla del §2. Fallar y
  recuperarse comparan con la ejecución anterior de la misma tarea, como antes.

## 7. Ligereza

- La bandeja no cambia de peso: lee un archivo pequeño cada 5 s.
- La ventana es **otro proceso** (`--ventana`) que solo existe mientras está
  abierta: al cerrarla, toda su memoria (y la de WebView2) se libera. Se pinta
  con `requestAnimationFrame` solo si hay datos nuevos y nada si está minimizada.
- El servicio muestrea cada 2 s **solo** con algo en marcha y la ventana no
  apagada; si no, cada 30 s como antes. Muestrear es leer `state.json` y
  `tasks.json`, que ya se escriben.
- La interfaz es una página sola (Svelte, sin Inter: la fuente del sistema)
  metida en el ejecutable; sin servidor web, sin puertos.
- WebView2 es el del sistema (Windows 10/11 lo traen). Sin WebView2, «Abrir
  Resguardo» explica que falta y la bandeja sigue igual.
- Linux: sin ventana por ahora (los equipos Linux suelen ser servidores); el
  `ipc_local` y el modo local funcionan igual desde la línea de órdenes.

## 8. Lo que queda fuera (por ahora)

- Editor local completo (retención, copia externa, verificación automática,
  ganchos): en modo local se usan los valores por defecto del motor.
- Ventana en Linux (WebKitGTK).
- Cambiar la clave de administración desde la ventana (se hace con la consola
  o `resguardo-agente` por línea de órdenes).
