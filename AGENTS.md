# Guía para asistentes de IA (Gemini CLI, Codex, Claude Code…)

Lee esto entero antes de tocar nada. Resume cómo está hecho Resguardo y las reglas que no se pueden saltar. El detalle está en `CONTRIBUTING.md` y en `docs/`.

## Qué es

Resguardo es una plataforma de copias de seguridad sobre **restic**:

- **Resguardo Agente** (Windows y Linux): hace las copias en cada equipo. `crates/agente`.
- **Resguardo Server**: consola local (on‑prem) con API, canal de agentes y consola web embebida. `crates/servidor` + `consola/` (SvelteKit, Svelte 5 runes).
- **Consola en línea**: el mismo servidor en modo multi‑cliente (`docs/consola-en-linea.md`).
- Un agente puede estar vinculado a **varias consolas a la vez** (`docs/consolas-multiples.md`).
- `crates/motor`: restic, planes, retención. `crates/protocolo`: sobres sellados y firmas (con vectores).
- `src/` + `src-tauri/`: la app de escritorio antigua, **congelada** (no se trabaja en ella; tampoco en la web antigua `resguardo-web`). `ui/`: sistema de diseño común.
- Trabajo pendiente acordado: `docs/plan-mejoras.md`.

Documentos clave: `docs/plataforma.md` (arquitectura), `docs/api-servidor.md` (contrato agente ↔ consola, con la sección «Cambios»), `docs/diseno.md` (diseño y voz de la interfaz), `docs/estabilidad.md`.

## Reglas que no se negocian

1. **Español** en interfaz, comentarios, docs y commits. De tú, frases cortas y claras (ver «Voz» en `docs/diseno.md`). La palabra «restic» no aparece en la interfaz.
2. **Seguridad primero**: nada de secretos en argumentos de procesos, registros, URL ni en la web. Validar todo lo que llega de fuera. Nada de `panic` con datos externos.
3. **Repositorio público**: nunca escribas nombres reales de clientes, equipos, IP, dominios privados ni rutas de clientes en código, pruebas o docs. Usa nombres inventados. `npm run test:sin-referencias` lo comprueba.
4. **Contrato compatible hacia atrás**: si cambias mensajes entre agente y consola, añade campos opcionales (los viejos los ignoran) y anótalo en `docs/api-servidor.md` → «Cambios» como «v1.4x (pendiente de numerar al unir)».
5. **Svelte 5**: un `$effect` nunca debe leer el estado que él mismo carga (provoca bucles de peticiones). Envuelve la carga en `untrack(() => …)`. Hay una comprobación estática en `npm run test:vectores` que lo detecta.
6. **No toques** versiones, instaladores ni publicación (`scripts/build-*.mjs`, `packaging/`, números de versión en `Cargo.toml`). Eso lo hace quien publica.

## Comprobaciones antes de cada commit

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cd consola && npm run check && npm run build && npm run test:vectores && cd ..
npm run test:sin-referencias
```

Si cambias flujos de la consola: `cd consola && npm run e2e`. Para ver la consola sin agentes reales: `cd consola && npm run dev:mock`.

En Windows conviene: `CARGO_BUILD_JOBS=3`, `CARGO_INCREMENTAL=0`.

## Cómo trabajar (para que otra IA entienda luego lo que hiciste)

1. **Nunca trabajes en `main`.** Crea una rama por tema: `git switch -c ia/<tema-corto>` desde `main` actualizado.
2. **Commits pequeños** y con mensaje en español que explique qué y por qué. Al final del mensaje añade una línea `Hecho-con: <nombre de la IA>`.
3. **Anota cada sesión** en `docs/registro-ia.md` (al principio del archivo): fecha, rama, qué pidió el usuario, qué cambiaste, qué quedó sin probar y decisiones dudosas.
4. Sube la rama (`git push -u origin ia/<tema>`) y **no la unas a `main`**: la revisión y la unión las hace otra sesión.
5. Si algo no lo pudiste comprobar, dilo en el registro. No inventes resultados de pruebas.
