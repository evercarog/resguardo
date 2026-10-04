# ui

Sistema de diseño y piezas Svelte comunes de la consola de Resguardo Server y (más adelante) la app de escritorio ([docs/diseno.md](../docs/diseno.md)).

- `src/estilos.css`: tokens y primitivas (copia de `src/app.css` de la app; cuando la app lo importe desde aquí, será la única fuente).
- `src/componentes/`: `Logo`, `Modal` (foco atrapado, Esc, transiciones locales), `InfoTip` (recibe la entrada del glosario y el enlace a la ayuda) e `Ilustracion` (los dibujos de línea de los vacíos, errores y logros, con los tokens del tema).
- `src/movimiento.ts`: duraciones que respetan «reducir movimiento».

No tiene `node_modules` propio: lo compila quien lo usa. La consola lo importa como `$ui` y resuelve `svelte` y `@lucide/svelte` con los suyos (`resolve.dedupe` en Vite y `paths` en TypeScript).
