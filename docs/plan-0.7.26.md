# Plan de la versión 0.7.26

Acordado con el responsable del proyecto el 2026-10-07. Marca aquí el estado de cada bloque (`[ ]` pendiente, `[~]` en curso, `[x]` hecho, con la rama). Lo que cambie mensajes entre agente y consola va en `docs/api-servidor.md` → «Cambios», compatible hacia atrás.

**Objetivo:** que los espejos sean el camino natural hacia discos y nubes, que configurar copias sea claro y por pasos (sin esconder nada a quien quiere verlo todo), y probar la primera actualización automática real.

## Principio de diseño: «guiado» y «avanzado»

- **Guiado** (por defecto): un paso cada vez, solo lo necesario, valores por defecto sensatos ya puestos. Los pasos hechos quedan arriba como resumen de una línea que se toca para volver.
- **Avanzado**: un interruptor «Avanzado» muestra todo de golpe (como hoy). La consola recuerda la preferencia por persona.
- Sin tratar al usuario como tonto: etiquetas cortas, sin párrafos; el detalle técnico en un «?» que se abre; teclado completo (Intro avanza, Esc cierra).
- Lo poco habitual de cada paso, bajo «Más opciones» dentro del paso.
- Cambiar algo hecho abre su resumen; se toca la línea del paso y solo se abre ese paso.

## Bloque 1 — Arreglos rápidos

- [ ] **1.1 Desconectar una nube en cualquier equipo.** «Nubes conectadas» en la página de cada equipo (no solo almacenes) con «Desconectar». Anula ese permiso concreto en el proveedor (Dropbox; Drive/OneDrive si se puede; B2/S3: borra las credenciales del equipo y avisa de que la clave sigue viva en el proveedor). Borra restos (credenciales selladas, temporales de rclone). No deja si un repositorio, espejo o copia derivada la usa (dice cuál). Clave de administración; queda en el historial de todas las consolas. Si no se puede anular en ese momento (sin Internet, error del proveedor, permiso caducado), las credenciales del equipo se borran igual y la anulación queda pendiente con reintentos (hasta 7 días); la consola lo dice y, si al final no se pudo, avisa de revisarlo en la web del proveedor.
- [ ] **1.2 Guiones de instalación de Linux.** Si falta minisign (Ubuntu 22.04), descargan el oficial y comprueban su huella fija; esperan al «lock» de las actualizaciones automáticas del sistema; no muestran el aviso de «este equipo guarda copias» si ya es almacén.

## Bloque 2 — Tipos de destino y marcas

- [ ] **2.1 Clasificación.** Tipo (uno): 🖥️ Local · 🏢 Fuera del sitio · ☁️ Nube. Marcas (combinables): 🔒 Inmutable (solo añadir u Object Lock de N días) · ⛓️‍💥 Aislado (desconectado, p. ej. USB rotado). Deducción automática (carpeta del equipo → Local; zona de un almacén → Local + Inmutable desde el equipo; Dropbox/Drive/OneDrive → Nube; B2/S3 con bloqueo → Nube + Inmutable N días), siempre editable; lo marcado por una persona manda y se indica.
- [ ] **2.2 Dónde se ve.** Icono y etiqueta en la cabecera de cada copia, el mapa, la página del destino, las tarjetas de «Repositorios y destinos» y los selectores. Regla 3-2-1-1-0: el «1 fuera» cuenta Fuera del sitio y Nube; el otro «1», Inmutable o Aislado; «Aislado» avisa si no se conecta desde hace N días: la última conexión la **detecta el agente** por la identidad del volumen (número de serie o GUID en Windows, UUID en Linux; varios discos que se rotan, cada uno con su última vez), con el texto «Conecta el medio aislado para comprobar la rotación (última vez visto hace N días)»; sin datos (agente antiguo), «sin datos de conexión», no un fallo.

## Bloque 3 — Editor de copias guiado

- [ ] **3.1 Lista de copias.** Cada copia, tarjeta cerrada con resumen («🕐 Cada hora · 📁 1 carpeta → 🖥️ Repositorio (Almacén · Disco D)»), debajo sus espejos y derivadas con iconos agrupados por almacén; estado y tira 3·2·1·1·0 compacta; asa para ordenar, «Activa» y «⋯» (duplicar, quitar). Un solo botón «+ Añadir».
- [ ] **3.2 Copia de carpetas, guiada.**
  1. **Cuándo:** «Con horario» (plantillas rápidas «Cada hora», «Cada día a las…», «Laborables a las…» y «Personalizar» = editor completo); «En cadena» (solo si la anterior sale bien); «Después de la anterior» (siempre). En estas dos, sin horario: «Inmediatamente» o «Con retraso de N min». La primera copia no puede ir en cadena ni después.
  2. **Qué:** carpetas («Elegir en el equipo»); «Más opciones»: exclusiones (con las habituales), «Solo guardar si hay cambios», «Antes de copiar».
  3. **Dónde:** repositorios existentes por destino con icono; «+ Nuevo repositorio» (nombre, destino con el almacén recomendado arriba, vacío o trayendo las versiones de otro —todas o filtradas—, contraseña generada o escrita → kit); «Avanzado»: nubes del propio equipo con el aviso «Este equipo guardará la credencial de la nube y podrá borrar en ella».
  4. **Resumen:** pocas líneas con «Cambiar»; cómo queda la regla 3-2-1 con sugerencias de un clic; «Guardar» pide la clave una vez para todo lo pendiente.
  «Avanzado» muestra la tarjeta entera como hoy.
- [ ] **3.3 «+ Añadir».** Copiar carpetas de este equipo (3.2) · Espejo (bloque 4) · Copia derivada (versiones de un repositorio, todas o filtradas por etiquetas, carpetas o fechas, a otro repositorio de forma continua; pasos: de qué repositorio → qué versiones → cuándo → dónde → resumen). Dentro de una copia, el espejo y la derivada ya saben de qué repositorio parten.

## Bloque 4 — Espejos como trabajos

- [ ] **4.1 Trabajo de espejo.** Quién lo hace: el almacén (recomendado si el repositorio está ahí) o el propio equipo (repositorios en sus discos; funciona solo con el agente). Qué: todos, los de ciertos equipos, o repositorios concretos. Adónde: otra zona/disco o una nube conectada en quien lo hace. Cuándo: horario, «en cadena» o «después de» otro espejo o de cada copia nueva (con retraso). Retención, freno, verificación sin contraseñas, límite de velocidad, activo/pausado, nombre. Varios trabajos por repositorio, a destinos u horas distintas.
- [ ] **4.2 Retención.** Nunca borra · Sigue al original con retraso de N días · Igual que el origen (sincroniza en la siguiente pasada; aviso «si algo borra en el original, aquí también»). Con bloqueo de objetos de N días: sin «igual que el origen», retraso mayor que N, nunca borrar algo bloqueado.
- [ ] **4.3 Freno configurable.** Si falta de una vez más de un X % de archivos (1–50 %, por defecto 10 %) o un repositorio entero: no borra y avisa, o pide confirmación con la clave (con la espera de seguridad). No se puede apagar. **Mínimo absoluto** para no saltar con repositorios pequeños o nuevos: el porcentaje solo cuenta si el origen tiene más de N archivos (configurable, por defecto 100) y faltan al menos M (por defecto 20); por debajo, solo frena la desaparición de un repositorio entero.
- [ ] **4.4 Editor de espejos guiado.** Qué repositorios → Adónde → Cuándo → Retención → Resumen; «Más opciones» (freno, verificación, velocidad); «Avanzado».
- [ ] **4.5 Dónde se configuran.** Página del almacén → «Espejos»; en cada copia del equipo, sus espejos agrupados por almacén, editables desde ahí (orden al almacén firmada con la clave del cliente; el almacén nunca recibe la contraseña del repositorio; si el almacén no está en esta consola, se dice y se ofrece «Conectar también…»); página del destino → «Usar en un espejo».
- [ ] **4.6 Límites y compatibilidad.** Un espejo no filtra por etiquetas, carpetas o fechas (haría falta la contraseña: para eso, la copia derivada). Los espejos actuales se convierten solos en trabajos equivalentes; con un agente antiguo la consola solo ofrece lo que entiende.

## Bloque 5 — Lo guiado en otras partes

- [ ] Por prioridad: editor de copias (3), editor de espejos (4), Nuevo repositorio y Nuevo destino (mismo estilo de resumen y «Más opciones»), copia externa y derivada (flujos guiados de 3.3), retención (plantillas «Recomendada», «Ahorro de espacio», «Larga duración» y «Personalizar»), Restaurar (repaso visual), página del equipo (lo menos usado plegado).

## Bloque 6 — Primera actualización automática real

- [ ] Uno o dos equipos de prueba con la 0.7.25 instalada a mano, en el anillo «prueba»; publicar la 0.7.26 firmada por el responsable (GitHub y consolas); comprobar que se actualizan solos, informan sanos y la consola lo muestra; vuelta atrás en una máquina virtual de pruebas (nunca en un equipo de cliente) con una versión que falla a propósito; después, el anillo «general».

## Bloque 7 — Despliegue masivo (para instalar muchos equipos)

- [x] **7.1 Código de alta para varios equipos:** (rama `ia/0726-bloque7`; contrato en `api-servidor.md` §4 y «Cambios», v1.4x; guía en `guia-instalacion.md` 5.5) válido para N altas y caduca en X días (por defecto 10 equipos, 7 días), generado en el navegador como el de 9a (el servidor solo guarda su huella); se puede anular; cada alta queda en la auditoría.
- [x] **7.2 Línea de PowerShell** (rama `ia/0726-bloque7`; huella SHA-256 fijada en la línea, también codificada para herramientas de despliegue; la de Linux, con la huella y fuera del historial de bash) (y la de Linux ya existente, con el mismo código): descarga el instalador desde la consola, comprueba su firma o huella, instala en silencio y vincula con el código. Para pegar en cada equipo o usar en una herramienta de despliegue.
- [x] **7.3 «Equipos esperando confirmación»:** (rama `ia/0726-bloque7`) lista con el código de comprobación de cada uno y «Confirmar los que coinciden» (uno a uno o en bloque); nada se da de alta sin confirmar.

## Bloque 8 — Lo mismo en todas las consolas

- [ ] **8.1 Datos comunes del cliente** que se reparten a través de sus equipos (como el nombre, las etiquetas y la observación de cada equipo, v1.56): **colores de las etiquetas**, **catálogo de destinos** (nombre, tipo, marcas y días de bloqueo) y **plantillas de copias**. Cada consola manda los cambios a los equipos del cliente; las demás los leen de sus resúmenes; gana el cambio más reciente por cada dato, diciendo desde qué consola. Las marcas y los tipos de los destinos piden la clave de administración (cuentan en la regla 3-2-1).
- [ ] **8.2 Lo que sigue siendo de cada consola**, con el porqué en `docs/consolas-multiples.md` §6: personas y accesos, canales de aviso y sus reglas, notas internas.
- [ ] **8.3 Primera vez:** si dos consolas ya tienen valores distintos, la consola enseña la diferencia y deja elegir cuál vale para todas; nada se pisa sin preguntar.

## Contrato (compatible hacia atrás)

Destinos: `tipo` (local, fuera, nube), marcas `inmutable`, `aislado`, `bloqueo_dias`. Copias: `inicio` = `horario` | `cadena` | `despues` con `retraso_min` (el `tras` actual se lee como «en cadena»). Espejos: `espejo.trabajos[]` (el formato antiguo se convierte solo). `admite`: espejo hecho por el equipo; `quitar_nube` anula el permiso en el proveedor. La preferencia guiado/avanzado vive en la consola, por persona.

## Pruebas

Rust (conversión de espejos, las tres retenciones con fechas simuladas, freno, bloqueo simulado, cadena frente a después con fallos, espejo del equipo con restic y rclone reales sobre carpetas, anulación del permiso contra un servidor falso). Consola (vectores de clasificación, regla 3-2-1, pasos guiados, equivalencia guiado = avanzado; capturas 1280/375, claro/oscuro). e2e (copia al almacén; espejo a otra zona «igual que el origen» y a una nube simulada «nunca borra»; freno; desconectar la nube; «después de» corre tras un fallo y «en cadena» no). CI Windows y Linux; todas las comprobaciones de `AGENTS.md`.

## Orden

1 → 2 (y 7 en paralelo) → 3 y 4 en paralelo, y 8 después de 2 (coordinados en «Dónde» y en los espejos dentro de la copia) → 5 → integración y pruebas → firma y publicación → 6. Capturas del editor guiado al responsable antes de unirlo.

## Fuera de esta versión

Copia del sistema operativo y discos virtuales; actualización automática del servidor; paso de la app de Dropbox a «producción» (trámite del responsable).

## Decisiones tomadas

Copia directa a la nube como «Avanzado» con aviso; retención elegible por espejo, incluida «igual que el origen»; espejos agrupados por almacén; «en cadena» = solo si la anterior sale bien, «después de» = siempre; tipos Local / Fuera del sitio / Nube y marcas Inmutable / Aislado; «copia derivada»; modo guiado por defecto con interruptor «Avanzado» recordado.
