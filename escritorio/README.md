# escritorio

La app de escritorio sigue, de momento, en la raíz del repositorio:

- interfaz Svelte: `src/`, `static/`, `svelte.config.js`, `vite.config.js`;
- Tauri y backend en Rust: `src-tauri/`.

Moverla aquí exige cambiar las rutas de Vite, de Tauri (`frontendDist`), de los scripts de compilación y de los pasos de publicación, así que se hará en un paso propio, cuando la interfaz compartida (`ui/`) esté separada. Ver [docs/plataforma.md](../docs/plataforma.md), §7.2.
