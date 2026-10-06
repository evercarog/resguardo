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

## 2026-10-06 · Claude Code (Claude Opus 5.5) · rama `ia/ordenes-entre-consolas`

- **Pedido:** tareas 1 («Órdenes visibles y cancelables desde cualquier consola») y 9c («Reloj del equipo en las esperas») del plan. El responsable estaba fuera: propuesta escrita en `docs/consolas-multiples.md` §5 y hecha sin esperar su visto bueno (lo pidió así), compatible con los agentes ya instalados.
- **Cambios:**
  - `protocolo`: orden `cancelar_espera` (inofensiva), campo opcional `por` en el sobre, `validar_con_espera`/`abrir_con_espera`.
  - `servidor`: entrega al momento las órdenes con espera que piden autorización a los agentes con `admite: "ordenes_en_espera"` (lo mira en el resumen guardado); `ahora` en `hola`, `ping` y `tomar`; las `entregada` que esperan siguen en «pendientes»; `rechazada` + `detalle.cancelada` → `cancelada`; un resultado firmado de una orden ya cancelada la sustituye y avisa (`cambio_inusual`); aviso nuevo `orden_en_espera`; historial `orden` (solo pedido); `pendientes` cuenta las de otras consolas.
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
- **Comprobado:** RESULTADOS_PENDIENTES
- **Sin probar / dudas:** el reinicio del servicio con órdenes en espera (solo que se guardan y se leen de `servidor.bin`); cambiar de verdad el reloj del equipo o del servidor; Linux; la prueba de integración de Rust va por sondeo (el canal WebSocket lo cubre el e2e); la ventana del equipo no enseña ni cancela las órdenes en espera; `cambiar_servidor` de una consola con órdenes en espera (se cancelan por identidad distinta, sin probar).

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
