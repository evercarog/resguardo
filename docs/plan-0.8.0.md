# Plan de la versión 0.8.0 (propuesta para revisar)

Propuesta del 2026-10-13, **pendiente del visto bueno del responsable**. Recoge los bloques 12, 13 y 14 de [plan-mejoras.md](plan-mejoras.md) y lo que quedó de la 0.7.26, tras una semana de uso real. Decidido (2026-10-09): **una sola versión grande**. Marca aquí el estado de cada bloque (`[ ]` pendiente, `[~]` en curso, `[x]` hecho, con la rama). Lo que cambie mensajes entre agente y consola va en `docs/api-servidor.md` → «Cambios», compatible hacia atrás.

**Objetivo:** que lo que ya existe funcione sin sorpresas (fallos encontrados en uso real), que se **vea** el camino completo de cada copia, ordenar equipos y destinos por sedes, restauraciones y pruebas con registro (RTO/RPO), y más seguridad en la consola web.

**Principio:** el detalle de cada punto está en `plan-mejoras.md` (bloque 14, puntos 14a–14bbb; bloques 12 y 13). Aquí, el orden, el alcance y las decisiones.

## Bloque A — Fallos encontrados en uso real (primero)

- [~] **A1. Vinculación silenciosa (14ee).** (Rama `ia/080-a-agente`: `vincular` prepara la carpeta del agente y deja siempre el motivo, el instalador espera y reintenta y lo anota en `instalador.log`, la línea lo enseña con la orden para vincular a mano. **Falta** probar el instalador en una máquina virtual de Windows.) La línea de PowerShell instala pero no vincula (3 de 3 veces, sin rastro en el registro). Reproducir en máquina virtual, arreglar `PairNow` del instalador, dejar el motivo en `instalador.log` y enseñarlo en la línea. **Imprescindible antes de desplegar muchos equipos.**
- [x] **A2. Derivada o copia externa hacia un destino con autoridad TLS propia (14jj).** (Rama `ia/080-a-agente`, con prueba de integración contra un rest-server real; queda verla en una máquina virtual de Windows con un almacén de verdad.) Pasar a restic las dos autoridades (origen y destino). Prueba de integración local → almacén con CA propia.
- [x] **A3. Claves de una orden en espera comprobadas al recibirla (14oo)** (rama `ia/080-a-agente`), no a las 24 h; la ventana dice de qué repositorio es la contraseña que pide.
- [~] **A4. «Elegir qué copiar» que no hace nada (14rr)**; **copia externa o derivada con el destino borrado (14nn)**: detectarlo, «Quitar» o «Volver a crearla», y «Quitar» también en la página del repositorio. (Rama `ia/080-a-consola`: 14rr y «Quitar» en la página del repositorio. Rama `ia/080-a-agente`: la parte del agente de 14nn y «Quitar esta copia» / «Volver a crearla desde cero» en la tarjeta del repositorio del equipo. Falta 14rr y «Quitar» en la página del repositorio.)
- [x] **A5. Mapa:** (rama `ia/080-a-consola`) el mismo servidor dos veces y espejos colgados del sitio equivocado (14mm); dibujar derivadas (14kk).
- [~] **A6. Textos:** (rama `ia/080-a-consola`: 14y, 14gg, 14ff y 14aa —botón apagado que dice qué falta— hechos; 14tt hecho en la rama `ia/080-a-agente`) sin datos de ningún cliente en ejemplos y pruebas (14y), aviso de derivada sin respuesta sin «restic» y con qué hacer (14tt), frase de varios volcados (14gg), «Un destino nuevo…» siempre visible (14ff), botón apagado dice qué falta (14aa).

## Bloque B — Lo que desbloquea el trabajo real

- [x] **B1. Nombres legibles y renombrar (14e):** (rama `ia/080-nombres`; diseño en [nombres-legibles.md](nombres-legibles.md), contrato v1.6x en `api-servidor.md`; en el destino del espejo no se mueve nada, valorado y dejado para más adelante) carpetas `equipo-sufijo/repositorio` en el almacén para lo nuevo; renombrar repositorios en todas las consolas; cambio coordinado de carpeta para lo existente (almacén, equipo y espejos a la vez).
- [~] **B2. Destinos claros (14b, 14r):** (rama `ia/080-destinos`: hecho todo menos, en 14r, el tamaño real de cada repositorio en el destino —se enseña el del almacén— y el enlace a la página propia de cada espejo, que llega con 14i; la carpeta base y el bucket de una nube no viajan aún a las demás consolas) un solo formulario; el destino recuerda bucket y carpeta base y el espejo no los vuelve a pedir (enseña la ruta final); «Quitar»/«Desconectar» en cada tarjeta cuando se puede (y bloqueado con motivo cuando no; hoy se puede desconectar una nube que usan los espejos); catálogo y nube conectada con el mismo bucket = un solo destino; aviso de repetidos.
- [x] **B3. Derivada a un almacén sin usuario (14hh):** (rama `ia/080-destinos`, agente con `solo_destino`; sin probar contra un almacén real) «Dar acceso a este equipo en el almacén» ahí mismo, sin repositorio de relleno.
- [~] **B4. Contraseñas (14ii, 14bbb):** (rama `ia/080-contrasena-versiones`: **14bbb hecha** —orden sin espera que abre el repositorio y sus copias con la misma contraseña, bloqueo por intentos, fecha en el resumen y en la salud, recordatorio por cliente—; 14ii va en otra rama) «Generada / La escribo yo» en todas partes; **«Comprobar la contraseña»** contra el propio repositorio (y sus espejos), sin espera, con bloqueo por intentos y fecha en la salud.
- [x] **B5. Instalador del agente sin terminal (14dd):** (rama `ia/080-contrasena-versiones`: lo toma solo de la versión firmada bajada de GitHub si no hay ninguno o es más nuevo, y «Ponerlo ahora» en «Instalar muchos equipos» y «Añadir equipo»; sin «Subir el instalador» aparte: «Subir una versión» ya lo trae) la consola lo toma de la publicación firmada (con B6) o se sube desde la consola.
- [x] **B6. Versiones del agente (14f):** (rama `ia/080-contrasena-versiones`: bajada y comprobación cada 6 h o con «Buscar ahora», y «Buscar ahora» para los equipos en «Versiones»; queda probarlo contra la publicación real de GitHub, que las pruebas nunca tocan) el servidor baja solo las versiones firmadas de GitHub (comprobando la firma) y «Buscar ahora».

## Bloque C — Ver claro (sin funciones nuevas)

- [x] **C1. El camino de cada copia a la vista (14kk, 14h):** (rama `ia/080-camino`: `lib/camino.ts` con vectores; bajo el título de la copia, en la tarjeta del repositorio, en «Copias» con sus cuentas y el filtro, y la derivada como repositorio propio; el mapa ya lo dibujaba desde `ia/080-a-consola`. Lo no probado, en `registro-ia.md`.) «copia → repositorio → derivada → espejo» bajo el título de la copia, en su tarjeta, en el mapa y en la página «Copias» (una línea por paso con estado; cuentan en «Necesitan atención»); la derivada como repositorio propio en el equipo.
- [ ] **C2. Espejos (14h, 14i, 14s, 14c):** página de cada espejo con historial y gráficos; primera subida avisada con su tamaño; un espejo que nunca terminó se cambia o quita sin espera; títulos de órdenes concretos.
- [ ] **C3. Progreso en vivo de todo lo largo, en todas las consolas (14p, 13c, 13f):** verificación, prueba, restauración, espejo, poda, traer historial, derivada y actualización; «en cola» mientras no empieza.
- [x] **C4. Un solo vocabulario (14x, 14z, 14w, 14n):** (rama `ia/080-camino`; «Solo añadir» y la salud también en el agente, con vectores compartidos y pruebas; contrato en `api-servidor.md` como v1.6x.) copia, espejo y copia derivada; «fuera del sitio» es del destino; «Solo añadir» no es «Inmutable»; la salud de la protección coherente con la regla 3-2-1-1-0; textos del «0».
- [~] **C5. Pulido de pantallas:** (rama `ia/080-pulido`: hechos 14q, 14vv, 14ww-1, 14bb, 14d, 14o —sin el enlace a las fichas de 14l y 14k, que aún no existen— y la ayuda de 14qq y 14yy; **falta 14zz**, el explorador de carpetas, que no entraba en esta rama; lo no probado en `registro-ia.md`) «Más opciones» con aire y aviso de exclusiones dudosas (14q); gráficas agrupadas por tema (14vv); observaciones de la copia a la vista (14ww-1); color al añadir una etiqueta (14bb); explorador de carpetas con reintento, sin cuentas internas de Windows y con subcarpetas marcables que excluyen lo desmarcado (14zz); historial con todas las versiones del rango (14d) y detalle de cada entrada (14o); ayuda del paso «Cuándo» con el truco «cada 5 min + solo si hay cambios» (14qq).
- [ ] **C6. Lo guiado en otras partes** (bloque 5 de la 0.7.26, pendiente): Nuevo repositorio y Nuevo destino, retención con plantillas sin nombres de cliente, Restaurar, página del equipo.

## Bloque D — Organizar: sedes, grupos y equipos

- [ ] **D1. Sedes (14ll, 14ss-4):** dato propio (no etiqueta), lista por cliente; cada equipo y cada destino en una sede; «fuera del sitio» = otra sede que el equipo (nube siempre fuera); regla 3-2-1, avisos de «mismo equipo» e «inmutable todo en el sitio» con sedes y derivadas.
- [ ] **D2. Vista de Equipos (14ss):** categorías de etiqueta (un valor por equipo), agrupar por niveles («Sede, luego Tipo») sin duplicados, orden manual igual en todas las consolas.
- [x] **D3. Datos del equipo (14uu):** (rama `ia/080-datos-equipo`; contrato v1.6x en `api-servidor.md`, lo no probado en `registro-ia.md`) sistema exacto, procesador, memoria, discos, modelo y último arranque; avisos de disco casi lleno y de sistema que se queda sin soporte.
- [ ] **D4. «Todos los clientes» con secciones (14xx):** Copias, Equipos, Avisos, Órdenes en espera, En curso y Restauraciones de todos.

## Bloque E — Restauraciones y pruebas

- [x] **E1. Restaurar mejor (13a, 13b, 14m):** (rama `ia/080-restaurar`; contrato v1.6x en `api-servidor.md`; falta en 14m los espejos del propio equipo; lo no probado en `registro-ia.md`) en otra carpeta (nueva o vacía), en .zip (en el equipo o descargado, con AES-256 opcional), y elegir desde dónde (almacén, espejo, derivada) con recomendación si el principal no responde.
- [x] **E2. Registro de restauraciones (13d, 14l, 13e):** (rama `ia/080-restaurar`; 13c y 13f a medias: sin carpeta en curso ni vista «En curso») página «Restauraciones» por cliente y equipo, ficha automática (quién, qué, cuánto, cuánto tardó) con motivo; estadísticas en Informes; una restauración real correcta cuenta como prueba.
- [ ] **E3. Pruebas funcionales (14k):** política por cliente, aviso al vencer, formulario de la prueba con RTO y RPO, historial; cuenta para el «0».
- [ ] **E4. Prueba de restauración desde un espejo (14j).**

## Bloque F — Copias y versiones

- [x] **F1. Cuándo termina una copia (14pp):** (rama `ia/080-copias`; contrato v1.6x en `api-servidor.md`, lo no probado en `registro-ia.md`) «Solo a mano», «Una sola vez» y «Termina después de N copias o el día …»; estado «completada» sin avisos.
- [x] **F2. Versiones (14cc, 14ww-2):** (rama `ia/080-copias`: comentarios y «conservar» guardados en el equipo; en el almacén, lista de ids protegidos) comentarios en cada versión; «Conservar para siempre» con motivo obligatorio (también al quitarlo), respetado por la retención del almacén.
- [x] **F3. Retención (14u):** (rama `ia/080-copias`: planificador del almacén y, con «por día» o versiones conservadas, también el del equipo; vectores compartidos) «N versiones por día» decidido por el planificador del almacén, también sobre la historia existente.
- [x] **F4. Límite de velocidad según la hora (14t)** (rama `ia/080-copias`: rclone `--bwlimit` con horario en los espejos a una nube; restic `--limit-upload` de la franja en la que empieza cada derivada) en espejos y derivadas.

## Bloque G — Informes y avisos

- [ ] **G1. Informe ejecutivo programado (14g):** PDF por correo con el logo del prestador (copias correctas, regla 3-2-1-1-0, verificaciones, pruebas y restauraciones del periodo).
- [ ] **G2. Plantillas de avisos para Teams, Slack y Discord (14g).**

## Bloque H — Seguridad de la consola y del equipo

- [ ] **H1. Aprobar una orden en espera desde el propio equipo (14a)** con la clave de administración (ventana del agente en Windows; `resguardo-agente ordenes aprobar` en Linux); nunca desde la web.
- [ ] **H2. Consola verificada (12a):** la consola web firmada con la llave de publicación y comprobada antes de escribir la clave de administración (desde la ventana del agente).
- [ ] **H3. Lo delicado desde la ventana del agente (12b):** cambiar la clave, restaurar en otro sitio, quitar repositorios.
- [ ] **H4. Passkeys (12c)** para entrar en la consola, además del TOTP.
- [ ] **H5. Regla de dos personas (12d)**, opcional por cliente.
- **12e. Pentest externo:** no es trabajo de código; se prepara la lista de lo que hay que revisar y se contrata aparte cuando el responsable decida.

## Bloque I — Publicar y actualizar

- [ ] **I1. Primera actualización automática real** (bloque 6 de la 0.7.26, pendiente): anillo «prueba» con uno o dos equipos de prueba, después «general»; vuelta atrás probada en una máquina virtual de pruebas.
- [ ] **I2. Publicación:** versiones de agente y servidor; el responsable firma; notas; instaladores en `instalar\`.

## Aparcado (no entra; decisiones del responsable)

- 14qq «Cuando algo cambie» (lo cubre «cada 5 min + solo si hay cambios»).
- 14yy Cambiar la contraseña de un repositorio (se documenta el procedimiento con una derivada con otra contraseña).
- 14aaa Exportar todas las contraseñas, kit del cliente y repositorios compartidos por varios equipos.
- 14v Copias derivadas filtradas hechas por el almacén: **a decidir** (ver «Decisiones pendientes»).

## Contrato (compatible hacia atrás)

Campos opcionales nuevos (los agentes antiguos los ignoran; la consola ofrece solo lo que cada agente entiende con `admite`): sedes y datos del equipo en el resumen; comprobación de claves al recibir una orden en espera; «comprobar contraseña»; dar acceso en el almacén; renombrar y mover carpeta coordinado; comentarios y «conservar» de versiones; copias con fin (`termina`); límites de velocidad por hora; aprobar en el equipo. Todo se anota en `api-servidor.md` → «Cambios».

## Pruebas

En cada bloque: `cargo fmt`, `clippy`, `cargo test --workspace`, `npm run check`, `build`, `test:vectores`, `test:sin-referencias` y el e2e (`npm run e2e`) con escenarios nuevos para A1–A4, B1–B4, C1, D1, E1–E2 y F1. A1 y A2 con prueba en máquina virtual de Windows. Al cerrar cada bloque, revisión del código y capturas de las pantallas nuevas para el responsable.

## Orden

A → B → C → D → E → F → G → H → I. Cada bloque en su rama `ia/080-<bloque>`, unida a la rama de integración solo si pasan todas las comprobaciones. Al terminar A y B (lo que más frena hoy), una versión de prueba para un equipo de prueba antes de seguir con el resto, sin publicarla.

## Decisiones pendientes (para el visto bueno)

1. **Nombre:** 0.8.0 (salto grande) o 0.7.27.
2. **Bloque H completo** en esta versión (consola verificada, passkeys, dos personas) o solo H1 y dejar H2–H5 para la siguiente.
3. **14v** (derivadas filtradas hechas por el almacén con la llave que ya tiene por la retención): sí o no.
4. **G1** con el logo del prestador: ¿uno para todos los clientes o uno por cliente?
5. **E1 .zip descargado al navegador:** tamaño máximo (propuesta: 2 GB; lo mayor, al disco del equipo).

## Decisiones tomadas

- Una sola versión grande (2026-10-09).
- Las sedes son un dato propio de equipos y destinos, no etiquetas (2026-10-09).
- Aparcados 14qq, 14yy y 14aaa (2026-10-09).
