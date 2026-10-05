# Estabilidad: horas de funcionamiento con averías

Qué pasa cuando Resguardo Server y los agentes funcionan horas seguidas y se les rompen cosas: lo que se midió (memoria, handles, hilos, base de datos, temporales, reconexiones), lo que ve la consola en cada avería, lo que se arregló y los límites que quedan (octubre de 2026, sobre 0.7.20).

## La prueba

`consola/scripts/e2e/resistencia.ts`, con los programas de verdad en 127.0.0.1 (procesos normales, carpetas temporales; nunca toca servicios instalados):

- **Resguardo Server** (compilación *release*), y los equipos le llegan por un **«cable»**: un proxy TCP en la misma prueba que se puede cortar **sin aviso** (las conexiones abiertas dejan de pasar datos y las nuevas no llegan a ningún sitio, como un cable quitado o un router que descarta; no un «conexión rechazada»).
- Un **segundo servidor**: EQUIPO-1 se gestiona desde las dos consolas.
- **ALMACEN** guarda copias (rest-server), con **espejo** a una carpeta y **retención** en el almacén; los equipos le llegan por otro cable.
- **EQUIPO-1, EQUIPO-2, EQUIPO-3** copian **cada 5 minutos** (y algo cambia en sus datos cada minuto); EQUIPO-3 copia a un rest-server aparte **con cuota** (`--max-size`), para llenarlo.
- Órdenes durante toda la prueba: verificación cada 10 min, retención en el almacén cada 20, el espejo otra vez cada 20, «Copiar ahora» desde la segunda consola cada 10, una sesión «explorar» y una **descarga por el relé** cada 15.
- **7 «pestañas» de consola**: lo que pide una consola abierta en reposo (~16 por minuto cada una) y su **canal en vivo**, que se reabre solo si se cierra.
- Correo de avisos a un buzón SMTP de mentira (lo que le llega a quien administra).

Las averías, una tras otra y luego en bucle (reinicio del servidor, del agente y corte de red cada 15 min) hasta 15 min antes del final, que es de calma: todo tiene que volver solo.

| Avería | Cómo |
|---|---|
| Servidor reiniciado | `taskkill /F` (como un corte de luz) y arrancar: 3 s, 5 s y **3 minutos** parado |
| Red cortada con el servidor | el cable, 60–90 s |
| Un equipo desaparece | red cortada y el agente apagado sin cerrar nada |
| Agente reiniciado | matado y arrancado; también **en mitad de una copia** y **con una orden larga** (restaurar) en marcha |
| Almacén apagado | 4 min, con su rest-server |
| Red cortada con el almacén | 2 min, con copias en marcha |
| Destino lleno | el rest-server con cuota, justo con lo que ya ocupa (`507 Insufficient Storage`, lo mismo que un disco lleno) y luego con sitio otra vez |
| Archivo dañado | 64 bytes cambiados en un archivo de datos del repositorio de EQUIPO-1 en el almacén, y una verificación del 100 % |
| Orden y sesión en vuelo | una orden mandada y una sesión abierta justo antes de reiniciar el servidor |

Cada minuto se mide: memoria privada, handles e hilos de cada proceso; tamaño de la base de datos y su WAL; archivos temporales de cada agente (con su propio `TEMP`), restic y rest-server vivos (para ver huérfanos), aperturas del canal, peticiones de las pestañas; y se apunta lo que enseña la consola (avisos, correos, estado de cada equipo). Cada 5 min, las órdenes sin terminar desde hace más de 20 min y las tareas «en marcha» más de 30.

Para repetirla:

```
CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS=true cargo build --release -p resguardo-servidor --bin resguardo-server \
    -p resguardo-agente --bin resguardo-agente --target-dir src-tauri/target-resistencia
cd consola && MINUTOS=135 npx tsx scripts/e2e/resistencia.ts
```

(*Release* con `debug_assertions`: la carpeta del agente en `RESGUARDO_AGENT_DIR` y las órdenes que reducen la protección sin su hora de espera solo existen así.) Deja en la carpeta de la prueba `diario.log` (lo que pasó y lo que vio la consola), `muestras.json` (las medidas de cada minuto), `final.json` y los registros.

## Lo medido

Dos vueltas en Windows 11 (16 núcleos, el mismo equipo para todo, con las pruebas de `cargo test` y `npm run e2e` corriendo a la vez parte del tiempo): la primera, de **2 h** (hasta el minuto 120 de 150: la cortó el límite de la herramienta que la lanzaba), con los primeros arreglos; la segunda, de **2 h 16 min** entera, con casi todos (después llegaron, con sus pruebas, el aviso de la retención del almacén, los mensajes más cortos y la caché del panel). En la segunda: 74 copias (27, 26 y 21; las que faltan en EQUIPO-3 son las 5 del destino lleno), 13 verificaciones pedidas (más las automáticas), 6 retenciones del almacén, 7 espejos (296 MB), 7 sesiones con su descarga por el relé, 6 reinicios del servidor, 6 de agentes (uno copiando y otro con una orden larga), 4 cortes de red (en uno, un equipo desaparece) y el almacén apagado y sin red.

**Memoria privada, handles e hilos** (segunda vuelta, al empezar → al terminar):

| Proceso | Memoria privada | Handles | Hilos | CPU en total |
|---|---|---|---|---|
| Servidor 1 (6 reinicios) | 7,7 → 15,2 MB | 148 → 165 | 21 → 24 | — |
| Servidor 2 (sin reinicios, 1 equipo) | 5,9 → 7,8 MB | 132 → 137 | 18 → 21 | 3,9 s |
| ALMACEN | 5,3 → 5,9 MB | 227 → 213 | 9 → 9 | 7,9 s |
| EQUIPO-1 (dos consolas) | 5,9 → 7,2 MB | 220 → 205 | 12 → 9 | 7,9 s |
| EQUIPO-2 / EQUIPO-3 | 5,1 → 6,2 / 5,8 → 5,7 MB | 205 → 188 / 205 → 186 | 8 → 8 | — |

- **Agentes**: planos (menos de 1,5 MB en 2 h, ±0,5 MB/h en la segunda mitad; handles e hilos sin crecer).
- **Servidor**: handles e hilos planos. La memoria sube unos MB después de cada arranque (la caché de páginas de SQLite, como mucho 2 MB por conexión, el índice del WAL, las cachés de TLS…); el servidor 2, sin reinicios y con un solo equipo, 1,9 MB en 2 h 16 min. Bajo carga constante (abajo) se queda en 20–21 MB.
- **Base de datos**: `control.db` 156 KB y la del cliente 1,2 MB al final (informes de 4 equipos copiando cada 5 min); el **WAL llega a ~4 MB por base de datos y se queda ahí** (punto de control automático de SQLite).
- **Temporales**: 0 en todo momento (cada agente con su propio `TEMP`); ni restos del espejo (`.tmp-espejo`) ni de descargas (`descarga-…`). Las carpetas del relé del servidor: como mucho 4 (las de la última hora; caducan y se borran).
- **Procesos sueltos**: como mucho 3 restic/rest-server vivos (los dos rest-server y una copia); al final, solo los dos rest-server. Ningún restic huérfano tras matar agentes (también en mitad de una copia).
- **Bloqueos**: matar un agente copiando deja el bloqueo de restic en el almacén; la copia siguiente lo da por caducado y se quita: al final no quedaba ninguno.
- **Canal**: 52 aperturas en total entre los 4 equipos (las de cada avería); las pestañas, 6 reaperturas cada una (los reinicios del servidor), ninguna en la del servidor 2.
- **Peticiones**: las 7 pestañas, 112 por minuto entre todas (16 cada una, lo previsto); solo fallan (sin respuesta) mientras el servidor está parado (2–3 %). Los equipos, unas 1,2 conexiones TCP nuevas por minuto cada uno (con copias cada 5 min, órdenes y reinicios).
- **Registros**: `agent.log` de 9–18 KB por equipo en 2 h (rota a 1 MB); `servidor.log` vacío (ni límites ni errores).
- **Órdenes colgadas al final**: ninguna. **Tareas «en marcha»**: ninguna.

**Volver a «conectado»** (lo que ve la consola, segundos):

| Avería | Antes (0.7.20) | Primera vuelta | Ahora |
|---|---|---|---|
| Servidor parado 3–5 s | 10–12 | 10–12 | 10–12 |
| Servidor parado 3 min | hasta 300 (el canal se volvía a probar a los 5 min) | 16 | 16 |
| Red cortada 60–90 s | hasta 300 | 2 (3 equipos), **301** (1) | 2 (todos; ≤ 60 por diseño) |
| Agente reiniciado | 2 | 2 | 2 |
| Equipo que desaparece sin cerrar | **nunca** (seguía «conectado») | — | «sin conexión» a los 138 s |

**Carga constante sin averías** (`carga.ts` con `FASES=1,1,…` de 5 min: 50 pestañas de 10 cuentas, 788 peticiones por minuto, 1 h, el mismo servidor): el servidor pasa de 19 a 21 MB en la primera media hora y ahí se queda (20–21 MB hasta el final); latencia p50 1–2 ms, p99 3–4 ms; 47 900 peticiones, todas 200; 1 % de un núcleo. Bajo esta carga no crece. En la prueba de resistencia, con más variedad (informes, notificaciones, sesiones, relé), sube unos 3–5 MB en la media hora que hay entre reinicios y no da tiempo a ver dónde se para (ver «Límites»).

## Lo que se encontró y se arregló

| Qué pasaba | Ahora |
|---|---|
| **Una orden mandada justo cuando se cortaba la red se quedaba «entregada» para siempre.** Salía por un canal que ya estaba muerto; al reconectar solo se entregan las pendientes y solo caducaban las pendientes. | Al abrir el canal o consultar, el equipo dice el último número de orden que aceptó: lo entregado después no le llegó y se le vuelve a entregar. Lo entregado sin respuesta pasada su caducidad pasa a «caducada», también con el equipo apagado. |
| **Un equipo que desaparece sin cerrar** (red caída sin aviso, portátil que se duerme) **seguía «conectado» para siempre** en la consola, con su «último contacto» renovado cada 30 s. | El servidor cierra el canal de un equipo que no contesta en 150 s (como ya hacía con las consolas en vivo), y el último contacto solo se anota si contesta. |
| **Tras una caída del servidor de más de 15 s, el equipo tardaba hasta 5 min en volver a «conectado»**; tras un corte de red, uno de cuatro tardó 301 s (su reintento coincidió con el final del corte). | En cuanto una consulta funciona después de un fallo de red, el canal se reabre: ≤ 1 min tras un corte, unos segundos tras reiniciar el servidor. Sin bucles: si el servidor contesta pero el canal no abre (un proxy sin WebSocket), sigue cada 5 min. |
| **Una orden larga (restaurar, descargar, retención del almacén…) cortada por un reinicio del agente se quedaba «en marcha» para siempre**; y si el servidor no estaba al terminar, su resultado se perdía a los 15 min. | Se apuntan en la carpeta privada del agente: al volver, la cortada queda «fallida» con el motivo («Se cortó: el agente o el equipo se reinició mientras se hacía. Vuelve a mandarla») en segundos; un resultado sin mandar se manda cuando vuelve el servidor. |
| **«Orden destructiva pendiente … (puedes cancelarla antes de que se aplique)» seguía abierto en la consola** después de aplicarse, cancelarse o caducar. | Se cierra solo (visto por «Resguardo (ya no está pendiente)»). |
| **Destino lleno**: con el disco del almacén lleno, la copia fallaba con el texto de restic en inglés («unexpected HTTP response (507): 507 Insufficient Storage»); y el «disco lleno» de un Windows en español no se reconocía. | «No queda espacio en el destino de las copias. Libera espacio en su disco o quita versiones antiguas (pestaña «Retención») y vuelve a copiar.» El espejo a una carpeta llena dice qué pasa y qué hacer y no deja el archivo a medias. |
| **Archivo dañado**: «Se encontraron errores en el repositorio. Revisa el registro y ejecuta «restic check» en el servidor». | Qué pasa (las copias nuevas siguen; alguna versión anterior podría no restaurarse entera) y qué hacer (revisar el disco del destino; repararlo con `restic repair packs`). |
| **La retención del almacén que se aplica sola a su hora fallaba sin que nadie se enterara** (con el archivo dañado, al podar): ni aviso ni correo, y el mensaje era el de restic en inglés («decrypting blob … ciphertext verification failed»). | Aviso «Retención del almacén fallida» (`retencion_fallida`, importante) con su correo y «Volvió a funcionar»; el mensaje, «Hay datos dañados en el destino…» con qué hacer. |
| **Mensajes cortados**: el aviso de la verificación con datos dañados salía cortado a media frase («…pero»): el informe lleva el de las verificaciones en 160 caracteres. | Los mensajes de datos dañados y disco lleno caben enteros (una prueba lo comprueba). |
| **El panel guardaba en memoria lo de cada cliente que alguien miró una vez** (hasta 4 MB de informes cada uno); lo caducado solo se quitaba pasados 1000 clientes. | Se quita siempre lo caducado (5 s). |
| El registro del equipo decía «IO error: peer closed connection without sending TLS close_notify: https://docs.rs/…» cada vez que se reiniciaba el servidor. | «El servidor cerró la conexión (se reinició, se actualizó o se cortó la red).» |
| **`agente_v2_con_servidor_real` fallaba en este equipo en «descargar»** («Windows no permite acceder a algún archivo»). Con la carpeta temporal en `C:\Users\…`, la descarga en zip de varios archivos restauraba `version:/` y con ello `C:\Users` y las demás carpetas de arriba con sus permisos: la carpeta temporal quedaba sin permiso de escritura (restic: «failed to restore timestamp … Access is denied») y luego no se podía borrar. Con `TEMP` en otro disco no pasaba. | Como «restaurar»: `version:carpeta` con `--include /nombre` por cada carpeta de origen. El zip lleva lo elegido, sin la ruta entera del equipo. |

## Lo que ve la consola en cada avería

| Avería | La consola | Correo |
|---|---|---|
| Servidor parado (segundos o minutos) | Los equipos, «sin conexión» mientras tanto; de vuelta en 10–16 s tras arrancar. Las pestañas reabren su canal solas. | — |
| Red cortada con el servidor | «Sin conexión» a los ~45–60 s (el equipo cierra y reabre); de vuelta en ≤ 60 s. | — (un corte de minutos no avisa: «sin contacto» es a las 24 h) |
| Equipo que desaparece | «Sin conexión» en ≤ 150 s. | «Sin contacto» a las 24 h. |
| Agente reiniciado | «Sin conexión» unos segundos. Una orden larga en marcha: «fallida — Se cortó…». Una copia a medias: la siguiente copia la sigue (restic). | — |
| Almacén apagado o sin red unos minutos | Nada: restic reintenta y la copia termina al volver (tarda más). Si dura más de lo que reintenta restic (~15 min): «Falló la copia…» con el motivo. | Si falla, «Falló la copia» y luego «Volvió a funcionar». |
| Destino lleno | «Falló la copia «Documentos» en «EQUIPO-3». No queda espacio en el destino…» | «Crítico: Falló la copia…» y, con sitio, «Volvió a funcionar la copia…». |
| Archivo dañado | «Falló la verificación de «Copias de EQUIPO-1»… Hay datos dañados en el destino…»; si la retención del almacén lo encuentra al podar, «Falló la retención de … en el almacén …». | «Crítico: Falló la verificación…»; «Falló la retención…». |
| Orden mandada con el servidor reiniciándose | Termina («hecha») al volver; mandada con la red cortada, se entrega al volver. | — |

## Pruebas que fallaban a veces

- `cargo test --workspace` **5 veces seguidas** (con la prueba de resistencia en marcha a la vez, el equipo cargado): **5 de 5 bien**, 421–702 s cada una. Y otra con `TEMP` en `C:\Users\…\Temp` (donde fallaba la de «descargar»): bien.
- `npm run e2e` **3 veces**: **3 de 3 bien** (525–546 s).
- Al final, con todo el código: `cargo test --workspace` y `npm run e2e` otra vez, bien.
- La única que fallaba aquí era `agente_v2_con_servidor_real` (ver arriba): no era el antivirus ni un bloqueo de archivos, sino los permisos de `C:\Users` restaurados en la carpeta temporal.

## Límites y riesgos que quedan

- **Restic reintenta ~15 min** si el almacén o la nube no contestan: mientras, la copia sale «en marcha» (con su progreso parado). Es lo que se quiere en cortes cortos; en uno largo, el aviso llega ~15 min tarde.
- **Un bloqueo de una copia cortada** (agente matado copiando): restic lo da por caducado si es del mismo equipo y su proceso ya no está, o a los 30 min; mientras tanto, la retención del almacén puede encontrarlo y fallar esa vez (avisa).
- **Los avisos son una bandeja**: «Copia fallida» sigue en «Para revisar» después de «Volvió a funcionar» (correo) hasta que alguien lo marca visto; solo «Orden destructiva pendiente» se cierra sola.
- **Un archivo dañado no se arregla solo**: la verificación lo dice, pero repararlo (`restic repair packs`, `repair snapshots`) es a mano en el destino.
- **Informes**: el servidor guarda los 1000 últimos de cada equipo (con copias cada 5 min, unos 2 días; ~7 KB cada uno: ~7 MB por equipo como mucho); historial y auditoría crecen con el uso (pocos KB al día por equipo).
- El WAL de SQLite llega a ~4 MB por base de datos y se queda ahí (punto de control automático); no crece más.
- La prueba es de **2 h largas**, no de semanas: lo que crece despacio no se ve entero. En concreto, la memoria del servidor 1 subió de 6–11 MB tras cada arranque a 12–15 MB en 30 min (sin reinicios no se midió más de 30 min con averías); con carga constante se para en 20–21 MB, y el servidor 2 sube 0,6 MB/h. Una vuelta de un día entero sin reinicios lo aclararía.
- El disco lleno se simula con la cuota de rest-server (lo mismo que contesta con el disco lleno, 507); el disco del propio equipo lleno, y el de la carpeta del espejo, solo con pruebas unitarias (crear un disco pequeño pide administrador).
