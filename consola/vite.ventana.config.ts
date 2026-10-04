// La ventana del agente (docs/agente-ventana.md): una página pequeña que el
// agente lleva dentro (crates/agente/ventana) y sirve con un protocolo propio.
// `npm run build:ventana` la compila; `npm run dev:ventana` la abre en el
// navegador con datos simulados (para diseñarla sin agente).
import { defineConfig } from "vite";
import { svelte, vitePreprocess } from "@sveltejs/vite-plugin-svelte";
import { fileURLToPath } from "node:url";

const aqui = (p: string) => fileURLToPath(new URL(p, import.meta.url));

export default defineConfig({
  root: aqui("./ventana"),
  // Su propio tsconfig (el de la consola es el de SvelteKit).
  tsconfig: aqui("./ventana/tsconfig.json"),
  base: "./",
  plugins: [svelte({ configFile: false, preprocess: vitePreprocess() })],
  resolve: {
    alias: { $ui: aqui("../ui/src"), $lib: aqui("./src/lib") },
    dedupe: ["svelte", "@lucide/svelte"],
  },
  // Sin preempaquetado en desarrollo (todo es ESM), como la consola.
  optimizeDeps: { disabled: true } as never,
  server: { host: "127.0.0.1", port: 5181, fs: { allow: [aqui("..")] } },
  build: {
    target: "es2022",
    outDir: aqui("../crates/agente/ventana"),
    emptyOutDir: true,
    sourcemap: false,
    assetsDir: "",
    modulePreload: { polyfill: false },
    // Nombres fijos (el agente los mete en el ejecutable con build.rs).
    rollupOptions: {
      output: { entryFileNames: "app.js", chunkFileNames: "[name].js", assetFileNames: "[name][extname]" },
    },
  },
});
