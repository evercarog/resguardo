# Plan de mejoras acordado

Lista de trabajo acordada con el responsable del proyecto el 2026-10-05 (sesión de Claude Code en la nube, ver `docs/registro-ia.md`). **Para el asistente que lo retome:** lee antes `AGENTS.md`; haz las tareas en orden, cada una en su rama `ia/<tema>`, con sus pruebas, y marca aquí su estado (`[ ]` pendiente, `[~]` en curso, `[x]` hecha, con la rama). Lo que cambie mensajes entre agente y consola va en `docs/api-servidor.md` → «Cambios». Antes de empezar una tarea de diseño grande (1, 4, 5), escribe la propuesta concreta en su documento de `docs/` y pide el visto bueno.

## Decisiones del responsable (2026-10-05)

- **Congeladas:** la **app de escritorio antigua** (`src/`, `src-tauri/`) y la **web antigua** (repositorio `resguardo-web`, Supabase). No se sigue trabajando en ellas: ni funciones nuevas ni arreglos, salvo que el responsable pida uno concreto.
- **Agente para Mac:** no por ahora. Primero se pule todo lo demás.
- **Se hace todo lo de este plan**, empezando al día siguiente. Orden sugerido: 0 → 10a → 9a → 1 → 2 → 9d–9h → 3 → 7 (con 4) → 8 → 6 → 9b → 10c. Antes de las tareas grandes (1, 7, 8), propuesta escrita y visto bueno.

---

## 0. Revisar y unir lo de la sesión en la nube

- [x] Revisar la rama `claude/analizar-repo-resguardo-d780qd` (7 commits sobre `main`) y unirla si está bien (unida el 2026-10-06; `restaurar-respaldo` protege la carpeta solo como administrador o si es la de por defecto, rama `ia/arreglo-e2e-retencion`). Lo que hace y lo que no se pudo probar está en `docs/registro-ia.md` (entrada del 2026-10-05). Resumen:
  - clippy 1.97 (`manual_option_zip`) en `agent.rs`;
  - restic del agente con `PATH` del sistema y `-o rclone.program=`; el proceso del servicio (`--service`, `--primer-plano`) usa solo el restic incluido;
  - restaurar rechaza nombres de dispositivo de Windows (`CON`, `NUL`, `COM1`…);
  - `restaurar-respaldo` protege la carpeta en Windows (ahora pide administrador);
  - la consola valida la línea de Linux antes de enseñarla;
  - **el servidor quita los equipos «Sin confirmar» cuyo código caducó** (un equipo duplicado «Sin confirmar» tras varios intentos de añadirlo).
- [~] Probar en Windows (2026-10-06: clippy, `cargo test --workspace` y el e2e completo en Windows, bien; falta `restaurar-respaldo` como administrador y una copia `rclone:`/`sftp:` real): compilar todo (`cargo clippy --workspace`, `cargo test --workspace`), `restaurar-respaldo` como administrador, una copia con repositorio `rclone:` o `sftp:` si hay alguna, y que el equipo duplicado desaparece unos 10 min después de arrancar el servidor actualizado.

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
- [ ] **3f. Elegir qué repositorios va a cada destino:** hoy el espejo copia **todo** el almacén a cada destino. Que cada destino tenga su selección: todos (como hoy), o solo algunos repositorios (`<usuario>/<repo>`), por equipo o por cliente, con su horario y su retención propios (p. ej. «contabilidad de RECEPCION y del servidor → B2 cada hora; todo → disco E cada noche»). En el destino cada repositorio sigue en su carpeta (`<destino>/<usuario>/<repo>`). Varios repositorios **en un mismo destino** sí; **fundirlos en un solo repositorio** no se puede con el espejo (copia archivos cifrados sin abrirlos, cada repositorio tiene su clave): eso es una copia derivada (tarea 4) de cada uno al mismo repositorio de destino. Un repositorio nuevo en el almacén: preguntar en la consola si entra en los destinos con selección (por defecto, solo en los de «todos»).
- [ ] **3d. Verificación del espejo sin contraseñas:** los archivos de restic (`data/`, `index/`, `snapshots/`, `keys/`) se llaman como el SHA-256 de su contenido. El espejo puede: comprobar el hash del archivo de **origen antes de copiarlo** (no propagar uno dañado: se avisa) y **verificar los del destino** por rotación (un % cada vez, como la verificación automática). Aviso `espejo_fallido` si algo no cuadra.
- [ ] **3e. Restaurar desde el espejo:** paso guiado en la consola para usar una carpeta o nube del espejo cuando el almacén se pierde (con la contraseña del repositorio del kit), y una prueba que lo cubra.

## 4. Copias derivadas (generalizar la copia externa)

Hoy cada repositorio tiene **una** copia externa (`tasks::Offsite`: `restic copy` a S3, B2, rest, carpeta o `rclone:`, con retención, verificación y freno ante cambios raros), sin Dropbox en la consola.

- [ ] **4a. Dropbox (y las demás nubes de rclone) como destino de la copia externa**, desde la configuración de cada copia: conectar la nube en el **equipo dueño** (como `conectar_nube`, que hoy solo se usa en el almacén) y usar `rclone:` con el rclone incluido.
- [ ] **4b. Varias copias externas por repositorio**, cada una con: destino (otro almacén u otro disco del almacén por rest-server, B2, S3, SFTP, NAS, Dropbox…), **contraseña propia** (la misma u otra, al kit), **retención propia**, horario (incluido **«después de cada copia»**) y verificación.
- [ ] **4c. Filtros** en cada copia derivada: etiquetas, rutas o carpetas y antigüedad («solo las versiones de los últimos 30 días», «solo una al mes»). `restic copy` ya admite `--tag`, `--path` y los ids de versión que se elijan.
- [ ] **4d. En la consola**, el flujo de cada copia de principio a fin: «Documentos (RECEPCION) → almacén D: → espejo E: y Dropbox; copia externa a B2».

- **4e. Descartada** (decisión del responsable): que el almacén haga copias con su propia clave del repositorio. Se usa 7 en su lugar: el almacén nunca tiene contraseñas de los repositorios.
- [ ] **4f. Más de una carpeta servida por el almacén** (opcional, si 4e no basta): que el Servidor de copias ofrezca varias zonas (`D:`, `E:`), cada una como destino elegible para las copias de los equipos (otro rest-server en otro puerto o rutas por zona), para que un equipo pueda copiar directamente a «Almacén · disco E».

## 5. «Un repositorio para todos y dividirlo después»: lo que se decidió

Pregunta: copiar todos los equipos a **un solo** repositorio del almacén y repartirlos después en repositorios distintos (por equipo, etiqueta o fecha, con otras claves).

- **El espejo no puede dividir:** copia archivos cifrados sin abrirlos (no tiene contraseñas, a propósito).
- **Un repositorio compartido por todos los equipos no conviene:** todos tendrían la misma contraseña, así que un equipo comprometido podría leer las copias de todos; hoy cada equipo tiene su usuario y su repositorio en el rest-server (`--private-repos`) y eso lo impide.
- **Lo que sí se hace:** cada equipo con su repositorio (como ahora) y las **copias derivadas** (tarea 4) para repartir: por etiqueta, ruta o fecha, a otros destinos y con otras contraseñas. Si hiciera falta juntar varios equipos en un destino, con una copia derivada de cada uno al mismo repositorio de destino (`restic copy` con `--copy-chunker-params` al crearlo, para deduplicar).

---

## 6. Etiquetas con color elegido y que sirvan para algo

Hoy (`consola/src/lib/etiquetas.svelte.ts`) las etiquetas de los equipos tienen un color automático (hash del nombre, paleta Okabe-Ito de 7 apta para daltonismo, solo en el punto) y sirven para filtrar.

- [ ] **6a. Elegir el color** de cada etiqueta (de la misma paleta, para que siga siendo accesible), guardado en el servidor por cliente; sin elegir, el de ahora.
- [ ] **6b. Que tenga efecto:**
  - agrupar por etiqueta en Estado, Equipos e Informes (un informe por etiqueta, p. ej. «Contabilidad»);
  - **acciones por etiqueta** en «Varios a la vez» (copiar ahora, pausar, aplicar una plantilla de copias a todos los de «Servidores»);
  - **avisos por etiqueta**: a quién se avisa y con qué importancia (p. ej. los de «Servidores», siempre por push);
  - plantilla por defecto por etiqueta: un equipo nuevo con esa etiqueta recibe esa plantilla (pidiendo la clave de administración).
- No confundir con las **etiquetas de las versiones** (las de restic, p. ej. `diaria`, `semanal`), que son las que usan los filtros de las copias derivadas (4c, 4e). Valorar enseñarlas también con color en la lista de versiones.

## 7. Destinos independientes, zonas del almacén y copias en cadena

Decidido después de 3, 4 y 5 (4e se descartó) porque el almacén sigue sin tener contraseñas. Es el modelo que une el espejo y las copias derivadas en algo que se configura en un solo sitio, la copia.

- [ ] **7a. Destinos de primera clase en la consola.** Crear un destino **sin crear un repositorio** (Dropbox, B2, S3, SFTP, NAS, una zona de un almacén), ponerle **nombre** («Almacén · Disco D», «Dropbox Oficina») y cambiarlo cuando quieras. Hoy el destino de un almacén lleva al equipo y no se renombra; en la app el modelo ya existe (`docs/destinos.md`, `places.json` con `name` editable). Al crear un repositorio se elige un destino de la lista.
- [ ] **7b. Varias zonas en un almacén.** Un rest-server sirve **una sola carpeta** (`--path`, `crates/agente/src/server.rs`) y el agente no admite enlaces dentro (seguridad), así que `E:\Backups` necesita **otra instancia de rest-server con su puerto** (mismo certificado de la CA del almacén, solo añadir y `--private-repos`, usuarios propios). La consola lo enseña como zonas del mismo almacén: «Almacén · Disco D» (:8000), «Almacén · Disco E» (:8002), cada una un destino de 7a. Espacio libre por zona, firewall por puerto, todo con la clave de administración.
- [ ] **7c. Copias ordenadas y en cadena.** En «Cambiar las copias», las copias se pueden **ordenar** y una copia puede empezar **«después de la anterior»** (la de arriba): cuando termina bien, empieza la siguiente; si falla, la cadena se para y avisa. Sin horas fijas y sin choques. El horario de siempre sigue disponible para cualquiera de ellas.
- [ ] **7d. Qué hace cada paso de la cadena** (acordado; tres opciones al añadir una copia debajo de otra):
  1. **Copia nueva**: de carpetas del equipo, como ahora.
  2. **Espejo**: el mismo repositorio en otro destino (mismos archivos, misma contraseña). Dos variantes: **con la retención del original** (lo que se poda en el original se quita del espejo pasados N días, 3b) o **sin retención** (nunca borra).
  3. **Repositorio nuevo a partir del anterior**: se crea un repositorio en el destino elegido (con `--copy-chunker-params` del anterior, para que deduplique), se le **traen las versiones** del anterior y desde ahí va por su cuenta, con su propia retención y verificación.
     - **Qué versiones traer**: todas, o filtradas por etiquetas, equipo (host), carpetas y fechas («desde el 1 de enero», «las de los últimos 90 días»). `restic copy` admite `--tag`, `--host`, `--path` e ids; las fechas se resuelven eligiendo los ids.
     - **Contraseña**: **la misma que el anterior** u **otra** (escrita por la persona o generada). Con la misma, son dos repositorios distintos que se abren con la misma contraseña; con otra, uno no abre el otro. La consola recomienda otra para destinos fuera de la oficina, sin impedir la misma. Siempre al kit.
     - **Desde entonces**, una de dos: **traer las versiones nuevas** del anterior cada vez (después de la anterior o con horario, con los mismos filtros), o **copiar directamente las carpetas** del equipo (una copia nueva que apunta a este repositorio).
  Los espejos (opción 2) no necesitan contraseña. Si origen y destino son zonas del mismo almacén, o una nube conectada en él, los hace **el almacén en local**: sin red y sin contraseñas. Para encadenarlos tras una copia de otro equipo, el almacén detecta la versión nueva en `snapshots/` de ese repositorio (3a).
  Ejemplo: «Contabilidad» → Almacén · Disco D; **después** → Almacén · Disco E (espejo con retención o copia independiente); **después** → Dropbox (espejo o copia independiente).
- [ ] **7f. Crear una copia: destino primero, repositorio después.** El flujo de «Añadir una copia»:
  1. **Qué**: carpetas del equipo (copia nueva) o el repositorio de otra copia (paso de una cadena, 7d).
  2. **Cuándo**: horario o «después de la anterior».
  3. **Destino**: de la lista de destinos (7a), o crear uno ahí mismo.
  4. **Repositorio en ese destino**:
     - **Espejo**: no se elige ni se crea nada; el repositorio de destino es el **mismo** (mismos archivos, misma contraseña) en `<destino>/<usuario>/<repo>`.
     - **Copia nueva**: uno **que ya existe** en ese destino (lista) o **uno nuevo** ahí mismo (nombre y contraseña escrita o generada, al kit).
     - **Repositorio nuevo a partir del anterior**: nombre, contraseña (la misma u otra) y qué versiones traer (7d, opción 3).
  5. **Traer versiones** reutiliza `copiar_historial` / «Traer el historial», que ya existe, añadiéndole los filtros.
  6. Resumen con el camino completo (4d) y la clave de administración.
- **Seguridad de 7 (para no perderla al implementar):**
  - Los espejos los hace el almacén en local (sin contraseñas). Los equipos **no** necesitan acceso a la zona de un espejo; si se les da (copias independientes a esa zona), es en **solo añadir** como la zona principal.
  - El espejo **con retención** sigue a la retención del original con retraso y freno (3b). Recomendar en la consola que al menos un destino de la cadena quede **fuera del alcance** de esa retención: un espejo sin retención, una copia independiente con su propia retención o una nube con Object Lock. Si no, un equipo comprometido que llene el repositorio de versiones basura (lo que la retención del almacén ya limita con su margen de 48 h, `docs/compartir.md`) acabaría desplazando las buenas en todos los destinos.
  - «Traer las versiones» lo hace el equipo dueño, que ya tiene las dos contraseñas; el almacén no ve ninguna.
  - Dropbox no es inmutable: decirlo al elegirlo (como hoy en el espejo).
- [ ] **7e. Convivencia con el espejo de hoy.** El espejo global del almacén («todo lo que guarda») sigue funcionando; la consola explica que los espejos por copia (7d) son la forma nueva y ofrece pasar uno a otro.

## 8. La regla 3-2-1 como guía de la consola (3-2-1-1-0)

Decidido: orientar la configuración a que cada copia cumpla la regla. Se usa la forma moderna **3-2-1-1-0**: **3** copias de los datos (contando los originales), en **2** soportes distintos, **1** fuera de la oficina, **1** inmutable o fuera del alcance de los equipos, y **0** errores al verificar y probar la restauración. Guía y recomendación, **nunca obligación**: se puede guardar una configuración que no cumple.

- [ ] **8a. Datos que faltan en los destinos (7a):** cada destino dice **dónde está** (este equipo, otro equipo de la oficina, otra sede, nube) y **si es inmutable** (rest-server en solo añadir desde el punto de vista del equipo, Object Lock, o no: carpeta local, Dropbox). Valores por defecto deducidos del tipo (nube → fuera; carpeta del propio equipo → mismo equipo), editables. **Soporte** = equipo + disco: dos discos del mismo almacén cuentan como 2 soportes, pero la consola avisa de que un fallo del equipo, un robo o un incendio se los lleva a la vez.
- [ ] **8b. Cálculo por copia** (lo que llega a cada carpeta protegida siguiendo la cadena de 7: copia, espejos, repositorios a partir del anterior, copia externa): nº de copias, soportes distintos, fuera de la oficina, inmutable, y verificación + prueba de restauración recientes y correctas. Solo cuentan los destinos **al día** (su último paso correcto dentro de su horario más un margen). Reglas en un solo sitio con pruebas (como `crates/agente/src/protection.rs`, que ya evalúa la «Salud de la protección» de 7 comprobaciones; la 3-2-1-1-0 se integra ahí, no al lado).
- [ ] **8c. En la consola:**
  - en cada copia, una tira «3 · 2 · 1 · 1 · 0» con cada parte cumplida o no y **qué hacer** para cumplirla («Añade un destino fuera de la oficina: Dropbox o B2 → Añadir paso»);
  - al crear o cambiar una copia (7f), el resumen dice cómo queda y propone el paso que falta;
  - en Estado y en Informes, por cliente: cuántas copias cumplen la regla entera y cuáles no (útil para enseñarlo al cliente);
  - un aviso (no urgente) cuando una copia que cumplía deja de cumplir (p. ej. el espejo en la nube lleva 3 días fallando).
- [ ] **8d. Plantilla «3-2-1 recomendada»** al añadir una copia: copia al almacén (zona D) → espejo a otro disco (zona E) → repositorio a partir del anterior en la nube (B2 con Object Lock o Dropbox), cada uno «después de la anterior», con verificación automática y prueba de restauración mensual.

- [ ] **8e. Inmutabilidad local y el almacén recomendado.**
  - **Guía** (`docs/servidor-linux.md` o una nueva `docs/almacen-inmutable.md`): almacén en un contenedor o máquina virtual Debian sobre **Proxmox con ZFS**, con **instantáneas del anfitrión** (cada hora o cada día, con su retención; p. ej. `sanoid` o `zfs-auto-snapshot`) que el almacén no ve ni puede borrar, anfitrión fuera de la red de la oficina y con otras credenciales, ZFS en espejo y `scrub` periódico. Alternativas: Linux endurecido (sin acceso remoto, `chattr +i` con plazo) y discos USB rotados (desconectados). Para un almacén en Windows: fuera del dominio, cuenta de administrador propia, sin escritorio remoto expuesto, y una copia fuera de su alcance (las instantáneas de Windows las borra cualquier administrador).
  - **En la consola:** cada destino puede marcarse como «con instantáneas inmutables fuera de su alcance» o «desconectado» (lo dice la persona; el almacén no puede comprobar lo que hace el anfitrión) y la regla 3-2-1-1-0 (8b) lo cuenta como inmutable, avisando de que es local (no protege de un incendio o un robo de la oficina).
  - **Detectar y enseñar, sin castigar:** el agente dice en su resumen el **sistema operativo** y el **sistema de archivos** de la carpeta de cada destino o zona (NTFS, ReFS, ext4, XFS, ZFS, Btrfs…) y si corre en un contenedor o máquina virtual. La consola lo **enseña como dato** en la ficha del destino. **Nunca resta** en la regla ni en la salud de la protección por usar Windows o un sistema de archivos sin instantáneas: lo que cuenta es lo que la persona marca (instantáneas fuera de su alcance, desconectado, Object Lock). A lo sumo, un enlace discreto a la guía («Cómo añadir instantáneas que el almacén no pueda borrar»), que se puede ocultar.

## 9. Seguridad y robustez pendientes

Lo pendiente de `docs/plataforma.md` §7.3.1 que se acordó hacer, más lo que se confirmó de una revisión externa (Gemini CLI, 2026-10-05). **No se da por cierto lo de la revisión**: cada punto dice qué se comprobó en el código. Cada uno en su rama, con su prueba.

- [x] **9a. El código de «Añadir equipo» lo genera el navegador** (rama `ia/codigo-desde-el-navegador`; ver `docs/registro-ia.md`), no el servidor (hoy el servidor lo crea y lo guarda en claro mientras sirve, hasta 24 h en el instalador «listo»; §7.3.1). La consola lo genera y manda solo su hash; el instalador «listo» y la línea de Linux los arma el navegador. Cambia el contrato con los agentes ya instalados: convivencia de las dos formas. Aprovechar para revisar a fondo ese flujo (falló varias veces con «demasiados intentos» antes de la corrección del equipo «Sin confirmar»).
- [ ] **9b. Ancla externa de la auditoría** (§7.3.1): publicar periódicamente el último hash de la cadena fuera del servidor (al correo de los propietarios y/o en el resumen que guardan los agentes), para detectar que un servidor rehaga la cadena entera.
- [ ] **9c. Reloj del equipo en las esperas** (§7.3.1): se decide junto con la tarea 1 (si el equipo pasa a guardar las órdenes en espera, la espera la cuenta él).
- [x] **9d. Webhooks y ntfy: SSRF por DNS** (rama `ia/seguridad-9d-9h`: `ResolutorPublico` en `transporte.rs`, también el correo de un cliente; sin proxy del entorno para los canales de un cliente) (revisión externa, **confirmado**): `notificaciones/ajustes.rs::es_red_local` solo mira el texto del nombre; `transporte.rs` usa `ureq` sin resolver antes. Un nombre público que resuelve a `127.0.0.1` o a una IP privada pasa. Importa sobre todo en la consola en línea (varios clientes). Arreglo: resolver el nombre y conectar **a la IP comprobada** (un resolutor propio en `ureq` que descarte privadas, bucle local, enlace local, CGNAT y la de metadatos `169.254.169.254`), no resolver dos veces (eso deja la carrera de *DNS rebinding*). Prueba con un nombre que resuelva a `127.0.0.1`.
- [x] **9e. Proxy inverso en otra máquina o contenedor** (rama `ia/seguridad-9d-9h`: `--proxy-red`, `RESGUARDO_PROXY_RED`, la IP más a la derecha que no es de un proxy) (revisión externa, **confirmado**): `lib.rs::ip_real` solo cree `X-Forwarded-For` si la conexión viene de `127.0.0.1`; con el proxy en otro contenedor (Docker, CT de Proxmox), todas las peticiones tienen la IP del proxy y los límites por IP (entrada, fallos de agentes) se comparten: un intruso que falle contraseñas bloquea a todos. Añadir `--proxy-red <CIDR>` (y `RESGUARDO_PROXY_RED`) con las redes de los proxies de confianza; sin ella, como hoy.
- [x] **9f. Tamaño de los relevos** (rama `ia/seguridad-9d-9h`: `estado::UsoRelevos`) (revisión externa, **confirmado**): `agentes.rs::subir_trozo` (async) llama a `uso_relevos`, que recorre la carpeta `relevos/` con E/S síncrona en cada trozo. Llevar la cuenta en memoria (un `AtomicU64` en el estado, recalculado al arrancar y en la limpieza) o, al menos, sacarlo a `spawn_blocking`.
- [x] **9g. Conexiones SQLite por cliente sin tope** (rama `ia/seguridad-9d-9h`: LRU de 128 que no cierra las que están en uso) (revisión externa, **confirmado**, solo importa con muchos clientes en la consola en línea): `almacen/sqlite.rs::conexion` guarda una conexión abierta por cliente para siempre. Caché acotada (LRU, p. ej. 128) que cierre las menos usadas.
- [x] **9h. Uniones de NTFS en `platform.rs::hay_enlace_en_el_camino`** (rama `ia/seguridad-9d-9h`: la prueba pasa, era falso; el código no cambia) (revisión externa, **probablemente falso**): usa `FileType::is_symlink()`, que en Windows (la biblioteca estándar de Rust) es cierto para los puntos de reanálisis «sustitutos de nombre», es decir, enlaces simbólicos **y uniones**; los de OneDrive no lo son, y por eso no se usa `is_reparse_point` (el comentario lo explica). Añadir una prueba en Windows que cree una unión (`mklink /J`) y compruebe que se detecta; solo si falla, cambiarlo.
- Revisado y **no** hace falta: purga de `notif_envios` (ya existe en `almacen/sqlite.rs`); verificación del espejo con SHA-256 (ya es la tarea 3d); DPAPI de máquina (compromiso de diseño documentado en `SECURITY.md`).

## 10. Mantenimiento

- [x] **10a. Pruebas de Windows en cada rama** (rama `ia/ci-windows-ramas`: el trabajo de Windows corre también en `ia/*` y `claude/*`): el trabajo «App completa (Windows)» de `.github/workflows/ci.yml` solo corre en `main`, en PR a `main` o a mano. Lo más delicado (servicio como SYSTEM, tuberías, permisos) es lo que menos se prueba: que corra también en cada push a ramas `ia/*` y `claude/*` (o, si cuesta demasiado tiempo de CI, un trabajo de Windows reducido con `clippy` y `cargo test -p resguardo-agente`).
- [x] **10b. App de escritorio antigua: congelada** (decisión del responsable, ver «Decisiones»). Más adelante, proponer cómo retirarla cuando el agente con su ventana la sustituya; no se borra nada sin su visto bueno.
- [ ] **10c. Versiones desalineadas** (agente y servidor 0.7.x, app 0.6.x, consola 0.1.0, y el `package.json` raíz con la descripción antigua): **no tocar** (regla 6 de `AGENTS.md`); dejarlo anotado para quien publica.

Fuera del plan por ahora (decisión del responsable): agente para Mac, instaladores firmados (de pago), «antes de copiar» para otras bases de datos (MySQL, PostgreSQL) y la actualización automática de los agentes (la revisa él).

## Mientras tanto (sin código)

- Cancelar las órdenes en espera que no se esperaban **desde la consola que las mandó**.
- Conectar el equipo que falta a la consola en línea: en la consola local, el cliente → Servidor → «Conectar también a otra consola…».
- Poner la hora del espejo **después** de la copia que quieres subir (p. ej. copia 21:00, espejo 23:00).
