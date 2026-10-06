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
