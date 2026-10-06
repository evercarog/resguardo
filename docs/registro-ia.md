# Registro de sesiones de IA

Cada sesión de un asistente de IA añade una entrada **al principio** (la más reciente arriba). Ver `AGENTS.md`.

Plantilla:

```md
## AAAA-MM-DD · <IA> · rama `ia/<tema>`

- **Pedido:** qué pidió el usuario.
- **Cambios:** archivos o áreas tocadas y por qué.
- **Comprobado:** qué comprobaciones pasaron.
- **Sin probar / dudas:** lo que falta verificar o decisiones a revisar.
```

## 2026-10-06 · Claude Code (Claude Opus 5.5) · rama `ia/ancla-auditoria`

- **Pedido:** tarea 9b del plan («Ancla externa de la auditoría»), con el usuario fuera y otras sesiones haciendo a la vez las tareas 1 y 3.
- **Cambios:**
  - `crates/protocolo`: `derivaciones::texto_ancla_auditoria` (lo que firma el servidor) y `linea_ancla` (`resguardo-ancla:1:<cliente>:<n>:<creado>:<huella>`).
  - `crates/servidor`: `ancla.rs` (la cabeza de la cadena de un cliente, firmada con la identidad); `ancla` en `hola`, en `tomar` y en un mensaje `{ t: "ancla" }` cada hora por el canal; el resumen diario y semanal lleva el ancla de cada cliente (correo, texto y webhook) con una explicación; historial `auditoria_rehecha` → aviso crítico `auditoria_rehecha` (una vez, si es de los últimos 7 días; si no, ya visto), con el texto escrito por el servidor. Pruebas: `tests/ancla.rs` (una cadena rehecha a mano en el SQLite pasa la verificación de siempre pero el ancla de antes ya no cuadra; aviso una vez, «Este servidor», antiguo ya visto, mal formada descartada), el resumen con el ancla y las huellas comunes con la consola.
  - `crates/agente`: `ancla.rs` (guarda las anclas de cada consola en `privado/anclas-auditoria.json` si las firma la identidad fijada y son de su cliente; la misma entrada con otra huella o un número menor que el mayor visto → bitácora `auditoria_rehecha`, sin la dirección de la consola, y sigue con la cadena nueva). Pruebas de veredictos, firma, persistencia y archivo dañado.
  - Consola: «Comprobar con un ancla» en Actividad (`ComprobarAncla.svelte`; el navegador baja toda la actividad y la recalcula desde la primera entrada), «Ancla de hoy» copiable, aviso y entrada en la Historia del equipo, glosario, simulador; `lib/auditoria.ts` (`leerAncla`, `comprobarAncla`…) con `scripts/vectores-auditoria.ts`. e2e, paso 6c: B guarda el ancla de después de la copia de la consola y, al restaurarla, avisa (`auditoria_rehecha`, «Este servidor…»); «Comprobar con un ancla» con la actividad real del restaurado.
  - Docs: contrato en `api-servidor.md` (§8 y «Cambios», v1.4x), modelo de amenazas y límites en `plataforma.md` §7.3.1, plan marcado.
- **Comprobado** (después de unir `main` con las tareas 1 y 2): `cargo fmt --check`, clippy del espacio de trabajo y con `consola-integrada`, `cargo test --workspace`, consola `check`, `build` y `test:vectores`, `test:sin-referencias` y `npm run e2e` (ver abajo). En el simulador: Actividad («Ancla de hoy», «Comprobar con un ancla» con un ancla buena, otra huella, un número mayor, texto sin ancla y de otro cliente; a 375 px y escritorio), Avisos y la Historia de CAJA-1.
- **Decisiones dudosas:**
  - **Correo:** el ancla va en el resumen diario y semanal (a quien lo recibe, no solo a los propietarios); no hay un correo semanal aparte para los propietarios que lo desactivaron (el semanal está activado por defecto). Si hiciera falta, es otro `Mensaje`.
  - **El equipo solo compara cabezas** (lo que pedía la tarea): una cadena rehecha y alargada antes de que el equipo vea otra ancla no la nota él; lo cubre «Comprobar con un ancla» con un correo. Comprobar el tramo de en medio en el equipo exigiría mandarle las entradas (correos de las personas…): no se hizo. Por eso el ancla va cada hora por el canal (y en cada sondeo).
  - **«Rechazar» la regresión** = avisar una vez a todas las consolas y seguir con la cadena nueva. No se bloquean las órdenes de esa consola: una copia de la consola restaurada a propósito también hace retroceder la cadena (el aviso lo dice) y dejaría el equipo sin gestión.
  - **Restaurar una copia de la consola dispara el aviso** en los equipos que vieron un ancla posterior (crítico, con correo). No se distingue de un servidor que miente, a propósito; el texto dice «Si nadie restauró una copia anterior…».
  - El aviso solo se crea si la entrada es de los últimos 7 días (al subir la bitácora entera a una consola nueva, las antiguas entran vistas). `auditoria_rehecha` va en `GET …/historial` sin `tipo` (una consola anterior la ignora en la Historia), al contrario que `orden` (tarea 1), que solo va pedida.
  - «Comprobar con un ancla» baja toda la actividad (páginas de 1000): con decenas de miles de entradas tarda; no se hizo por tramos.
  - En la unión con `main`, `EntradaHistorial.identidad` (consola) la comparten `orden` y `auditoria_rehecha`; `ahora` (tarea 1) en `hola`/`tomar` es la hora del servidor y no tiene que ver con el `ahora` de una entrada `auditoria_rehecha` (otra cabeza).
- **Sin probar / dudas:** el correo real (solo las muestras y el transporte falso de las pruebas); el mensaje `{ t: "ancla" }` de cada hora por el canal (solo el del `hola` y el de `tomar`, que usan la misma función); un equipo con varias consolas donde solo una rehace la cadena (probado en unidades del agente, no en el e2e).

## 2026-10-06 · Claude Code (Claude Opus 5.5) · rama `ia/ordenes-entre-consolas`

- **Pedido:** tareas 1 («Órdenes visibles y cancelables desde cualquier consola») y 9c («Reloj del equipo en las esperas») del plan. El responsable estaba fuera: propuesta escrita en `docs/consolas-multiples.md` §5 y hecha sin esperar su visto bueno (lo pidió así), compatible con los agentes ya instalados.
- **Cambios:**
  - `protocolo`: orden `cancelar_espera` (inofensiva), campo opcional `por` en el sobre, `validar_con_espera`/`abrir_con_espera`.
  - `servidor`: entrega al momento las órdenes con espera que piden autorización a los agentes con `admite: "ordenes_en_espera"` (lo mira en el resumen guardado); `ahora` en `hola`, `ping` y `tomar`; las `entregada` que esperan siguen en «pendientes»; un resultado firmado de una orden ya cancelada la sustituye y avisa (`cambio_inusual`); aviso nuevo `orden_en_espera`; historial `orden` (solo pedido); `pendientes` cuenta las de otras consolas.
  - `agente`: `espera_v2.rs` (guardar con el sobre sellado en `servidor.bin`, aplicar con los dos relojes en el canal de la consola que la mandó, `resumen.en_espera`, cancelar desde cualquiera o desde su servidor, aviso a las demás, historial común `orden` de todas las órdenes); `procesar` partido en `comprobar` / `autorizar_y_ejecutar`; `largas::guardar_resultado`; prueba de integración `espera_it.rs` con dos servidores reales.
  - `consola`: `lib/espera.ts` (+ `vectores-espera.ts`), «Órdenes esperando su turno» con las de otras consolas y su «Cancelar», «Desde otras consolas» en Órdenes, aviso nuevo, `por` en el sobre, mock, paso 8a2 del e2e.
  - Docs: `consolas-multiples.md` §5, `api-servidor.md` (§1, §4, §5, §6, §8 y «Cambios» v1.4x), `plataforma.md` §7.3.1, `plan-mejoras.md`.
- **Encontrado de paso:** con el flujo anterior, una orden con espera que el servidor entregaba a su hora llegaba **después** de las posteriores (`seq` mayor) y el agente la rechazaba por «antigua». Con agente y servidor nuevos ya no pasa (se entregan en orden); con un agente anterior sigue igual.
- **Decisiones dudosas (a revisar):**
  - La espera la cuentan los dos relojes: el del equipo (con la holgura de 5 min de siempre) **y** el del servidor que la mandó (su `ahora` + reloj monotónico, 2 s de margen por redondeo). Sin `ahora` (servidor anterior) solo el del equipo: ese servidor nunca entrega antes de tiempo. No hay referencia de tiempo firmada independiente (sigue en §7.3.1).
  - Solo se aplica en el canal de la consola que la mandó, justo después de hablar con ella. Si esa consola no vuelve, la orden caduca sin aplicarse (lo seguro, como antes, pero puede sorprender).
  - Autorización comprobada al recibirla (los fallos cuentan para los bloqueos) **y** al aplicarla (si cambió la clave, se rechaza). Inofensivas con espera: no se adelantan ni se guardan. Como mucho 20 por consola.
  - `cancelar_espera` es inofensiva: un técnico (o una consola maliciosa) puede cancelar lo que mandó un administrador desde otra consola. Es reversible y queda en el historial de todas (ver §5.9).
  - A la consola que la mandó, la cancelación llega como `rechazada` con `detalle.cancelada` (un servidor anterior la enseña como rechazada), por `largas` (en la siguiente vuelta del servicio, ≤ 10 s, o cuando vuelva).
  - Nombre de la consola, nunca su dirección (como el progreso de «Mover a otro sitio»); sí su identidad (ya estaba en `resumen.consolas`). La consola no enseña un nombre que parezca una dirección (`nombreConsola`).
  - El aviso `orden_en_espera` va solo a las **demás** consolas (la que la manda ya tiene «Orden destructiva pendiente»); una consola apagada en ese momento no lo recibe por correo/push.
  - Historial `orden`: todas las órdenes salvo las de sesión y `cancelar_espera`, también las rechazadas por clave; de las largas solo se anota `en_marcha`. `por` lo pone la consola (no se puede comprobar).
  - El servidor decide si adelantar por el `admite` del último resumen: si se instala un agente anterior encima de uno nuevo, hasta su primer resumen rechazaría por «todavía no es la hora» las que le lleguen antes.
  - Quitar la consola que la mandó (o desvincular) cancela sus órdenes en espera.
- **Comprobado** (Windows, tras unir `origin/main` 0.7.22): `cargo fmt --check`, `cargo clippy --workspace --all-targets -D warnings` y con `--features consola-integrada`, `cargo test --workspace` (con `espera_it`: dos servidores reales, recibir, ver desde la otra, cancelar desde la otra, aplicar a su hora, cancelar desde la propia, inofensiva no adelantada, clave mal, quitar la consola), consola `check`, `build`, `test:vectores` (con `vectores-espera.ts`), `test:sin-referencias` y `npm run e2e` completo con el paso 8a2 (la en línea ve la destructiva de la local, recibe el aviso, la cancela, nunca se aplica; otra con 25 s de espera no se aplica antes y sí a su hora). El primer e2e encontró que guardar la cancelación como `cancelada` rompía la firma comprobada por la consola: ahora queda `rechazada` con `detalle.cancelada`.
- **Sin probar / dudas:** el reinicio del servicio con órdenes en espera (solo que se guardan y se leen de `servidor.bin`); cambiar de verdad el reloj del equipo o del servidor; Linux; la prueba de integración de Rust va por sondeo (el canal WebSocket lo cubre el e2e); la ventana del equipo no enseña ni cancela las órdenes en espera; `cambiar_servidor` de una consola con órdenes en espera (se cancelan por identidad distinta, sin probar).

## 2026-10-06 · Claude Code (Claude Opus 5.5) · rama `ia/equipos-en-todas-las-consolas`

- **Pedido:** tarea 2 de `docs/plan-mejoras.md` («Equipos que no están en todas las consolas»), con el usuario fuera y otra sesión haciendo a la vez la tarea 1.
- **Cambios:** consola: `lib/consolasCliente.ts` (las otras consolas del cliente, deducidas de `resumen.consolas` de sus equipos, y qué equipos faltan en cada una), `AvisoConsolas.svelte` (en Estado, en la ficha del equipo y al terminar el alta), `ConectarConsola.svelte` (consola esperada y equipos ya elegidos; «ya gestiona este equipo» se enseña como «Ya estaba conectado»; un equipo sin resumen dice «aún no ha informado» en vez de «actualiza el agente»), datos simulados (ALMACEN-SUR no está en la consola en línea), `scripts/vectores-consolas.ts`. Agente: solo una prueba más en `consolas_it.rs` (repetir `anadir_consola` con B ya conectada). e2e: paso 8. Docs: `consolas-multiples.md` §2.5, plan marcado. **Sin cambios de contrato.**
- **Comprobado** (antes y después de unir `origin/main`): `cargo fmt --check`, clippy del espacio de trabajo y con `consola-integrada`, `cargo test --workspace`, consola `check`, `build` y `test:vectores` (con la comprobación de `$effect`), `test:sin-referencias` y `npm run e2e` completo (paso 8 con lo nuevo). En el simulador: Estado, ficha del equipo y fin del alta (tras unir el 9a), a 375 px y escritorio, claro y oscuro; «Conectar también…» completo con un código fabricado de la consola en línea simulada (preelige el equipo, reconoce la identidad, «Hecho» y el aviso desaparece).
- **Decisiones dudosas:**
  - No se sugieren consolas «abandonadas»: sin contacto de ningún equipo en 30 días, o añadidas hace más de 7 y nunca contactadas. Los plazos son a ojo.
  - Aunque el código pegado tenga la misma identidad que ya fijaron los demás equipos (lo que prueba que es esa consola), se sigue pidiendo marcar «He comprobado las huellas»: se enseña un aviso verde, pero no se quita el paso (lo más seguro). Si el código es de otra identidad, solo se avisa (no se bloquea: conectar a otra consola es legítimo).
  - Repetir `anadir_consola` sigue acabando en `fallida` en el agente (no se cambió a `hecha`, para no tocar el contrato ni el caso «identidad de otra consola»); la consola reconoce el texto «ya gestiona este equipo» para enseñarlo como bien. Depende de ese texto.
  - Los equipos en modo local, trasladados o sin confirmar no cuentan. Un equipo con agente anterior cuenta («necesita actualizar el agente») y en Estado no tiene botón si todos los que faltan son así.
  - No hay forma de «no avisar más» de un equipo que se quitó a propósito de una consola: el aviso se queda mientras los demás sigan allí.
  - El aviso de Estado usa todos los equipos del cliente, no el filtro de etiquetas.
- **Sin probar / dudas:** el flujo completo con un código real de otra consola solo se probó en el simulador (código fabricado con la identidad de la consola en línea simulada) y en el e2e por API (el e2e no pasa por la interfaz). El aviso al terminar el alta se vio en el simulador (con «Vincular este servidor»), no con un agente real.

## 2026-10-06 · Claude Code · rama `ia/codigo-desde-el-navegador`

Desde `origin/ia/base-nube`. Tarea 9a de `docs/plan-mejoras.md`.

- **Pedido:** que el código de «Añadir equipo» lo genere el navegador (el servidor solo su hash), que el instalador listo y la línea de Linux los arme el navegador, convivencia con consolas y agentes anteriores, y revisar a fondo el flujo («Demasiados intentos»).
- **Cambios:**
  - `crates/servidor`: `POST …/emparejamientos` y `POST …/instaladores` aceptan `codigo_hash` (el servidor guarda el hash en el índice y `sha256:<hash>` en `emparejamientos.codigo` como marca de «a medias»: así no cambia el esquema ni la lógica de limpieza, anular o alta). `GET …/emparejamientos/{p}` da `codigo_hash`/`codigo_navegador`; `codigo-abierto?navegador=1`; nuevo `GET …/instalador-agente` (genérico, sin cola); `codigo_navegador: true` en `GET /api/servidor`; 409 `codigo_repetido`; `Almacen::codigo_indexado`. Pruebas en `tests/codigo_navegador.rs` (el servidor nunca devuelve el código del navegador en ninguna respuesta).
  - **«Demasiados intentos»**: `POST /api/agente/unirse` contaba también los equipos que se unían bien (20 por IP y hora): con varias instalaciones tras la misma IP (o el instalador, que reintenta `vincular`) se agotaba. Ahora 20 fallos por hora y 300 intentos en total.
  - Confirmar un emparejamiento ya confirmado sin el alta vale (antes, si el alta no salía tras confirmar, reintentar daba «aún no se ha unido» y no había forma de seguir); la consola tampoco vuelve a confirmar.
  - `crates/agente/src/servidor_v2.rs::guardar`: temporal por proceso y reintentos al renombrar (el servicio y `vincular` usaban el mismo temporal; si `vincular` fallaba ahí, el equipo ya estaba unido y el segundo intento del instalador gastaba el código otra vez: un «Sin confirmar» y «código no válido»). Sin prueba propia (es una carrera).
  - `crates/protocolo`: `pairing_code` sin sesgo (ya no usa los bytes de versión/variante del UUID ni `% 31` sin rechazo); `vectors/instalador.json` con la cola exacta.
  - `consola`: `lib/codigo.ts` (generar con `crypto.getRandomValues`; 10 caracteres a mano, 16 en preparados; guardado en `localStorage` hasta el alta, al anular o 8 días), `lib/cola.ts` (la cola, mismos bytes que Rust), `lib/emparejar.ts` (`pedirCodigo` con el navegador), página «Añadir equipo» (instalador listo = genérico + cola en el navegador; línea de Linux; reutiliza el código guardado de un preparado con el mismo nombre; campo para escribir el código si se preparó en otro navegador, comprobado con el hash). Simulador (`dev:mock`) con la forma nueva. e2e: A con instalador listo armado en el navegador (`vincular --instalador`), B con la forma de antes, B en el servidor 2 con el código de 15 min del navegador.
  - Docs: `api-servidor.md` §4 y «Cambios», `plataforma.md` §7.3.1, plan 9a marcado.
- **Comprobado (Windows):** `cargo fmt --check`, `cargo clippy --workspace --all-targets -D warnings`, `cargo clippy -p resguardo-servidor --features consola-integrada`, `cargo test --workspace`, consola `check`, `build`, `test:vectores` (incluye los vectores nuevos), `test:sin-referencias`. Con agentes y servidor de verdad: `npm run e2e` pasó los pasos 1–6b2 (emparejar con instalador armado en el navegador y con la forma de antes, y todo lo que sigue) y falló en 6c, `restaurar-respaldo`, con «Acceso denegado (os error 5)»: desde la sesión en la nube ese comando pide administrador en Windows (`proteger_carpeta`) y esta terminal no lo es; no tiene que ver con este cambio. El paso 7 (código de 15 min del navegador) no llegó a correr ahí, así que se probó con `scripts/e2e/consola-abierta.ts` (dos agentes de verdad emparejados con el código del navegador: bien). En el navegador, con `dev:mock`: código de 15 min (recargar la página lo recupera del navegador), línea de Linux, volver a prepararla (mismo código, sin pedir otro) y terminarla «desde otro navegador» escribiendo el código (uno mal se rechaza).
- **Sin probar / dudas:**
  - La descarga del instalador listo en un navegador de verdad (`Blob` de decenas de MB) y ese instalador en Windows de verdad (NSIS + `vincular --instalador`): la cola se probó con el agente, no con el instalador real.
  - **Decisión a revisar:** el código se guarda en `localStorage` (hasta el alta, al anular u 8 días). Cifrarlo con la clave de administración obligaría a pedirla antes de dar el código. Desde otro navegador hay que escribirlo (Windows: `resguardo-agente leer-instalador`, como administrador, lo enseña) o anular.
  - **Decisión a revisar:** el de 15 min sigue con 10 caracteres (≈ 49,5 bits): un servidor malicioso tiene su SHA-256 y podría buscarlo por fuerza bruta antes del alta (lo frenan el SAS y el «primer alta gana»). Los preparados, 16 (≈ 79 bits). Un hash lento exigiría cambiar `code_hash` en los agentes.
  - **Decisión a revisar:** la marca `sha256:<hash>` en la columna `codigo` en vez de una columna nueva (para no tocar el esquema mientras otra rama cambia `sqlite.rs`, 9g).
  - «Vincular este servidor» y las consolas anteriores siguen con el código del servidor (el servidor tiene que escribirlo en `vincular-local.json`).
  - Cambiar el límite de `unirse` toca la misma función que puede tocar 9e (`ip_real`): solo cambian las claves del límite, no cómo se saca la IP.
  - Si el instalador está firmado, la cola sigue invalidando la firma (ahora la añade el navegador).

## 2026-10-06 · Claude Code · rama `ia/seguridad-9d-9h`

- **Pedido:** tareas 9d, 9e, 9f, 9g y 9h de `docs/plan-mejoras.md` (seguridad y robustez), sobre `ia/base-nube`, una por commit.
- **Cambios:**
  - **9d** (`notificaciones/transporte.rs`, `ajustes.rs`, `mod.rs`): `ResolutorPublico` para ureq (resuelve una vez con el de ureq y rechaza si alguna IP es de la red local; ureq conecta a esas mismas IP) y, para el correo de un cliente, resolución propia y conexión a la IP (el certificado se mira con el nombre). `ip_de_red_local` amplía la lista (multidifusión, reservadas, NAT64, 6to4, Teredo, IPv4 compatibles). `transporte::enviar` recibe el ámbito: los canales del servidor siguen pudiendo ir a la red local (era el permiso que ya existía); los de un cliente, no. Error definitivo, sin reintentos. Doc en `api-servidor.md` (Notificaciones).
  - **9e** (`lib.rs`, `estado.rs`, `main.rs`): `--proxy-red` / `RESGUARDO_PROXY_RED` (CIDR, varias), implica `--detras-de-proxy`; `ip_real` toma la IP más a la derecha de `X-Forwarded-For` que no es de un proxy de confianza; junta todas las cabeceras `X-Forwarded-For`; los proxies de confianza no cuentan para el tope de conexiones por IP. Doc en `servidor-linux.md`, `consola-en-linea.md` y `api-servidor.md`.
  - **9f** (`estado.rs`, `agentes.rs`, `api/sesiones.rs`, `lib.rs`): `UsoRelevos` (AtomicU64): medido al arrancar y en la limpieza, reserva atómica antes de escribir cada trozo, se devuelve al fallar o al borrar el relé.
  - **9g** (`almacen/sqlite.rs`): caché LRU de 128 conexiones por cliente que nunca cierra una en uso.
  - **9h** (`crates/agente/src/platform.rs`): prueba en Windows con `mklink /J`; pasa, la revisión externa se equivocaba y el código no cambia.
- **Comprobado (Windows):** `cargo fmt --all --check`; `cargo clippy --workspace --all-targets -- -D warnings`; `cargo clippy -p resguardo-servidor --features consola-integrada --all-targets -- -D warnings` (con `consola` compilada); `cargo test --workspace` (547 bien, 1 ignorada); `npm run test:sin-referencias`. `npm run e2e` de la consola: bien hasta 6b2 (incluidas las notificaciones por correo, 6a); falla en 6c («restaurar-respaldo»: «Acceso denegado») al parecer porque desde `ia/base-nube` `restaurar-respaldo` protege la carpeta (`proteger_carpeta`, solo SYSTEM y Administradores) y la prueba no corría elevada: la carpeta restaurada quedó sin acceso para el usuario normal (no se pudo borrar sin elevar). Estos cambios no tocan la copia de la consola; no se repitió el e2e en `ia/base-nube` para confirmarlo. Lo de después de 6c no se ejecutó.
- **Sin probar / dudas:**
  - 9d: los canales de un cliente ya no usan el proxy del entorno (`HTTPS_PROXY`); si un servidor solo sale a internet por proxy, sus webhooks y ntfy fallarán (los del servidor siguen igual). Se rechaza el nombre si **alguna** de sus IP es local (no solo se descartan esas). El correo de un cliente se conecta a la primera IP (IPv4 antes), sin probar otras si falla. Sin probar contra un servidor SMTP real.
  - 9e: con `--proxy-red`, si todos los saltos son de confianza se toma el de más a la izquierda. Se rechaza `/0`. Sin probar con un proxy real en otro contenedor.
  - 9f: la limpieza mide la carpeta mientras pueden llegar trozos: un trozo reservado y aún sin escribir puede quedar fuera de la cuenta hasta la siguiente limpieza (como mucho un trozo por subida en curso).
  - 9g: si las 128 están en uso a la vez, se pasa del tope un momento (a propósito, para no abrir dos conexiones al mismo cliente).

## 2026-10-06 · Claude Code (Claude Opus 5.5) · rama `ia/arreglo-e2e-retencion`

- **Pedido:** el e2e de `main` (d73180e) fallaba en el paso 5 esperando la vuelta de la retención en el historial del almacén; buscar la causa y arreglarla.
- **Causa:** no era el código. El e2e usó un `resguardo-agente.exe` del 4 de octubre (anterior a «Retención en detalle»): solo se había recompilado el servidor. Ese agente no anota nada (en el almacén ni siquiera existía `privado/bitacora`). Con los binarios recompilados, `main` pasa el escenario entero.
- **Cambios:** `consola/scripts/e2e/escenario.ts` comprueba antes de empezar que `resguardo-server` y `resguardo-agente` no son anteriores a su código (`src/` y `Cargo.toml` de su crate, `motor` y `protocolo`, por la fecha de los archivos, como cargo) y, si lo son, falla en ese momento diciendo cómo compilar (`RESGUARDO_E2E_BINARIOS_VIEJOS=1` para probarlo igual).
- **Rama de la nube unida** (`claude/analizar-repo-resguardo-d780qd`, como en el `main` local): con ella el paso 5 pasa, pero el 6c fallaba («restaurar-respaldo: Acceso denegado»): `restaurar-respaldo` protegía siempre la carpeta de datos y eso pide administrador, también con `--datos` en una carpeta propia (la del e2e). Ahora protege la de ProgramData (la del servicio) siempre y otra carpeta solo si se ejecuta como administrador (`servicio::es_administrador`).
- **Comprobado:** el aviso salta con un agente viejo; `npm run e2e` completo con binarios nuevos sobre `origin/main` y otra vez con la rama de la nube unida y el arreglo de `restaurar-respaldo`.
- **Sin probar / dudas:** al cambiar de rama cambian las fechas de los archivos y el aviso pide recompilar aunque el código sea igual (lo mismo que haría cargo). `restaurar-respaldo` como administrador sobre ProgramData no se ha probado aquí. Dos ejecuciones del e2e cayeron con `read ECONNRESET` (el servidor del escenario dejó de existir sin escribir nada) mientras corrían a la vez otros e2e en la misma máquina; sin otros, pasó entero: parece choque entre escenarios simultáneos (puertos o procesos), sin confirmar.

## 2026-10-06 · Claude Code · rama `worktree-agent-ae3d01db4ee36f1d3`

- **Pedido:** que un «Mover a otro sitio…» empezado en una consola se vea en todas las consolas del equipo (solo lectura, con quién lo empezó), y corregir «Cambiar ninguna copia…».
- **Cambios:** agente (`progreso_v2::ops`: traer el historial, aplicar la retención y restaurar entran en el progreso de cada canal, con `otra_consola` y `consola`; entrada `historial` en la bitácora; huella del informe al terminar), servidor (tipos y campos nuevos en el progreso; `historial` en el historial del equipo), consola (`MoviendoseAviso`, `lib/mover.ts`, historial «Movido a otro sitio», «Mover a otro sitio…» bloqueado si lo lleva otra consola, paso de las copias en 0/1/N), docs/api-servidor.md (v1.4x), e2e paso 8a.
- **Comprobado:** fmt, clippy (también con consola-integrada), cargo test, consola check/build/test:vectores, test:sin-referencias, e2e completo (paso 8a incluido; una vuelta anterior cayó en el paso 5 por un ECONNRESET ajeno a este cambio).
- **Sin probar / dudas:** el progreso de un paso muy corto puede no llegar a verse en la otra consola (el e2e solo lo anota); entre pasos del movimiento (p. ej. mientras se cambian las copias) el aviso desaparece un momento; las operaciones solo viven en memoria del servicio (si se reinicia, desaparecen con la orden cortada).

## 2026-10-05 · Claude Code (en la nube) · rama `claude/analizar-repo-resguardo-d780qd`

La sesión en la nube solo puede subir a esa rama, no a `ia/<tema>`.

- **Pedido:** analizar el proyecto, pasar las comprobaciones de `AGENTS.md` y cerrar pendientes de seguridad baratos de `docs/plataforma.md` §7.3.1.
- **Cambios:**
  - `crates/agente/src/agent.rs`: `Option::zip` (clippy 1.97 da `manual_option_zip` como error; la CI usa `stable`).
  - `crates/motor`: el agente lanza restic con un `PATH` solo de carpetas del sistema (`proceso::path_del_sistema`) y, con `rclone:` (también de origen en `copy`), `-o rclone.program=` con el rclone que va junto a Resguardo. Solo con `require_bundled` (en pruebas y en la app, como antes). `agente.rs`: `--service` y `--primer-plano` también llaman a `require_bundled`; antes solo `--agent-run` y las tareas, así que el restic que el servicio lanza dentro de su proceso (explorar, restaurar, órdenes) podía salir del `PATH` en Linux.
  - `crates/agente/src/sesiones_v2.rs`: `ruta_local` y `carpeta_destino` rechazan nombres de dispositivo de Windows (`dispositivo_de_windows`).
  - `crates/servidor/src/main.rs`: `restaurar-respaldo` en Windows llama a `proteger_carpeta` antes de restaurar.
  - `consola`: `lineaVincular` (`lib/emparejar.ts`) comprueba código, dirección y huella antes de enseñar la línea de Linux; si no, aviso. Pruebas en `scripts/vectores-emparejar.ts`.
  - `crates/servidor` (segundo pedido: un equipo «Sin confirmar» duplicado que no se va): la limpieza de cada 10 min quita los equipos que se unieron con un código y nunca se confirmaron cuando su emparejamiento ya caducó o se anuló (`equipos_sin_alta`, `quitar_sin_alta` en `lib.rs`, con auditoría `quitar_equipo_sin_alta`). Antes el código caducaba pero el equipo quedaba para siempre y la consola no ofrecía cómo quitarlo.
  - `docs/plan-mejoras.md` (nuevo) y `CLAUDE.md` (apunta a `AGENTS.md` y al plan, como `GEMINI.md`): lo acordado para que lo haga la siguiente sesión (órdenes visibles entre consolas, equipos en todas las consolas, espejo con horario flexible y retención, copias derivadas con Dropbox).
  - `docs/plataforma.md`: esas filas pasan de «Queda» a «Cerrados después».
- **Comprobado (Linux):** `cargo fmt --check`; `cargo clippy -D warnings` y `cargo test` de protocolo, motor, servidor y agente (472 bien, 1 ignorada); clippy de motor, agente y servidor para `x86_64-pc-windows-gnu` (MinGW); consola `check`, `build`, `test:vectores`; `test:sin-referencias`.
- **Sin probar / dudas:**
  - Nada se ejecutó en Windows: `proteger_carpeta` en `restaurar-respaldo` (exige administrador: antes no), el PATH fijo con un repositorio `sftp:` (busca `ssh` en `System32\OpenSSH`) o `rclone:` real.
  - Las pruebas con restic real no corrieron (no hay restic en la máquina).
  - `src-tauri` (app de escritorio) no se compiló aquí.
  - `e2e` de la consola sin ejecutar.
  - En Linux, el servicio en una versión publicada ya no usa un restic del `PATH` (p. ej. `/usr/local/bin`): solo el de `/opt/resguardo-agente` o `/usr/bin/restic`, como dice `docs/agente-linux.md`.
  - Rechazar `CON`, `NUL`… impide restaurar por su ruta un archivo así si existiera.
