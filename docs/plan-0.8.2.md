# Plan de la 0.8.2 (versión pequeña de pulido)

Propuesto el 2026-10-21 a partir del uso real de la 0.8.1. **Pendiente del visto bueno del responsable.** Rama de trabajo: `ia/082-pulido` (y ramas `ia/082-*` si hace falta repartir).

## A. Infraestructura clara y a mano

- [x] (rama `ia/082-infra`) **A1. Dónde se configura:** pestaña propia **«Infraestructura»** en **Ajustes**, junto a «Organización» (se configura una vez, como las sedes; no es un destino). La de «Destinos → Infraestructura» redirige allí. Accesos directos: «Anotar» en «Dónde está físicamente» de cada equipo y destino, y un aviso suave en Estado si hay equipos o destinos sin anotar (se puede ocultar).
- [x] (rama `ia/082-infra`) **A2. Discos físicos uno a uno (15vv):** discos de cada servidor con nombre, tipo, modelo, número de serie (escrito a mano), tamaño, bahía y estado; cada conjunto hecho de discos concretos.
- [x] (rama `ia/082-infra`; el soporte de los originales en la regla aún es el del equipo, no el de cada volumen) **A3. La tarjeta «Equipo» en tres niveles (15ww):** discos físicos → conjuntos o discos virtuales → volúmenes (C:, D:, E:), con lo que guarda cada volumen y «dónde está físicamente» por volumen.
- [~] (rama `ia/082-infra`: filtro en Estado, Equipos y «Todos los clientes» con un cliente elegido, recordado por persona, y lo anotado en cada tarjeta; **falta** la capa Infraestructura como columnas propias servidor → VM) **A4. Filtrar el mapa por infraestructura (15xx):** por servidor físico y por disco o conjunto, y lo anotado visible en cada nodo.
- [x] (rama `ia/082-infra`: `infra:` con `admite: "infra_comun"`; dónde está cada cosa, con la clave de administración; sin probar con dos consolas reales) **A5. Infraestructura igual en todas las consolas del cliente** (lo que quedó a medias de C4): hoy lo anotado en la consola local no se ve en la en línea. Pasa a dato común (`infra:`), como las sedes.

## B. Fallos vistos con la 0.8.1

- [x] **B1.** «Firma no válida» en Actividad → Órdenes mientras carga el cliente (rama `ia/082-pulido`).
- [x] **B2.** El mapa dibuja una copia derivada «de cada día» ya quitada (rama `ia/082-pulido`, agente y consola).
- [ ] **B3. El freno del espejo tras la poda del propio almacén (15yy):** la retención del almacén poda y el espejo lo toma por «falta de golpe el 25 %» y sale como «Espejo fallido». El almacén sabe lo que podó: no cuenta para el freno, y el aviso dice «tras la poda del día X».
- [x] (rama `ia/082-consola`: tarjeta y camino del repositorio, «¿Por qué este estado?» y los avisos `externa_fallida` del servidor; en el historial queda como entrada pasada) **B4.** Repasar las otras tarjetas que usan la última vuelta de una copia ya quitada (tarjeta del repositorio, historial, «Dónde está») con el mismo criterio que B2.

## C. Detalles pequeños

- [ ] **C1. Dejar las consolas desde la ventana del agente (15uu)**, con la clave de administración del cliente, y que la consola diga «dejó esta consola desde el propio equipo».
- [x] (rama `ia/082-consola`: «No copiar…» en «Varios a la vez»; la plantilla, también la de una etiqueta, con las reglas de ahora de cada grupo) **C2.** Grupos de exclusiones en «Varios a la vez» y como plantilla por etiqueta (lo que quedó de 15y).
- [x] (rama `ia/082-consola`: lo calcula el servidor, en memoria; en el progreso, la página de la copia y «Copias», con «desconectado desde…»; **falta** en «Todos los clientes») **C3.** Una copia que estaba en marcha cuando el equipo deja de responder: «Interrumpida: X se desconectó a las …» (lo que quedó de 15jj).
- [x] (rama `ia/082-infra`: «luego» se oculta en el mapa) **C4. Equipos en vista «Mapa»: los controles de arriba hacen lo que dicen** (visto el 2026-10-21): hoy «Agrupar por», «luego» y «Orden» casi no cambian el mapa (por sede, el mapa ya dibuja sus recuadros; los demás niveles solo se aplican si el control propio del mapa, «Con más de 8», agrupa; «luego» y «Orden» no se usan). Hacer: en el mapa de Equipos, **un solo control**: «Agrupar por» manda (sede, categoría, sistema, tipo, y con A4 servidor físico o disco), con recuadros por grupo como los de sede y grupos plegables, sin el selector propio del mapa; «luego» agrupa dentro de cada recuadro o se oculta; «Orden» ordena los equipos de arriba abajo en su columna (también «A mano»). Lo que no aplique al mapa se oculta en vez de quedarse sin efecto.
- [~] (rama `ia/082-consola`: `.page-actions` y dos desbordes a 375 px arreglados; lo grande, en plan-mejoras 15zz) **C5.** Repaso de textos y pantallas que se vean en el uso hasta empezar.

## Fuera (para una versión posterior)

Pasar al almacén gestionado los adoptados (15dd punto 2), reanudar una copia cortada por perder la red (15ii punto 3), conectar un hipervisor (15e), que Resguardo Server se actualice solo (11g).
