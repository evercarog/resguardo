# Plan de mejoras acordado

Lista de trabajo acordada con el responsable del proyecto el 2026-10-05 (sesión de Claude Code en la nube, ver `docs/registro-ia.md`). **Para el asistente que lo retome:** lee antes `AGENTS.md`; haz las tareas en orden, cada una en su rama `ia/<tema>`, con sus pruebas, y marca aquí su estado (`[ ]` pendiente, `[~]` en curso, `[x]` hecha, con la rama). Lo que cambie mensajes entre agente y consola va en `docs/api-servidor.md` → «Cambios». Antes de empezar una tarea de diseño grande (1, 4, 5), escribe la propuesta concreta en su documento de `docs/` y pide el visto bueno.

---

## 0. Revisar y unir lo de la sesión en la nube

- [ ] Revisar la rama `claude/analizar-repo-resguardo-d780qd` (7 commits sobre `main`) y unirla si está bien. Lo que hace y lo que no se pudo probar está en `docs/registro-ia.md` (entrada del 2026-10-05). Resumen:
  - clippy 1.97 (`manual_option_zip`) en `agent.rs`;
  - restic del agente con `PATH` del sistema y `-o rclone.program=`; el proceso del servicio (`--service`, `--primer-plano`) usa solo el restic incluido;
  - restaurar rechaza nombres de dispositivo de Windows (`CON`, `NUL`, `COM1`…);
  - `restaurar-respaldo` protege la carpeta en Windows (ahora pide administrador);
  - la consola valida la línea de Linux antes de enseñarla;
  - **el servidor quita los equipos «Sin confirmar» cuyo código caducó** (un equipo duplicado «Sin confirmar» tras varios intentos de añadirlo).
- [ ] Probar en Windows: compilar todo (`cargo clippy --workspace`, `cargo test --workspace`), `restaurar-respaldo` como administrador, una copia con repositorio `rclone:` o `sftp:` si hay alguna, y que el equipo duplicado desaparece unos 10 min después de arrancar el servidor actualizado.

---

## 1. Órdenes visibles y cancelables desde cualquier consola

**Problema.** Hoy la consola que manda una orden con espera (destructiva) la guarda ella y solo la entrega al equipo cuando llega su `not_before` (`almacen/sqlite.rs`, órdenes `pendiente` con `not_before > ahora`; el agente rechaza las que llegan antes, `orden_v2::validar`). El equipo no sabe que existe, así que **las demás consolas no la ven ni pueden cancelarla**. Tampoco ven el historial de órdenes de las otras (cada una numera las suyas: `seq` por vínculo). Ejemplo: dos «Quitar un repositorio» en espera en la consola local de un cliente que la consola en línea no enseña. Riesgo: alguien con las claves en una consola manda algo destructivo y quien está en la otra no se entera hasta que se aplica.

**Propuesta** (cambio de contrato compatible; diseño en `docs/consolas-multiples.md`):

1. Las órdenes con espera se **entregan al equipo al momento**; el equipo las guarda «en espera» (bajo el cerrojo de las consolas) y las aplica al llegar su `not_before`, contado por él.
2. El **resumen** del equipo lleva `en_espera: [{ id, tipo, descripcion, consola: { nombre, url, identidad }, emitida, aplica }]` y todas las consolas lo enseñan en «Órdenes esperando su turno», diciendo desde qué consola vino.
3. **Cancelar desde cualquier consola** con una orden nueva `cancelar_espera { id }`, inofensiva (no pide clave: cancelar solo aumenta la protección). Pensar el abuso (una consola maliciosa cancela lo legítimo de otra): es reversible y queda en el historial.
4. **Aviso** (consola, correo, push) en todas las consolas cuando entra una orden con espera.
5. **Historial común:** el equipo anota cada orden aplicada (tipo, consola, quién, resultado) en la bitácora que ya reparte a todas (`historial`), y «Órdenes» enseña también las que llegaron de otras consolas.

A decidir: el reloj del equipo pasa a contar la espera (ver el pendiente «Reloj propio del agente» en `docs/plataforma.md` §7.3.1). Mantener también la comprobación en el servidor mientras convivan agentes viejos.

## 2. Equipos que no están en todas las consolas

**Problema.** Un equipo añadido en una consola no llega a las demás: hay que repetir «Conectar también a otra consola…» (pasó con un equipo añadido en la consola local, que no aparecía en la consola en línea).

**Propuesta:**

- La consola sabe qué otras consolas tiene el cliente (resumen de los demás equipos). Al dar de alta un equipo nuevo: «Este equipo solo está en esta consola; los demás también están en `<otra>`» → **Conectar también…**.
- En Estado, un aviso «N equipos no están en todas tus consolas» con el mismo botón.
- Repetir `anadir_consola` en equipos que ya la tienen ya es inofensivo («ya gestiona este equipo»).

## 3. Espejo más flexible

Hoy (`crates/agente/src/espejo.rs`, `nube.rs`): una vez al día a una hora; copia archivo a archivo **todo** el almacén a carpetas de discos del equipo o a Dropbox/Drive con `rclone copy --immutable`; nunca borra; nadie lo verifica.

- [ ] **3a. Cuándo, con la misma flexibilidad que las copias:** el espejo usa el **mismo horario que las copias** (el de `crates/motor/src/plans.rs`: días de la semana con varias horas, cada N minutos dentro de una franja, cada N días, mensual; mismo editor en la consola; la copia externa ya tiene además `tasks::Schedule::AfterBackup`), por destino si hace falta (p. ej. disco cada hora, nube por la noche). Además, **«después de cada copia nueva»**: el almacén ve una versión nueva en `snapshots/` de algún repositorio y lanza el espejo pasados 10–15 min (agrupando varias copias seguidas), se puede combinar con el horario. Compatible: `hora` sigue valiendo y se convierte a un horario de «cada día a esa hora».
- [ ] **3b. Retención del espejo:** que no crezca sin fin. **Borrado diferido**: lo que ya no está en el origen (lo quitó la poda de la retención del almacén) se borra del espejo pasados N días (30 por defecto, configurable; mínimo 7). Freno: si de una vez falta en el origen más de un X % de los archivos (o un repositorio entero), no se borra nada y se avisa. En destinos con Object Lock, respetar el bloqueo.
- [ ] **3c. Más destinos:** Backblaze B2 y S3 (con Object Lock si se puede), SFTP, carpetas de red (NAS por SMB) y WebDAV, todos por rclone con credenciales selladas para el equipo (como Dropbox). Hoy las carpetas de red están prohibidas a propósito (v1.10: SYSTEM no sigue rutas UNC); por rclone con usuario propio se evita ese problema. Varios discos locales del almacén a la vez (comprobar si `destinos` ya lo permite y enseñarlo claro en la consola).
- [ ] **3d. Verificación del espejo sin contraseñas:** los archivos de restic (`data/`, `index/`, `snapshots/`, `keys/`) se llaman como el SHA-256 de su contenido. El espejo puede: comprobar el hash del archivo de **origen antes de copiarlo** (no propagar uno dañado: se avisa) y **verificar los del destino** por rotación (un % cada vez, como la verificación automática). Aviso `espejo_fallido` si algo no cuadra.
- [ ] **3e. Restaurar desde el espejo:** paso guiado en la consola para usar una carpeta o nube del espejo cuando el almacén se pierde (con la contraseña del repositorio del kit), y una prueba que lo cubra.

## 4. Copias derivadas (generalizar la copia externa)

Hoy cada repositorio tiene **una** copia externa (`tasks::Offsite`: `restic copy` a S3, B2, rest, carpeta o `rclone:`, con retención, verificación y freno ante cambios raros), sin Dropbox en la consola.

- [ ] **4a. Dropbox (y las demás nubes de rclone) como destino de la copia externa**, desde la configuración de cada copia: conectar la nube en el **equipo dueño** (como `conectar_nube`, que hoy solo se usa en el almacén) y usar `rclone:` con el rclone incluido.
- [ ] **4b. Varias copias externas por repositorio**, cada una con: destino (otro almacén u otro disco del almacén por rest-server, B2, S3, SFTP, NAS, Dropbox…), **contraseña propia** (la misma u otra, al kit), **retención propia**, horario (incluido **«después de cada copia»**) y verificación.
- [ ] **4c. Filtros** en cada copia derivada: etiquetas, rutas o carpetas y antigüedad («solo las versiones de los últimos 30 días», «solo una al mes»). `restic copy` ya admite `--tag`, `--path` y los ids de versión que se elijan.
- [ ] **4d. En la consola**, el flujo de cada copia de principio a fin: «Documentos (RECEPCION) → almacén D: → espejo E: y Dropbox; copia externa a B2».

## 5. «Un repositorio para todos y dividirlo después»: lo que se decidió

Pregunta: copiar todos los equipos a **un solo** repositorio del almacén y repartirlos después en repositorios distintos (por equipo, etiqueta o fecha, con otras claves).

- **El espejo no puede dividir:** copia archivos cifrados sin abrirlos (no tiene contraseñas, a propósito).
- **Un repositorio compartido por todos los equipos no conviene:** todos tendrían la misma contraseña, así que un equipo comprometido podría leer las copias de todos; hoy cada equipo tiene su usuario y su repositorio en el rest-server (`--private-repos`) y eso lo impide.
- **Lo que sí se hace:** cada equipo con su repositorio (como ahora) y las **copias derivadas** (tarea 4) para repartir: por etiqueta, ruta o fecha, a otros destinos y con otras contraseñas. Si hiciera falta juntar varios equipos en un destino, con una copia derivada de cada uno al mismo repositorio de destino (`restic copy` con `--copy-chunker-params` al crearlo, para deduplicar).

---

## Mientras tanto (sin código)

- Cancelar las órdenes en espera que no se esperaban **desde la consola que las mandó**.
- Conectar el equipo que falta a la consola en línea: en la consola local, el cliente → Servidor → «Conectar también a otra consola…».
- Poner la hora del espejo **después** de la copia que quieres subir (p. ej. copia 21:00, espejo 23:00).
