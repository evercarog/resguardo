# Cómo contribuir a Resguardo

¡Gracias por querer ayudar! Resguardo está en **desarrollo activo**: el diseño puede cambiar y algunas partes todavía se están moviendo de sitio (ver [docs/plataforma.md](docs/plataforma.md)). Antes de un cambio grande, abre un issue para hablarlo.

*English summary at the end.*

## Antes de empezar

- **Seguridad:** si encuentras una vulnerabilidad, **no abras un issue**. Sigue [SECURITY.md](SECURITY.md).
- **Licencia y CLA:** Resguardo es **AGPL-3.0-or-later** ([LICENSE](LICENSE)). Para aceptar tu contribución necesitamos que aceptes el [CLA](CLA.md) (conservas la propiedad de tu código; nos das una licencia para poder ofrecer también licencias comerciales). En tu primer pull request, marca la casilla y comenta: «He leído el CLA de Resguardo (versión 1.0) y lo acepto para mis contribuciones».
- **Código de conducta:** [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md).
- **Código de terceros:** no copies código de otros proyectos sin decirlo. Si hace falta, indica su origen y su licencia en el pull request; tiene que ser compatible con la AGPL-3.0.

## Estructura del repositorio

| Ruta | Qué hay |
|---|---|
| `crates/protocolo` | Sobres sellados, mensajes firmados, códigos de emparejamiento. Con vectores de prueba (`vectors/`). |
| `crates/motor` | Motor de copias: restic, planes, retención, tamaños. |
| `crates/servidor` | Resguardo Server: API, canal de los agentes y consola embebida. |
| `crates/agente` | Resguardo Agente (Windows y Linux) y lo que comparte con la app de escritorio. |
| `consola/` | Consola web de Resguardo Server (SvelteKit). |
| `ui/`, `escritorio/` | Sistema de diseño común y marcador de la app de escritorio (ver su README). |
| `src/` y `src-tauri/` | La app de escritorio (SvelteKit + Tauri). |
| `packaging/` | Instaladores: NSIS del agente y del servidor (Windows), paquetes y systemd (Linux), Docker. |
| `fuzz/` | Fuzzing de los analizadores del protocolo (cargo-fuzz). |
| `docs/` | Diseño, modelo de amenazas y sistema de diseño. |

## Compilar y probar

Requisitos: Rust estable, Node 22 y, en Windows, las herramientas de compilación de Visual Studio (MSVC) y WebView2.

```sh
npm ci                      # dependencias de la interfaz
npm run check               # svelte-check
npm run dev                 # interfaz en el navegador, con datos simulados (src/lib/mock.ts)
npm run tauri dev           # la app de escritorio completa

cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
node crates/protocolo/vectors/verificar.mjs   # vectores del protocolo en JS
```

- Para compilar la app, Tauri necesita `src-tauri/binaries/restic-x86_64-pc-windows-msvc.exe` y `rest-server-…exe`. Descárgalos con su huella comprobada: `scripts/fetch-restic.ps1` y `scripts/fetch-rest-server.ps1`. El instalador del agente lleva además rclone (espejo en la nube), si está: `scripts/fetch-rclone.ps1`; con él también corre la prueba del espejo con rclone de verdad (un remoto «local», sin cuenta de nube). Los paquetes del agente de Linux (x86_64) llevan los restic, rest-server y rclone oficiales: los baja con sus huellas `scripts/fetch-binarios-linux.sh`.
- La compilación va a `src-tauri/target` (ver `.cargo/config.toml`).
- **Pruebas con un repositorio real** (opcionales): `RESGUARDO_TEST_REPO`, `RESGUARDO_TEST_PASSWORD` y `RESGUARDO_TEST_DATA` (un repositorio y una carpeta de prueba, nunca datos reales), y `RESGUARDO_TEST_REST_SERVER` (ruta a `rest-server.exe`) para la prueba de integración del servidor de copias.
- **Instaladores:** `npm run build:todo` (el del agente y el de la app).
- **Fuzzing:** `cargo +nightly fuzz run sobre_firmado` dentro de `fuzz/` (ver `fuzz/Cargo.toml`).

## Estilo

- **Idioma:** el código, los comentarios, los mensajes de commit, la interfaz y la documentación van **en español** (de tú, frases cortas y claras). Los nombres de funciones y tipos pueden seguir en inglés donde ya lo están.
- **Rust:** `cargo fmt` (líneas de hasta 160, ver `rustfmt.toml`) y `clippy` sin avisos.
- **Interfaz:** Svelte 5 (runes) y el sistema de diseño de [docs/diseno.md](docs/diseno.md). Textos según su sección «Voz».
- **Commits:** pequeños y con sentido propio; el mensaje en español, en presente o infinitivo («Añade…», «Corrige…»), explicando el porqué si no es obvio.
- **Seguridad primero:** nada de secretos en argumentos de procesos, en registros ni en la web; validar todo lo que llega de fuera; nada de `panic` con datos externos (añade el caso al fuzzing o a la prueba de humo si tocas un analizador).
- **Pruebas:** todo cambio de comportamiento lleva su prueba. Si cambias el formato del protocolo, regenera los vectores a propósito (ver `crates/protocolo/src/vectores.rs`) y explícalo.

## Pull requests

1. Una rama por cambio, desde `main`.
2. Que pase la CI (formato, clippy, pruebas, cargo-deny, interfaz).
3. Rellena la plantilla del pull request (incluida la casilla del CLA).
4. Si cambia algo visible, añade capturas y actualiza la ayuda (`src/lib/helpContent.ts`) y las novedades (`src/lib/changelog.ts`).

---

## English summary

Resguardo is under **active development**. Please open an issue before large changes. Security issues go to [SECURITY.md](SECURITY.md), never to public issues. The project is **AGPL-3.0-or-later**, and contributions require accepting the [CLA](CLA.md) (you keep ownership; it lets the project also offer commercial licenses). Code, comments, commit messages and docs are written **in Spanish**. Run `cargo fmt`, `cargo clippy -- -D warnings`, `cargo test --workspace` and `npm run check` before opening a pull request.
