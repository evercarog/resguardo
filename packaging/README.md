# Empaquetado

- `windows/agente.nsi`: instalador «Instalar Resguardo Agente» (NSIS). Lo usa `npm run build:agente` (ver `scripts/build-agente.mjs`).
- El instalador de la app de escritorio lo genera Tauri (`src-tauri/tauri.conf.json`, ganchos en `src-tauri/windows/installer-hooks.nsh`).

- `linux/`: agente y Resguardo Server en Linux. Contiene sus unidades de systemd, sus instaladores (con firma minisign), cómo se construyen el `.deb` y el `.tar.gz` (x86_64 y arm64), el Servidor de copias con nftables y el kit de la consola en internet (`linux/nube/`, `linux/fail2ban/`; ver [docs/consola-en-linea.md](../docs/consola-en-linea.md)). Ver [linux/README.md](linux/README.md).
- `servidor/`: la imagen Docker de Resguardo Server.

Pendiente (ver [docs/plataforma.md](../docs/plataforma.md), §2.1 y §4): el `.rpm`, la publicación firmada (llave minisign del proyecto) y la imagen Docker publicada.
