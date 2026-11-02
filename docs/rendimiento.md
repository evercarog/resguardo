# Rendimiento con muchos equipos (16q y 16x)

Cuánto le cuesta a la consola un cliente grande (200 equipos, 600 repositorios) y qué se cambió, siempre con su medida antes y después (plan 0.8.3, puntos 16q y 16x de [plan-mejoras.md](plan-mejoras.md)). Lo que pide una consola en reposo y cuánto aguanta el servidor está en [capacidad.md](capacidad.md).

Decisión del responsable (16x, 2026-10-22): **nada drástico; ajustes seguros, poco a poco y medidos**.

## Cómo se mide

**El simulador grande.** `MOCK_EQUIPOS=200` crea en el simulador (`consola/src/mock/grande.ts`) el cliente inventado «Distribuidora Gran Escala»: 8 almacenes (uno cada 25 equipos), cada uno con su espejo a una nube, y 192 puestos y servidores con 3 repositorios (uno de cada 8, 4): dos en el almacén de su sede y uno en la nube, **600 repositorios** en total. Unos pocos sin conexión, con la última copia fallida o atrasados. Con la misma semilla (`MOCK_SEMILLA`, 83 por defecto) salen siempre los mismos nombres, estados y tamaños. Cada equipo tiene su último informe con 60 días de versiones y vueltas, como los de verdad (el agente los manda de hasta 200 KiB: `informe_v2.rs`); en el simulador salen de ~63 KB por equipo. «Todos los clientes» (`GET /api/panel`) se resume y se acota igual que en el servidor (60 versiones y 60 vueltas de 15 días por repositorio, 4 MiB de informes en total). `MOCK_LATENCIA=0` quita la espera inventada del simulador (600–1500 ms por petición).

**La medida.** `consola/scripts/medir-rendimiento.ts` abre la consola compilada en un Edge sin ventana (protocolo de depuración) contra el simulador servido con `vite preview --mode mock` (el mismo código que lleva el servidor):

```sh
cd consola && npm run build
MOCK_EQUIPOS=200 MOCK_LATENCIA=0 npx vite preview --mode mock --port 4180 --strictPort
npx tsx scripts/medir-rendimiento.ts        # en otra terminal; SALIDA=archivo.json lo guarda
```

Mide, en Estado, Equipos, Copias y «Todos los clientes»: la **carga** completa de la página (hasta que no queda ningún esqueleto ni petición en vuelo durante 1 s, menos ese segundo), lo que pide a la API (peticiones y bytes), la memoria de JavaScript, los elementos de la página y el tiempo de JavaScript, de colocar y de estilos del navegador; la **navegación** dentro de la consola (pulsar el menú) y pasar Equipos a «Mapa» y volver; y el **reposo**: lo que pide sola cada página en 60 s con la pestaña a la vista.

**Dónde.** Windows 11, AMD Ryzen 7 5700G (16 hilos), 31 GB, el simulador y el navegador en el mismo equipo, **con otras dos ramas compilando a la vez parte del tiempo**: los tiempos varían bastante de una vuelta a otra (p. ej. Estado «con caché» salió en 26 s una vez y en 13 s otra con el mismo código); los bytes y las peticiones no. El simulador no tiene canal en vivo: los refrescos son los de una consola sin él (con él, cada 60 s como mucho más lo que avise el canal; ver [capacidad.md](capacidad.md)). En el simulador los informes no cambian durante la medida: en reposo, los «informes nuevos» son los que habría en una oficina sin copias en ese minuto.

## Antes (rama `ia/083-pulido`, 2026-11-02)

Carga completa, sin caché del navegador:

| Página | Tiempo | Peticiones a la API | Bytes de la API | Elementos | JavaScript | Memoria JS |
|---|---|---|---|---|---|---|
| Estado | 8,8 s | 18 | 24,9 MB | 199 765 | 7,4 s | 301 MB |
| Equipos | 2,3 s | 14 | 24,9 MB | 51 661 | 1,5 s | 128 MB |
| Copias | 6,1 s | 14 | 24,9 MB | 116 969 | 6,1 s | 246 MB |
| Todos los clientes | 2,5 s | 5 | 5,0 MB | 30 570 | 1,8 s | 65 MB |

Lo más pesado de cada página: `GET …/informes` (**12,1 MB**, el último informe de los 200 equipos) **dos veces** (Estado, Equipos y Copias lo pedían al entrar con la lista de equipos vacía y otra vez al llegar la lista), `GET …/resumen` (663 KB) y, en «Todos los clientes», `GET /api/panel` (5,0 MB).

Dentro de la consola: Estado → Equipos 2,5 s; Equipos a «Mapa» 1,9 s y volver 0,9 s; → Copias 4,3 s; → Estado 9,9 s (y 536 000 elementos entre lo nuevo y lo que aún no se ha liberado).

**En reposo, 60 s con la pestaña a la vista** (sin canal en vivo):

| Página | Peticiones | Bytes | Lo que más pesa |
|---|---|---|---|
| Estado | 24 | **51 MB** | `…/informes` cada 15 s (4 × 12,1 MB) y `…/resumen` (4 × 663 KB) |
| Equipos | 27 | 51 MB | lo mismo |
| Copias | 21 | 51 MB | lo mismo |
| Todos los clientes | 22 | 10 MB | `/api/panel` cada 30 s (2 × 5 MB) y `/api/panel/progreso` cada 3 s |

**La consola compilada**: 486 archivos de texto (JavaScript, CSS, HTML) con **3,55 MB**; en gzip serían 1,25 MB y en brotli 1,10 MB (−69 %). El servidor los mandaba sin comprimir (`crates/servidor/src/consola_integrada.rs`). Más 241 KB de fuentes e iconos, que ya van comprimidos.

## Cambios, uno a uno

### 0. La consola, comprimida al compilarla (16x, paso 0)

`npm run build` deja junto a cada archivo de texto de `consola/build` su `.br` (brotli, calidad 11) y su `.gz` (gzip 9) si sale más pequeño (`consola/scripts/comprimir.mjs`). El servidor los lleva dentro y manda el que acepte el navegador (`Accept-Encoding`: brotli primero, luego gzip, sin los de `q=0`), con `Content-Encoding` y `Vary: accept-encoding`; **no comprime nada al servir**. Cada archivo lleva su `ETag` (de la huella que guarda la compilación, distinta por codificación): `index.html`, que va sin caché, vuelve con un 304 si no cambió. Lo de `_app/immutable/` se sigue guardando para siempre. Las cabeceras de seguridad (CSP, `nosniff`, `frame-ancestors`…) son las mismas. Las comprimidas no se piden por su nombre (404). Con `--consola DIR` (para probar), lo mismo con los `.br` y `.gz` que haya en la carpeta. El programa del servidor crece con las dos copias comprimidas (~2,3 MB).

### 7a. Los informes, una vez por cliente y luego solo los nuevos (16x, paso 7)

- `cargarInformes` (`consola/src/lib/informes.svelte.ts`) pide todos los informes **una vez por cliente** (antes, cada vez que cambiaba la lista de equipos: dos veces al entrar).
- Los refrescos piden **solo los que llegaron después** del más reciente que ya hay: `GET …/informes?desde=<fecha>` (contrato en [api-servidor.md](api-servidor.md), «Cambios»). El servidor no lee ni manda los demás. Un servidor anterior no mira `desde` y los manda todos: la consola los junta igual.
- Al volver a una pantalla del cliente pasados 30 s, se piden los nuevos (antes se volvían a pedir todos si cambiaba la lista).

Medido (la misma compilación salvo esto):

| Página | Antes: bytes de la API al cargar | Después | Tiempo antes → después |
|---|---|---|---|
| Estado | 24,9 MB | 12,8 MB | 8,8 s → 6,3 s |
| Equipos | 24,9 MB | 12,8 MB | 2,3 s → 1,2 s |
| Copias | 24,9 MB | 12,8 MB | 6,1 s → 3,8 s (JavaScript 6,1 s → 2,9 s: la página se calculaba dos veces) |

En reposo, 60 s: **51 MB → 7,0 MB** en Estado, Equipos y Copias (lo que queda: `…/resumen`, 4 × 663 KB, y los informes que el simulador daba como nuevos por llegar en el mismo segundo; corregido después en el simulador, cada uno en su segundo). «Todos los clientes» no cambia con esto (10 MB por minuto).

### Estado: las primeras 24 tarjetas (16q, «cargas parciales»)

Estado pintaba una tarjeta por equipo y otra por repositorio: con 200 y 600, ~190 000 elementos y 4–5 s de JavaScript solo en eso. Ahora enseña las primeras 24 de cada lista (lo que falla va primero, como antes) y «Ver los N restantes» (o la lista en Equipos). Con menos de 25 no cambia nada.

| Estado (carga sin caché) | Antes de esto | Después |
|---|---|---|
| Tiempo | 6,3 s | 4,2 s |
| Elementos | 188 478 | 41 794 |
| JavaScript | 4,6 s | 2,9 s |
| Memoria JS | 202 MB | 102 MB |
| Copias → Estado (dentro de la consola) | 9,0 s | 2,9 s |

En reposo (60 s), con los informes del simulador cada uno en su segundo: **3,1 MB** por minuto en Estado, Equipos y Copias (casi todo `…/resumen`, 4 × 663 KB).

### 7b. Las respuestas grandes de lectura, en gzip (16x, paso 7)

Lo que queda pesado se manda en gzip si el navegador lo acepta (`crates/servidor/src/comprimir.rs`): **solo** `GET /api/panel`, `…/informes`, `…/resumen`, `…/equipos/{e}/informes`, `…/equipos/{e}/historial` y `…/restauraciones`, con estado 200, JSON y más de 32 KiB. Son rutas de solo lectura sin secretos (ni contraseñas ni claves: las llaves públicas y las etiquetas de los equipos no sirven sin la clave de administración), así que comprimirlas no abre la puerta a adivinar un secreto por el tamaño (BREACH); el resto de la API sigue sin comprimir. Con el deflate de `miniz_oxide`, que ya estaba en el árbol (sin dependencias nuevas), nivel 3, fuera del hilo de la red.

Medido con las respuestas del simulador (compilación *release*, el mismo equipo):

| Respuesta | Sin comprimir | En gzip | Tiempo de comprimir |
|---|---|---|---|
| `…/informes` (200 equipos) | 12,1 MB | 1,66 MB (13,7 %) | 101 ms |
| `/api/panel` | 5,0 MB | 0,83 MB (16,5 %) | 46 ms |
| `…/resumen` | 663 KB | 96 KB (14,4 %) | 6 ms |

Con esto, lo que baja Estado al cargarlo pasa de 12,8 MB a ~1,8 MB, y lo que pide en reposo de ~3,1 MB a ~0,45 MB por minuto; «Todos los clientes» en reposo, de 10 MB a ~1,7 MB por minuto. (Calculado con los tamaños de arriba: el simulador no comprime, así que esta parte no sale en la medida del navegador.) A cambio, unos 100 ms de procesador del servidor cada vez que alguien abre un cliente de 200 equipos, y ~50 ms cada 30 s por pestaña de «Todos los clientes».
