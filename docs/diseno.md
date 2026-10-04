# Sistema de diseño de Resguardo

Este documento es la referencia común de la app de escritorio y de Resguardo Web, para que las dos parezcan el mismo producto. Si cambias algo aquí, cámbialo en las dos.

El estilo es **sobrio y moderno**, en la línea de Linear, Vercel o Raycast:

- mucho aire, tipografía nítida y pocos bordes;
- colores apagados: **el color se reserva para el estado**;
- profesional y tranquilo.

Los modos claro y oscuro están igual de cuidados. Por defecto se sigue el del sistema, y el usuario puede elegir otro en *Apariencia*.

Los nombres de las variables CSS de este documento son los canónicos. La app mantiene alias de los antiguos (`--text`, `--success`, `--danger`…) solo mientras dura la migración.

---

## 1. Tipografía

**Fuente:** Inter variable, alojada con el producto. Se instala con `@fontsource-variable/inter`, sin CDN, para que funcione sin conexión en Tauri.

```css
@import "@fontsource-variable/inter";            /* wght 100–900 */
--font: "Inter Variable", "Segoe UI Variable Text", "Segoe UI", system-ui, sans-serif;
--mono: "Cascadia Mono", "JetBrains Mono", ui-monospace, "SF Mono", Consolas, monospace;
font-feature-settings: "cv11", "ss01";           /* «a» de un piso, dígitos claros */
-webkit-font-smoothing: antialiased;
```

**Cifras:** las cifras, estadísticas, tiempos y tablas usan `font-variant-numeric: tabular-nums` (clase `.num`).

**Escala** (el texto base de la interfaz es de 14 px):

| Token | Uso | Tamaño / interlineado | Peso | Tracking |
|---|---|---|---|---|
| `--fs-display` | Titular del resumen («Todo protegido») | 30 / 36 px | 650 | −0,022em |
| `--fs-title` | Título de página | 22 / 28 px | 650 | −0,018em |
| `--fs-stat` | Cifras grandes | 22 / 28 px | 600 | −0,02em, tabular |
| `--fs-h2` | Cabecera de sección o tarjeta | 15 / 22 px | 600 | −0,01em |
| `--fs-body` | Texto normal, botones, campos | 14 / 20 px | 400 (500 en botones) | 0 |
| `--fs-sm` | Texto secundario, filas densas | 13 / 18 px | 400 | 0 |
| `--fs-xs` | Leyendas, metadatos, chips | 12 / 16 px | 500 | 0 |
| `--fs-overline` | Etiquetas de sección de la barra lateral | 11 / 16 px | 600 | +0,06em, MAYÚSCULAS |

En la web, para pantallas de móvil, `--fs-display` baja a 24 / 30 y `--fs-title` a 20 / 26.

---

## 2. Color

Los colores son tokens. Cada uno tiene su valor para el modo claro y para el oscuro: el oscuro se aplica con `prefers-color-scheme: dark`, salvo que el usuario elija otro, y también con `[data-theme="dark"]`. El modo «Negro» (OLED) solo cambia los neutros.

### Neutros

| Token | Claro | Oscuro | Uso |
|---|---|---|---|
| `--bg` | `#ffffff` | `#0f0f11` | Fondo del contenido |
| `--bg-subtle` | `#f7f7f8` | `#0b0b0d` | Barra lateral, zonas hundidas |
| `--surface` | `#ffffff` | `#151518` | Tarjetas y diálogos |
| `--surface-2` | `#f4f4f5` | `#1b1b1f` | Hover, filas alternas, campos |
| `--surface-3` | `#ececee` | `#232328` | Pistas de barras, chips neutros |
| `--border` | `#e9e9ec` | `#25252a` | Bordes por defecto (finos, 1 px) |
| `--border-strong` | `#d5d5da` | `#34343b` | Separadores marcados, botones secundarios |
| `--border-input` | `#86868f` | `#6c6c76` | Contorno de campos e interruptor apagado (3:1 sobre las superficies) |
| `--text-1` | `#18181b` | `#ededf0` | Texto principal |
| `--text-2` | `#52525b` | `#a6a6b0` | Texto secundario |
| `--text-3` | `#666670` | `#96969f` | Metadatos y leyendas (AA sobre todas las superficies y fondos suaves) |
| `--overlay` | `rgb(9 9 11 / .4)` | `rgb(0 0 0 / .6)` | Fondo de los modales |

Modo Negro: `--bg #000`, `--bg-subtle #000`, `--surface #0a0a0b`, `--surface-2 #121214`, `--surface-3 #19191c`, `--border #1f1f23`.

### Acento

Hay un solo acento, que se usa con moderación: la acción principal de cada zona, el elemento activo, el foco y los enlaces. **Nunca indica un estado.**

| Acento (`data-accent`) | Claro | Oscuro |
|---|---|---|
| `teal` (por defecto, el de la marca) | `#0f766e` | `#3cc4ad` |
| `blue` | `#2563eb` | `#6aa1ff` |
| `indigo` | `#4f46e5` | `#8e8cff` |
| `violet` | `#7c3aed` | `#b38bff` |
| `rose` | `#d6336c` | `#ff7aa6` |
| `amber` | `#b45309` | `#f2b33d` |
| `graphite` | `#3f3f46` | `#d4d4d8` |

De ese tono se derivan los demás:

- `--accent-hover`: 88 % con negro en claro y con blanco en oscuro.
- `--accent-contrast`: el texto sobre el acento, `#fff` en claro y `#0b0b0d` en oscuro.
- `--accent-soft`: 10 % del acento sobre transparente en claro, 14 % en oscuro.
- `--accent-text`: el acento legible sobre `--accent-soft`, al 85 % con negro en claro y al 80 % con blanco en oscuro.

### Estado

Cada estado tiene un color de texto o icono (`--ok`) y un fondo suave (`--ok-soft`). **Un estado nunca se indica solo con color:** siempre lleva icono y texto.

| Estado | Significado | Claro | Oscuro | Icono (Lucide) |
|---|---|---|---|---|
| `ok` | Al día, verificado | `#126c35` | `#4ade80` | `circle-check` |
| `warn` | Con avisos, retraso leve, atención | `#a04e09` | `#fbbf24` | `triangle-alert` |
| `bad` | Fallo, atrasado, sin conexión | `#c42121` | `#f87171` | `circle-alert` |
| `info` | En curso, informativo | `#2257d6` | `#60a5fa` | `loader-circle` (gira) / `info` |
| `paused` | En pausa | `#6d28d9` | `#a78bfa` | `circle-pause` |
| `neutral` | Sin datos, desactivado | `#666670` | `#96969f` | `circle-dashed` |

- **Fondos suaves:** `color-mix(in srgb, var(--X) 9%, transparent)` en claro y al 14 % en oscuro.
- **Bordes de estado**, cuando hacen falta: el mismo color al 30 %.
### Gráficas

Tokens comunes de todas las gráficas (barras, minigráfica, previsión de espacio, línea de tiempo y ondas), para que hablen el mismo idioma:

| Token | Claro | Oscuro | Uso |
|---|---|---|---|
| `--graf-rejilla` | `--border` | `--border` | Líneas de referencia, finas y discontinuas (2 3) |
| `--graf-base` | `--border-strong` | `--border-strong` | La base (línea continua) |
| `--graf-eje` | `--text-3` | `--text-3` | Rótulos de los ejes, 10,5 px tabular |
| `--graf-linea` | `--text-2` | `--text-2` | Líneas (1,5 px) |
| `--graf-marca` | `--text-3` al 75 % | igual | Barras (3:1 sobre la tarjeta) |
| `--graf-guia` | `--text-3` | `--text-3` | Guía vertical al pasar el ratón o con las flechas |
| `--graf-area` / `--graf-area-alfa` | `--text-3` / 0,18 | `--accent` / 0,28 | Arriba del degradado del área (abajo se desvanece) |
| `--graf-brillo` | `none` | `drop-shadow(0 0 3px …)`, acento al 45 % | Brillo bajo la línea y en la barra elegida |
| `--graf-brillo-px` | 3 | 10 | Desenfoque del brillo en las ondas (canvas) y en el punto de hoy |

Los colores de las series (`--onda-1…3`) son los de «Colores por copia» (§4): la línea de tiempo y las ondas usan los mismos tokens.

### Pantallas sin sesión

`--portada-brillo` (7 % en claro, 16 % en oscuro) y `--portada-puntos` (18 % / 22 %): el resplandor del acento y la rejilla de puntos de `.fondo-portada` (§4).

- **Contraste:** todos los textos de estado y `--text-3` cumplen AA (≥ 4,5:1) sobre `--surface`, `--surface-2` y su fondo suave, en los dos modos (comprobado: el peor caso es 4,70:1, `--warn` claro sobre su fondo suave en `--surface-2`). Ver §7.

---

## 3. Espaciado, radios, elevación, foco y movimiento

**Espaciado** (base 4): `--sp-1 4` · `--sp-2 8` · `--sp-3 12` · `--sp-4 16` · `--sp-5 20` · `--sp-6 24` · `--sp-8 32` · `--sp-10 40` · `--sp-12 48` · `--sp-16 64` (px). Para separaciones mínimas dentro de un componente se usan también 2 y 6.

- Entre secciones de una página: 32.
- Dentro de una tarjeta: 20 (16 en las densas).
- Entre filas de una lista: 0, con separador de 1 px (`--border`).

**Radios:** `--radius-sm 6` (chips, botones pequeños) · `--radius 8` (botones, campos) · `--radius-lg 12` (tarjetas) · `--radius-xl 16` (modales, el resumen del inicio) · `999` (píldoras, puntos).

**Elevación:**

- **Claro:**
  - `--shadow-sm: 0 1px 2px rgb(0 0 0 / .04)` para tarjetas que se pueden pulsar.
  - `--shadow-md: 0 1px 2px rgb(0 0 0 / .04), 0 4px 12px -2px rgb(0 0 0 / .06)` para menús y popovers.
  - `--shadow-lg: 0 24px 48px -12px rgb(0 0 0 / .18)` para modales.
- **Oscuro:** sin sombras visibles. La jerarquía se marca con los escalones de superficie y con `--border`. Los modales llevan `--shadow-lg` con negro al 50 % y un borde de 1 px.
- `--shadow-hover` (claro: `0 1px 2px` al 4 % y `0 8px 20px -10px` al 16 %; oscuro: `0 10px 24px -12px` al 70 %): la tarjeta que se puede pulsar, al pasar el ratón.
- Las tarjetas normales no llevan sombra: les basta el borde de 1 px.

**Foco:** `:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }`. Los campos usan, en su lugar, el anillo `--focus: 0 0 0 3px color-mix(in srgb, var(--accent) 25%, transparent)` junto con un borde de acento. El foco nunca se quita sin sustituirlo.

**Movimiento:**

- **Duraciones:** `--dur-fast 120ms` (hover, pulsar), `--dur 180ms` (abrir o cerrar, plegar) y `--dur-slow 260ms` (cambios de cifras, entrada de la vista).
- **Curvas:** `--ease-out: cubic-bezier(.2,.8,.2,1)` y `--ease: cubic-bezier(.4,0,.2,1)`.
- **Movimiento reducido:** con `prefers-reduced-motion: reduce`, todas las duraciones pasan a 0. Se quitan el brillo de progreso, las cifras animadas y los giros que no indican actividad.
- **Al pasar y al pulsar:** las tarjetas que se pulsan suben 1 px (`--dur-fast`) y vuelven al pulsar; los botones bajan 1 px; los fantasma y los de icono toman `--surface-3` mientras se pulsan. Con movimiento reducido no suben: solo cambian el borde y la sombra.
- **Entrada de la vista:** cada página entra con un fundido de `--dur-slow` y 4 px hacia arriba, solo al empezar (`animation-fill-mode: backwards`): al acabar no deja `transform`, que rompería lo fijo de dentro. Las cifras y los pasos de un asistente entran igual (`rise`).

---

## 4. Componentes

- **Botones:**
  - Alto de 34 px (`.btn`), 28 px el pequeño (`.btn-sm`) y 40 px el grande. Padding horizontal de 14 / 10 / 18. Radio 8 (6 en el pequeño). Texto en `--fs-body`, peso 500.
  - Icono de 16 px (14 en el pequeño) con 6 px de separación. El tamaño lo fija el estilo (`.btn > svg`), no cada botón.
  - Variantes:
    - **Primario** (`.btn-primary`): fondo de acento con texto `--accent-contrast`. Solo uno por zona.
    - **Secundario** (por defecto): fondo `--surface`, borde `--border-strong`, texto `--text-1`; al pasar el ratón, `--surface-2`.
    - **Fantasma** (`.btn-ghost`): sin fondo ni borde y texto `--text-2`; al pasar el ratón, `--surface-2`.
    - **Peligro** (`.btn-danger`): fondo `--bad` con texto `--bad-contrast`, que es `#fff` en claro y `#1a0806` en oscuro (el blanco sobre `#f87171` no llega a AA). Solo para acciones destructivas, que siempre piden confirmación.
  - Desactivado: opacidad .45, con un *tooltip* que explique por qué.
  - Botón de icono (`.icon-btn`): 32×32 (28 el pequeño), fantasma y con `aria-label`.
- **Campos** (`.input`):
  - Alto de 34, radio 8, fondo `--surface`, borde `--border-input` (3:1, para que el campo se distinga). El *placeholder* va en `--text-3`.
  - Con foco, borde de acento y anillo `--focus`. Con error, borde `--bad` y el mensaje debajo en `--fs-sm` y `--bad`.
  - La etiqueta va encima, en `--fs-sm` con peso 500 y color `--text-1`, y la ayuda debajo en `--fs-xs` y `--text-3`.
  - El *select* es igual que un campo, con un chevron de 16 px.
- **Casilla e interruptor:**
  - Casilla de 16 px con radio 4; marcada, en color de acento.
  - Interruptor (`.switch`) de 32×18 con pomo de 14; apagado, con contorno `--border-input`; encendido, en color de acento. Se usa para ajustes que se aplican al momento; la casilla, dentro de formularios.
- **Pestañas:** subrayado de 2 px en el color de acento bajo la activa. Texto en `--fs-body` con peso 500: `--text-2` las inactivas y `--text-1` la activa. Sin fondos, y con flechas para moverse.
- **Chips de estado** (`.badge` + `.tone-*`): alto de 22 (18 el pequeño), padding 0 8, radio 999, `--fs-xs` con peso 600. Llevan siempre icono de 12 px y texto, por ejemplo «Al día» o «Atrasado». El fondo es el `-soft` del tono y el texto, su color.
- **Punto de estado** (`.dot`): 8 px, en la barra lateral y en listas compactas. Siempre lleva al lado un texto o un `aria-label`.
- **Tarjetas** (`.card`): fondo `--surface`, borde 1 px `--border`, radio 12, padding 20. Sin sombra. Si se pueden pulsar (`a.card`, `button.card`), al pasar el ratón el borde pasa a `--border-strong`, toman `--shadow-hover` y suben 1 px (§3); con el foco de teclado, el borde es del acento.
- **Escala de radios por tamaño:** 6 lo pequeño (losas de 24, botones pequeños, globos), 8 botones, campos y losas de icono de 32–36, 10 las tarjetas del mapa, 12 las tarjetas, 16 los modales y la tarjeta de entrar, 999 las píldoras.
- **Losas de icono** (como en las tarjetas de NetBird): el icono en una caja `--surface-2` con borde 1 px `--border` y el icono en `--text-2`. 24 px (radio 6) en las cifras, 32 en las tarjetas tranquilas (`.tile-ic`), 44 en la cabecera de página (`.page-icon`, con un degradado mínimo de `--surface` a `--surface-2`).
- **Pastilla de dato** (`.pastilla`, `.pastilla.mono`): para un dato técnico que no es un estado (versión del agente, puerto, la dirección de un destino). Alto 20, padding 0 7, radio 999, fondo `--surface-2`, borde `--border`, texto `--text-2` en 11,5 px (11 px monoespaciada y tabular con `.mono`). Nunca lleva color de estado.
- **Etiqueta de sección** (`.etiqueta-seccion`): `--fs-overline`, peso 600, +0,06em, mayúsculas, `--text-3`. La de los grupos de la barra lateral y la de los grupos dentro de una tarjeta.
- **Tablas** (`.tabla`): líneas finas `--border` entre filas, **sin cebra**; al pasar el ratón, `--surface-2` al 70 %. Cabecera en `--fs-xs` 600 `--text-3` sobre `--surface`, fija arriba (`position: sticky`); las largas (Actividad) van en `.desplazable.alto` (como mucho 72 vh, solo por encima de 640 px) para que la cabecera se quede a la vista. Cifras tabulares; las cantidades (versiones, tamaños, archivos) a la derecha (`.der`, también en su `<th>`); lo técnico en mono de 12 px. Al imprimir, la cabecera no se fija.
- **Filas de lista** (`.row`): alto mínimo de 44, padding 10 16 y separador `--border`. Al pasar el ratón, `--surface-2`. Llevan:
  - a la izquierda, un icono o punto de 16;
  - en medio, el título en `--fs-body` con peso 500 y una segunda línea en `--fs-sm` y `--text-3`;
  - a la derecha, metadatos en `--fs-sm` y `--text-3`, y la acción.
- **Cabecera de sección:** título en `--fs-h2` con un contador opcional en `--text-3` («Destinos · 4»). A la derecha, una acción fantasma o pequeña. Margen inferior de 12.
- **Cabecera de página:** título en `--fs-title`, una línea de resumen en `--fs-sm` y `--text-2`, y a la derecha la acción principal (un único primario) y las secundarias. Margen inferior de 24.
- **Tooltips** (`use:tip`, `lib/tooltip.ts`): fondo `--text-1` con texto `--bg`, en `--fs-xs`, padding 4 8, radio 6 y ancho máximo de 280. Un solo globo al final de `<body>` (nada lo recorta), arriba o abajo si no cabe, siempre dentro de la pantalla.
  - Ratón: a los 400 ms; se va al salir, al pulsar o al desplazar. Teclado: al enfocar con el teclado (`:focus-visible`); Esc lo quita. Táctil: en lo que no es un botón, un toque lo enseña 2,5 s; en botones y enlaces, una pulsación larga.
  - El texto va también en un elemento oculto enlazado con `aria-describedby` (salvo que repita el `aria-label`).
  - Los botones de solo icono (`.icon-btn` con `aria-label`) enseñan su nombre sin ponérselo (se escucha en el documento). Una abreviatura o un dato con explicación: `Tip` (subrayado de puntos y enfocable).
  - En la consola no se usa `title` nativo. La app de escritorio puede usarlo.
- **Modales:** ancho de 440, 560 o 760, radio 16, fondo `--surface` y `--shadow-lg`.
  - Cabecera: icono de 18 en una caja `--accent-soft` de 36 (`--bad-soft` si es de peligro), título en `--fs-h2`, una línea en `--text-2` y el botón de cerrar a la derecha.
  - Cuerpo con padding 24.
  - Pie con las acciones **alineadas a la derecha**: la cancelación como fantasma y la principal al final. Una acción destructiva o secundaria puede ir a la izquierda.
  - Esc cierra, el foco queda atrapado dentro y vuelve a donde estaba al cerrar.
- **Avisos (toasts):** abajo a la derecha, ancho máximo de 380, fondo `--surface`, borde y `--shadow-md`. Llevan un icono de estado y una frase, más una acción opcional (un botón o un enlace: «Ver la copia»). Duran 4 s (8 s los errores y los que llevan acción), se pueden cerrar y se anuncian con `aria-live`.
  - **Órdenes que terminan sin nadie mirando:** si se cierra el diálogo de una orden antes de que el equipo responda, la consola la sigue en segundo plano y avisa al terminar, con enlace. El diálogo lo dice («Puedes cerrar: te avisaremos al terminar»).
- **Estados vacíos:** centrados, con un icono de línea de 32 px en `--text-3` y **una frase** en `--text-2`, con una **acción** como mucho (normalmente primaria). En los momentos clave (primer uso, sin equipos, sin repositorios, sin avisos, sin versiones, sin resultados) llevan una **ilustración** en lugar del icono (`Vacio` con `ilustracion`).
- **Ilustraciones** (`ui/src/componentes/Ilustracion.svelte`): once dibujos de línea propios, en SVG dentro del código (nada externo):
  - `bienvenida`, `sin-equipos`, `sin-repos`, `todo-en-orden`, `sin-versiones`, `sin-resultados` (vacíos);
  - `sin-conexion`, `no-encontrado`, `sin-permiso` (errores);
  - `primera-copia`, `restaurado` (logros).

  Caja de 160 × 120 (144 px de ancho por defecto, 176 en los errores), **un solo grosor de trazo** (2) con puntas redondas y **dos tintas** de los tokens: la línea en `--text-2` (y `--text-3` para lo secundario), los rellenos en `--surface`/`--surface-2`, un suelo en `--surface-3` y el detalle en el acento (trazo `--accent` y relleno al 14 % sobre `--surface`). Así cambian solas con el claro, el oscuro y el acento elegido. Son decorativas (`aria-hidden`): el texto de al lado dice lo mismo. Movimiento solo sin «reducir movimiento»: un vaivén de 2,5 px en 5 s, un parpadeo suave de los destellos y el ✓ que se dibuja una vez. El estado no lo dicen ellas (no son rojas ni verdes): lo dice el texto.
- **Fondo de las pantallas sin sesión y de error** (`.fondo-portada`): `--bg-subtle` con un resplandor del acento arriba (elipse de 60 × 42 %, `--portada-brillo`) y una rejilla de puntos de 22 px (`--text-3` a `--portada-puntos`) que se desvanece hacia los bordes. Quieto y decorativo; el contenido va siempre en una tarjeta opaca o en texto con contraste medido sobre él (§7). La tarjeta de entrar (`Portada`): radio 16, sombra amplia (`0 24px 48px -20px` al 22 %) y un filo de luz arriba; en oscuro, borde `--border-strong`. Es la única textura fuera del mapa: el resto de la consola va en superficies lisas.
- **Pantallas de error** (`PantallaError`, `routes/+error.svelte`): ilustración, título en `--fs-title`, una frase en `--text-2` y una o dos acciones («Ir al inicio», «Reintentar», «Ver mis clientes»). Una dirección que no existe es `no-encontrado`; un cliente que no es de tu cuenta, `sin-permiso`; sin servidor, `sin-conexion`.
- **Logros:** «Primera copia hecha» (una tarjeta con `primera-copia` y «Entendido» encima de *Primeros pasos*; sale una vez y solo si este navegador vio el cliente sin copias) y la restauración terminada (`restaurado` en el último paso).
- **Esqueletos:** bloques `--surface-3` con radio 6 y una opacidad que pulsa entre 1 y .5 en 1,2 s (estáticos con movimiento reducido). Imitan la forma real del contenido.
- **Barras de progreso:** pista de 4 px en `--surface-3`, relleno de acento (o del estado) con radio 999. Las indeterminadas llevan un tramo que se desliza; las activas, un brillo suave que recorre el relleno cada 1,6 s.
- **Anillo de protección:** círculo con pista `--surface-3` y arco proporcional a «puntos / total». Su tono: `bad` si alguna comprobación está en `bad`; si no, `warn` si alguna no está en `ok` (falta algo o está sin comprobar); si no, `ok`. Grosor de 7 sobre 64 (proporcional en otros tamaños). En el centro va «puntos / total» en `--fs-h2` tabular. Hay una versión compacta de 18 px junto al texto «Protección 5 de 7».
- **Cuadros de 60 días** (actividad por día): rejilla de cuadros de 10 px (8 en compacto), radio 2 y separación de 3, el día más reciente a la derecha. Colores:
  - copia correcta: `--ok` al 85 % si guardó datos y al 45 % si fue «sin cambios»;
  - con avisos: `--warn`;
  - fallo: `--bad`;
  - sin copia: `--surface-3`;
  - día futuro o fuera de rango: transparente con borde `--border`.

  Cada cuadro lleva un *tooltip* con la fecha y el resultado. Junto a las tarjetas se usa la versión **mini** de 14 días. Los de un equipo juntan las versiones y copias de todos sus repositorios.

  Para lectores de pantalla, la rejilla es una imagen cuyo texto cuenta los días de cada resultado («Últimos 14 días: 9 días con versiones, 1 día falló, 4 días sin copia.»). Cuando se pueden pulsar (filtrar las versiones de un día), son un solo punto de parada del tabulador con flechas, Inicio y Fin, y al lado va un desplegable «Todos los días» que hace lo mismo con un tamaño cómodo (los cuadros de 11 px no llegan a los 24 px de WCAG 2.5.8).
- **Gráficas** (una serie por gráfica, nunca doble eje). Todas con los tokens `--graf-*` (§2) y el mismo **globo de dato** (`.graf-tip`): ficha `--surface` con borde `--border-strong`, radio 6, `--shadow-md`, el valor en `--text-1` 600 tabular y la fecha o la serie en `--text-3`. En oscuro, el área toma el acento con brillo (como un gráfico en vivo); en claro, tinta neutra.
  - **Barras** (`GraficaBarras`): marcas finas (máx. 10 px) en `--text-3` al 75 % (3:1 sobre la tarjeta), ancladas a la base (`--border-strong`); dos líneas de referencia discontinuas en `--border` (la mitad y el máximo) rotuladas a la izquierda en 10,5 px tabular; fechas del principio, el medio y el final abajo; la media en la cabecera. Al pasar el ratón o con las flechas (la gráfica se enfoca), una guía vertical discontinua en `--text-3`, la barra en el acento y su dato en un *tooltip* (y leído en una región viva). Debajo, «Ver los datos» abre la misma serie en una tabla.
  - **Minigráfica** (`Sparkline`, tendencia de 30 días): trazo de 1,5 px en `--graf-linea` con un área en degradado (`--graf-area`, que se desvanece abajo) y `--graf-brillo`, el último punto en el acento con un halo suave y, al pasar el ratón o con las flechas, guía y dato del día (también leído en una región viva). Su texto alternativo dice el periodo «de X a Y». Sin ejes: el valor de hoy y el cambio del periodo van al lado, en texto.
  - El color de las marcas es tinta neutra; el acento solo señala lo elegido. Los colores de estado solo para estados. Las cifras derivadas («+122 MB», «−3 MB») van en tinta neutra, no en verde ni rojo.
  - **Previsión de espacio** (`GraficaLlenado`, en «¿Cuándo se llena?»): lo ocupado los últimos 60 días (línea de 1,5 px en `--text-2` con un área de `--text-3` que se desvanece; en oscuro, el área en el acento al 28 % y un brillo suave de 3 px bajo la línea), la recta del ritmo actual en discontinua (5 4), la capacidad en punteada `--text-3` rotulada «Capacidad» y el punto de lleno (círculo hueco en `--text-1`) con «lleno · marzo de 2028». Es de líneas, así que el eje empieza cerca del mínimo (rotulado, con los decimales que hagan falta para que los dos rótulos no digan lo mismo). Guía y dato con el ratón o las flechas, región viva y «Ver los datos». El estado (aviso con menos de 3 meses, urgente con menos de 1) va en el chip de al lado, nunca en la línea.
  - **Colores por copia** (la línea de tiempo de las versiones): tres huecos en orden fijo, validados para daltonismo con todos los pares (claro `#2a78d6` `#eb6834` `#1baf7a`, oscuro `#3987e5` `#d95926` `#199e70`); la cuarta copia en adelante y las versiones sin copia, «Otras», en `--text-3`. Cada hueco lleva además su forma (círculo, cuadrado, rombo; raya en «Otras») y siempre hay leyenda: el color nunca va solo. El verde claro no llega a 3:1 sobre blanco: lo compensan la forma, la leyenda, la ficha y la lista de debajo.
- **Mapa de la protección** (`mapa/MapaProteccion`, `lib/mapa.ts`): en Estado y, compacto («Camino de sus copias»), en la ficha de cada equipo (un almacén enseña también lo que los demás guardan en él). De izquierda a derecha en columnas: equipos → repositorios → almacén o destino → espejo y copia externa.
  - **Lienzo**: fondo `--bg-subtle` con una rejilla de puntos de 18 px (`--text-3` al 26 %), a sangre en la tarjeta. Tarjetas de radio 10, borde `--border-strong`, `--shadow-sm` en claro: caja de icono de 30 (`--surface-2`), nombre en `--fs-sm` 600, una línea en `--fs-xs` `--text-3` y el estado con su icono y palabra (en tinta `--text-2`, el icono en su color) y la última vez. Los repositorios son **píldoras** (radio 999, alto 36): un punto neutro si van bien (o el icono y la palabra del estado si no), el nombre, «última hace 3 h» y su tamaño en mono pequeño.
  - **Trazos**: curvas de Bézier discontinuas (5 5) de 1,5 px que salen de un solo puerto (círculo de 3) a la derecha de cada tarjeta. Tinta tenue (`--text-3` al 70 %) si todo va bien; `--warn` o `--bad` si no, con una marca redonda de 20 px con el icono del estado en su mitad (la palabra está en la tarjeta a la que llega); `--info` mientras algo está en marcha (del progreso en vivo), con el trazo moviéndose (0,9 s) y, en oscuro, un brillo suave. Con movimiento reducido, quietos.
  - Arriba, «Equipos | Repositorios | Destinos» (segmentos) y un selector de raíz con chevrons (todos, o uno); se recuerda por cliente en el navegador. Al pasar por una tarjeta o enfocarla, su cadena resalta y lo demás baja al 35 %. Con más de 8 equipos, los que están al día y van a los mismos sitios se juntan en un grupo («13 equipos», con una píldora por destino); los que tienen algo, sueltos. Las columnas se ordenan por la altura media de lo que les llega (menos cruces).
  - **Accesible**: las tarjetas son enlaces (Tab, Intro) y las flechas recorren el mapa (↑↓ en la columna, → a lo que recibe, ← a lo que le manda). Siempre lleva su alternativa en frases («CAJA-1 copia «Caja» a ALMACEN-SUR (al día, hace 3 h)… ALMACEN-SUR se refleja en E:\Resguardo-espejo (al día, hace 13 h) y Dropbox Oficina (falló hace 13 h).»). Por debajo de 640 px de ancho del propio mapa, o con «Ver como lista», es un árbol en vertical con lo mismo.
- **¿Cuándo se llena?** (`llenado/CuandoSeLlena`, `lib/llenado.ts`): en Estado, una fila por almacén, destino y destino del espejo: la frase («Al ritmo actual (~1,5 GB al mes), se llena en ~14 meses (marzo de 2028).»), lo que ocupa de cuánto y cuándo se midió, de dónde sale el ritmo (lo añadido por las versiones de los últimos 60 días; «no descuenta lo que quita la retención») y la gráfica. Siempre «aproximado». Sin capacidad, solo el ritmo y qué haría falta («actualiza el agente de …»; en la nube, «el proveedor no dice un límite»).
- **Línea de tiempo de las versiones** (`repo/LineaTiempoVersiones`, `lib/lineaTiempo.ts`): en la página del repositorio (elegir deja «Explorar y restaurar») y en el paso «Versión» de Restaurar (elegir pasa a los archivos), encima de la lista de siempre. A todo lo ancho, alto 112, fondo `--bg-subtle`: escala Día · Semana · Mes · Año (segmentos), «‹ Hoy ›», arrastre y rueda horizontal. Cada versión es una línea de 2 px con la forma de su copia arriba; las que quitaría la próxima retención (simulada con la regla del repositorio o la del almacén que la aplica: `motivosQuedan`), huecas, a trazos y al 45 %. Debajo, una franja por regla («diarias · 7», «semanales · 4»…) con un punto por versión que guarda. La ficha (fecha, tamaño, lo nuevo, copia y si se queda) sale al pasar el ratón o con el foco. Es un solo punto de parada (`listbox` con `aria-activedescendant`): ←→ versión anterior o siguiente, Inicio y Fin, Intro elige, + y − cambian la escala, Re Pág y Av Pág mueven; lo activo se lee en una región viva.
- **Cabecera de página** (`CabeceraPagina`): migas, icono de 22 en una caja de 44 (`.page-icon`), título, una línea de resumen (o un fragmento con enlaces y «?») y las acciones a la derecha. Todas las secciones del cliente la usan, con el icono de su sección (`lib/iconos.ts`). Por debajo de 860 px, sin icono y con las acciones debajo.
- **Migas** (`Migas`): en las páginas de detalle, encima del título: «Equipos › CONTABILIDAD › Siigo y documentos». En las secciones del cliente, «Ferretería Altamar › Avisos». `--fs-sm`; las anteriores en `--text-2` y enlazadas, la actual en `--text-1` con peso 500; separador chevron de 13 px en `--border-strong`.
- **Copiar** (`Copiable`): códigos, rutas, huellas e identificadores llevan al lado un botón de 24 px con el icono de copiar, que pasa a ✓ en `--ok` 1,5 s y anuncia «Copiado».
- **Índice de página** (`IndicePagina`): en páginas largas (repositorio, copia), una barra fija arriba con un enlace en píldora a cada sección; la que se está leyendo, con fondo `--surface-3`. Solo por encima de 860 px.
- **Menú rápido de una fila** (`MenuAcciones` sin texto, «… ▾»): acciones frecuentes sin entrar en la ficha (copiar ahora, restaurar, ver órdenes). Toda la fila se ilumina al pasar el ratón.
- **Paleta** (`Ctrl+K` / `⌘K`): ventana de 600 px a 14 vh del borde superior, radio 16, campo de 52 px y resultados agrupados (Acciones, Ir a, Equipos, Repositorios, Copias, Clientes) con la fila elegida en `--surface-2`. Busca sin tildes y por todas las palabras. Flechas, Intro y Esc. En la barra lateral, un botón «Buscar o ir a…» con su atajo; en móvil, una lupa en la cabecera.
  - **Acciones:** «Añadir equipo», «Nuevo repositorio», «Copiar ahora en…» y «Restaurar archivos de…». Las dos últimas escriben «copiar ahora» o «restaurar» en el campo y dejan elegir la copia o el repositorio. Siempre abren **los mismos diálogos** que sus botones (`lib/acciones.svelte.ts` + `AccionesGlobales`): la paleta no manda nada por su cuenta.
  - Cada sección con su icono de siempre.
- **Atajos:** «?» los enseña; «G» y una letra va a una sección del cliente (S Estado, E Equipos, R Repositorios, T Restaurar, O Órdenes, A Avisos, I Informes). Nunca mientras se escribe ni con un diálogo abierto.
- **Cifras** (`Cifra` dentro de `.cifras`, fila de 4 bajo un resumen): etiqueta de 12 px con icono de 14 en `--text-3`, el número en `--fs-stat` (con un «de 4» pequeño si hace falta) y una línea en `--fs-xs` (en `--bad` si es un problema). Cuatro como máximo; dos por fila por debajo de 900 px. Las líneas, cortas: en el móvil no deben cortarse.
- **Tarjeta tranquila** (`.tile`, la de `TarjetaEquipo`): icono de 16 en una caja `--surface-2` de 32, nombre y una línea de 12 px, el chip de estado a la derecha y debajo una o dos líneas y los chips. Para equipos, destinos, almacenes y lo que se elige en un asistente.
- **Pasos de un asistente** (`Pasos`, Restaurar y Añadir equipo): círculo de 22 con el número (el actual en acento con un halo `--accent-soft`, los hechos con ✓ en `--accent-soft`) y una línea fina entre pasos, en acento si ya se pasó. Los hechos se pueden pulsar para volver. Por debajo de 760 px solo el nombre del actual; por debajo de 480, «Paso 3 de 7 · Versión».
- **Iconografía** (`lib/iconos.ts`): el mismo icono para lo mismo en la barra lateral, la paleta, las cabeceras y las listas. Secciones: Estado `layout-dashboard`, Equipos `monitor`, Repositorios `database`, Restaurar `history`, Órdenes `clipboard-list`, Avisos `bell-ring`, Informes `file-bar-chart`, Actividad `activity`, Servidor `arrow-right-left`, Personas `users`. Órdenes: Copiar ahora `play`, Restaurar `history`, Descargar `download`, Verificar `shield-check`, Pausar `pause`, claves `key-round`, repositorio nuevo `database-zap`, borrar `trash-2`… En una lista de órdenes, el icono va en una caja de 30 con el color de su estado.
- **Marca del cliente** (`MarcaCliente`, v1.32): cada cliente puede tener **su logo y su color**, que se guardan en el servidor y ven todas sus personas. El color es uno de los siete acentos (mismos valores y mismo contraste; con «El de cada persona», el acento propio). Se usa con moderación, como el acento: solo en su monograma (la inicial en `--marca-texto` sobre `--marca-suave`, como `--accent-text` sobre `--accent-soft`) y en la raya de la portada de su informe (2 px en `--marca`); nunca cambia los colores de la consola ni indica un estado. El logo va sobre una placa blanca con un borde de 1 px al 8 % (un logo oscuro se lee igual en el modo oscuro), dentro de una caja de radio 27 %. Sale en el selector de clientes (26 / 24 px), la cabecera del móvil (24), la lista de clientes (40), Personas y ajustes (36) y la portada del informe (hasta 160 × 64). Es decorativo: el nombre va siempre al lado. Lo cambian propietarios y administradores (`EditorMarca`, desde Informes o Personas y ajustes); el logo se convierte **siempre a PNG en el navegador** (como mucho 512 px y 200 KB) para que ningún SVG llegue al servidor.
- **Icono de la pestaña** (`lib/favicon.ts`): el de siempre; con avisos sin revisar, un punto rojo arriba a la derecha; con algo en marcha, uno azul.

---

## 5. Disposición

- **Barra lateral:** 248 px de ancho (232 por debajo de 1100 px de ventana), fondo `--bg-subtle` y borde derecho `--border`.
  - Arriba, la marca y el botón de plegar; debajo, el selector de cliente (42 px, `--surface` con borde y `--shadow-sm`) y «Buscar o ir a…».
  - Las secciones del cliente van en **tres grupos** con su etiqueta (`.etiqueta-seccion`): **Protección** (Estado, Equipos, Repositorios y destinos), **Operación** (Restaurar, Órdenes, Avisos) y **Gestión** (Informes, Actividad, Servidor, Personas y ajustes). Cada lista se nombra con su etiqueta (`aria-labelledby`).
  - Elementos de 32 px de alto con radio 6 e icono de 16 en `--text-3`.
  - El elemento activo: fondo `--surface` con borde fino (`0 0 0 1px --border` y `--shadow-sm`), texto `--text-1` con peso 550, el icono en `--accent-text` y una **raya del acento** de 3 px a la izquierda (8 px más corta que el elemento por arriba y por abajo).
  - Contadores en pastilla de 18 px (`--surface-3` y `--text-2`; `--warn-soft`/`--warn` o `--bad-soft`/`--bad` si son de un estado), tabulares.
  - **Plegada a iconos** (60 px, `--sidebar-plegada`): el botón junto a la marca la pliega y la despliega; se recuerda en el navegador (`rg.barra.plegada`, con `try/catch`) y, si nunca se tocó, se pliega sola por debajo de 1100 px. Plegada: solo los iconos (36 × 36), una raya corta entre grupos, el nombre en `aria-label` y en el tooltip, los contadores encima del icono (en el color del estado, con texto `--accent-contrast` o `--bad-contrast`), lo que está en marcha en una pastilla de icono y porcentaje, y el menú de clientes de 260 px. En el móvil (cajón) nunca se pliega.
- **Contenido:** ancho máximo de 1120 px, centrado, con padding de 32 (24 por debajo de 1100 px y 16 en móvil en la web).
- **Ancho mínimo** de la ventana de escritorio: 720 px.
- **Densidad:** por defecto, **cómoda**. Las tablas de versiones y de actividad usan filas de 40 a 44 px; en las tarjetas, cuatro datos como máximo.
  - **Compacta** (Mi cuenta → Apariencia → Densidad; una preferencia de este navegador, `localStorage` con `try/catch`): para quien lleva muchos equipos. Pone `data-densidad="compacta"` en `<html>` y quita aire, nada más: filas de lista de 34 px (padding 5 12), celdas de tabla 5 10, tarjetas con padding 16 (12 las tranquilas), separación de 16 entre bloques, 8 en las rejillas y las cifras, elementos de la barra lateral de 28 px y el contenido con 20 / 24 de margen. Los textos, los botones y los campos no cambian, y todo lo que se pulsa sigue en 24 px o más (§7).
- **En marcha** (barra lateral, `CopiasEnMarcha`): cuenta todas las tareas («2 copias en marcha · 40 %», el porcentaje de todas juntas por bytes) y debajo cada una con el suyo y su enlace. El avance entre noticias del equipo se calcula en un solo sitio (`pctPintado`/`pctVisible` de `progreso.svelte.ts`), así que la barra lateral, la fila de la copia y los chips dicen el mismo número. Con dos tareas en el mismo equipo, el chip dice «2 en marcha · 35 %».
- **Restaurar** (asistente): Equipo (tarjetas) → Repositorio (con sus copias; se salta si solo hay uno) → Contraseña → Versión (la línea de tiempo de las versiones de §4 y, debajo, la lista por días, «Hoy», «Ayer», la más reciente marcada; primero la última semana) → Archivos (árbol con «Disco C:», fechas y tamaños; búsqueda con el nombre arriba y la carpeta del equipo debajo; la selección fija abajo con chips que se quitan) → Dónde («junto al original», recomendado) → Progreso (Enviada → El equipo la recoge → Restaurando → Hecho) → Listo, con la ruta de la carpeta «Restaurado …» para pegar en el Explorador.
- **Avisos:** agrupados por gravedad («Urgente», «Para revisar», «Ya vistos») o por equipo; cada uno con su acción directa («Ver sus copias», «Ver el equipo», «Ver las órdenes», «Ver la actividad») y «Visto», y «Marcar todos como vistos».
- **Informe** (para imprimir): periodo (este mes, el pasado —por defecto los cinco primeros días— o 30 días); portada con la marca del cliente (su logo, o el de Resguardo, y su color en la raya; el logo que antes se guardaba solo en el navegador se ofrece pasarlo al cliente); una frase de resumen; cifras; una pila fina por día (correctas en tinta neutra y fallidas en rojo encima); tabla de equipos con sus cuadros y, de cada equipo, sus repositorios. Al imprimir: A4, solo la hoja, siempre en claro, sin cortar bloques ni filas y con los colores de los cuadros (`print-color-adjust: exact`).
- **Inicio («Estado»):**
  1. **Resumen grande:** titular en `--fs-display` («Todo protegido», «2 cosas necesitan atención», «Copiando…»), una línea de resumen y una lista breve, ordenada por gravedad, de lo urgente: cada punto con el icono de su estado, quién y qué le pasa, y su acción directa.
  2. **Cifras:** equipos al día (con una barra fina del reparto por estado), datos protegidos, versiones de las últimas 24 h (y copias fallidas) y la próxima copia.
  3. **Mapa de la protección** (§4), con el filtro de etiqueta.
  4. **Equipos** en tarjetas de salud: nombre, estado, conexión, última copia y próxima, y los 14 días de todas sus copias.
  5. **Repositorios** en tarjetas tranquilas: nombre, estado, última versión, nube, protección compacta y los 14 días.
  6. **Dónde se guarda:** cada destino (un almacén cuenta una vez) con sus repositorios, lo protegido, lo que ocupa en disco y la minigráfica de 30 días.
  7. **¿Cuándo se llena?** (§4): previsión de espacio de cada almacén, destino y destino del espejo.
- **Ficha de un equipo:** migas, cabecera con la acción principal, cuatro cifras (última copia, próxima, protegido, versiones en 24 h), los 60 días de todas sus copias con leyenda, el camino de sus copias (el mapa con solo lo suyo) y, debajo, copias, repositorios y destinos.
- **Listas con filtros** (Equipos, la etiqueta elegida): el filtro se recuerda en el navegador por cliente (`localStorage`, siempre con `try/catch`; nunca datos de los equipos ni secretos). Cuando un filtro deja la lista vacía, el estado vacío ofrece quitarlo.

---

## 6. Voz

- **Español, de tú, frases cortas y tranquilas.** «Todo protegido.» «Última copia hace 12 min.»
- **Alarma solo cuando es urgente de verdad.** Un retraso leve es un aviso, no un error. Se evitan las exclamaciones y las mayúsculas para enfatizar.
- **Cada problema dice qué hacer:** «Atrasado 2 días · Copiar ahora».
- **Mismos términos en todas partes:** equipo, copia, repositorio, versión, destino, almacén (un equipo que guarda copias de otros), espejo (lo que un almacén copia cada noche a otro sitio), copia externa (o subida a la nube), consola (Resguardo Server, que coordina y no guarda copias), verificación, prueba de restauración, en pausa, sin cambios, kit de recuperación.
- **Comunicativa:** todo lo que tarda dice que está en marcha («Enviada. Esperando a que el equipo la recoja…») y avisa al terminar, aunque ya no se esté mirando. Cada acción da noticia al momento (un aviso, el botón que dice «Marcando…», la fila que se va) y cada estado vacío ofrece el siguiente paso.
- **Sin jerga:** nada de «restic», «rest-server», «SAS» ni «lo destructivo» en la interfaz: «Espera antes de borrar», «número de comprobación», «los almacenes»…
- **Números:** con espacio fino antes de las unidades («2,1 TB», «12 min»), coma decimal y tiempos relativos con la fecha exacta en el *tooltip*.

---

## 7. Accesibilidad (WCAG 2.2 AA)

La consola se revisa con el teclado, con el árbol de accesibilidad del navegador, en claro y en oscuro, a 1280 px, a 640 px (zoom del 200 %) y a 320 px. Estas son las reglas que hay que mantener al añadir pantallas.

**Contraste** (comprobado con la fórmula de WCAG; claro / oscuro):

- Texto: 4,5:1 como mínimo, también el de estado sobre su fondo suave y sobre `--surface-2` (las filas al pasar el ratón). Peores casos: `--warn` claro sobre `--warn-soft` en `--surface-2`, 4,70:1; `--neutral` oscuro sobre su fondo suave en `--surface-2`, 4,74:1; `--text-3` 4,8 / 5,3:1 sobre `--surface-3`.
- Lo nuevo de la pasada estética (medido con `node` sobre los tokens; claro / oscuro): etiquetas de grupo 5,30 / 6,70; contadores de estado sobre su fondo suave en la barra (también al pasar el ratón) 4,62 / 5,05 en el peor caso; contadores de la barra plegada 5,85 / 7,02; pastillas 5,16 / 5,85 (en tinta tenue); globo de gráfica y cabecera de tabla 5,68 / 6,21; fila de tabla al pasar 5,31; raya del elemento activo ≥ 4,31:1 con los siete acentos; el pie de entrar sobre el resplandor ≥ 4,73.
- Componentes y gráficos: 3:1. Los campos y el interruptor apagado usan `--border-input` (3,0:1 en el peor caso, sobre `--surface-3` en oscuro; 3,6 / 3,5:1 sobre `--surface`), no `--border-strong` (1,46:1, solo decorativo). Las marcas de las gráficas van al 75 % de `--text-3` (3,3 / 3,8:1).
- El foco es el contorno de 2 px del acento (≥ 3:1 sobre todos los fondos, con los siete acentos). Un fondo solo (`--surface-2`) no basta como foco: los menús y el «?» llevan también el contorno.
- Las herramientas: `node` con la fórmula de luminancia sobre los tokens, y en cada página una pasada que mide el color real de cada texto sobre su fondo opaco.

**Teclado:**

- El primer Tab de cada pantalla con sesión es «Saltar al contenido». Al navegar, el foco va al `<main>`.
- Los diálogos (`Modal`) atrapan el foco, se cierran con Esc y devuelven el foco a lo que los abrió. Lo mismo el cajón del móvil (`role="dialog"`) y la paleta (`Ctrl+K`, patrón *combobox*: el foco se queda en el campo y las flechas mueven la opción).
- Los desplegables (menú de acciones, selector de cliente) se abren con Intro, se recorren con las flechas, se cierran con Esc devolviendo el foco al botón y al salir con Tab.
- El «?» (`InfoTip`) abre un globo al final de la página: el foco entra en él y, al salir con Tab, vuelve al «?» y sigue en orden.
- Un grupo de muchos objetivos pequeños (los cuadros de días) es **un solo** punto de parada con flechas (*roving tabindex*). La línea de tiempo de las versiones también (con `aria-activedescendant`); el mapa de la protección, en cambio, son enlaces de verdad y las flechas solo ayudan a moverse.
- `scroll-padding-top` de 64 px: lo enfocado no queda debajo de lo fijo (índice de página, cabecera del móvil).

**Objetivos táctiles:** 24 × 24 px como mínimo (WCAG 2.5.8). El «?» mide 24 aunque su icono sea de 13; los botones de copiar, 24; las migas, 24 de alto. Los enlaces dentro de una línea de texto quedan exentos. Si algo tiene que ser más pequeño (los cuadros de 11 px), al lado va un control equivalente de tamaño normal.

**Lectores de pantalla:**

- `lang="es"`, un solo `<h1>` por pantalla y sin saltos de nivel (una hoja dentro de la página, como el informe, empieza en `<h2>`). Puntos de referencia: el `<nav>` principal con nombre, el `<main>`, y `<section aria-labelledby>` para las zonas con título.
- Nada de `aria-label` en un `<span>`, `<p>` o `<div>` sin rol (no se lee en todos los lectores): un texto `.sr-only` al lado y lo visual con `aria-hidden`. Los códigos que se comparan cifra a cifra (emparejar, número de comprobación) se leen separados.
- Las tablas llevan `<caption>` (puede ser `.sr-only`) o `aria-labelledby` y `<th scope="col">`.
- Las gráficas son `role="img"` con un texto que resume los datos; el dato que se elige con las flechas se lee en una región viva **fuera** de la imagen (sus hijos no se leen), y la de barras tiene «Ver los datos» con una tabla.
- **Regiones vivas sin ruido** (`Anuncio.svelte`): solo hablan cuando su texto cambia, nunca al cargar. Se anuncian el titular del estado, las fases y cada cuarto (25, 50, 75 %) de una copia en marcha, el número de resultados de la paleta, los avisos (`Toaster`), «Copiado» y los errores (`role="alert"`). No se anuncian los refrescos automáticos («Actualizando…» es solo visual) ni cada porcentaje.
- Barras de progreso: `role="progressbar"` con `aria-valuenow` y un `aria-valuetext` con la fase y las cifras.

**Formularios:**

- Cada campo con su `<label>`; la ayuda y el error del campo enlazados con `aria-describedby`, y `aria-invalid` mientras hay error. Los errores de todo el formulario van en un `role="alert"` debajo.
- Los obligatorios llevan `aria-required="true"` (en `CampoClave`, la propiedad `requerido`). El botón de enviar se desactiva hasta que están completos y su `tooltip` dice qué falta.

**Ilustraciones:** `aria-hidden` (el texto de al lado lo dice todo) y quietas con `prefers-reduced-motion: reduce`. La marca del cliente también es decorativa: su nombre va siempre al lado en texto.

**Movimiento y zoom:** con `prefers-reduced-motion: reduce`, duraciones a 0 (también las transiciones de Svelte, con `dur()`), sin brillo de progreso ni barras que crecen, y el índice de página salta sin desplazamiento suave. A 320 px de ancho y al 200 % de zoom no hay desplazamiento horizontal de la página: las tablas anchas se desplazan dentro de su tarjeta (`.desplazable`).

---

## 8. Instalable (PWA)

La consola se puede instalar como aplicación en el móvil y en el escritorio (Chrome, Edge, Safari).

- **Manifiesto** (`static/manifest.webmanifest`): «Resguardo Server», nombre corto «Resguardo», `display: standalone`, empieza en `/`. Iconos: el SVG de la pestaña y PNG de 192 y 512 (el escudo con sus esquinas redondeadas) y sus versiones *maskable* (fondo a sangre y el escudo dentro de la zona segura, el 80 % central). `apple-touch-icon` de 180.
- **Colores de la barra del sistema:** `theme-color` del fondo, `#ffffff` en claro y `#0f0f11` en oscuro (cada `<meta>` con su `media`); si en Apariencia se elige un tema, el de ese tema (`#000` en Negro).
- **Service worker** (`src/service-worker.js`), mínimo: guarda **solo el armazón** (lo de `static/` al instalarse, unos 60 KB, y los archivos de `_app/immutable`, que llevan su hash en el nombre, según se piden). **Nunca** guarda respuestas de la API (`/api/…`) ni ninguna página HTML: los datos de los clientes no se quedan en el navegador. Las páginas siempre van a la red; si no hay red, enseña **«Sin conexión con la consola»** (`static/sin-conexion.html`: la ilustración `sin-conexion`, una frase y «Reintentar»; sin scripts, en claro u oscuro según el sistema). Se registra a mano y solo en producción (en `npm run dev` no hay); si el navegador no se fía del certificado del servidor, no se registra y la consola va igual.
- **CSP:** el `<meta>` añade `manifest-src 'self'` (`worker-src 'self'` ya estaba). La cabecera del servidor no cambia: `manifest-src` y `worker-src` caen en `default-src 'self'` y `script-src 'self'`.
- Para probarlo sin servidor: `npm run build && npx vite preview --mode mock` (el simulador también responde en `preview`).
