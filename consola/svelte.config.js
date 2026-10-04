// Consola de Resguardo Server: SPA estática (adapter-static con index.html de
// reserva) que el servidor lleva dentro con rust-embed y sirve en «/».
import adapter from "@sveltejs/adapter-static";
import { vitePreprocess } from "@sveltejs/vite-plugin-svelte";
import fs from "node:fs";

// Versión fija (la del paquete, o RESGUARDO_VERSION): el build sale igual
// cada vez, para que el binario del servidor sea reproducible.
const version = process.env.RESGUARDO_VERSION ?? JSON.parse(fs.readFileSync(new URL("./package.json", import.meta.url), "utf8")).version;

/** @type {import('@sveltejs/kit').Config} */
const config = {
  preprocess: vitePreprocess(),
  kit: {
    // Salida estable: consola/build (lo que el servidor sirve con --consola o mete con rust-embed).
    adapter: adapter({ pages: "build", assets: "build", fallback: "index.html", precompress: false, strict: true }),
    version: { name: version, pollInterval: 0 },
    // Rutas absolutas a /_app: el arranque va en un archivo aparte (ver scripts/arranque-externo.mjs).
    paths: { relative: false },
    // El service worker (src/service-worker.ts) lo registra la consola a mano,
    // solo en producción: en `npm run dev` no se guarda nada.
    serviceWorker: { register: false },
    alias: {
      // Sistema de diseño y piezas compartidas con la app de escritorio.
      $ui: "../ui/src",
    },
    // CSP por <meta> con el hash del arranque en línea (sin 'unsafe-inline' en
    // scripts). 'wasm-unsafe-eval' hace falta para Argon2id (hash-wasm, en un
    // worker). El servidor debe enviar además, por cabecera, lo que <meta> no
    // admite: frame-ancestors 'none' (ver README.md).
    csp: {
      mode: "hash",
      directives: {
        "default-src": ["self"],
        "script-src": ["self", "wasm-unsafe-eval"],
        "worker-src": ["self"],
        // PWA (docs/diseno.md §8): el manifiesto, del propio servidor.
        "manifest-src": ["self"],
        "style-src": ["self", "unsafe-inline"],
        "img-src": ["self", "data:"],
        "font-src": ["self"],
        // Dropbox: el navegador cambia el código de autorización por el token (OAuth con PKCE, sin secreto).
        "connect-src": ["self", "https://api.dropboxapi.com"],
        "object-src": ["none"],
        "base-uri": ["none"],
        "form-action": ["self"],
      },
    },
    typescript: {
      config(cfg) {
        // ui/ no tiene node_modules propio: sus importaciones se resuelven con
        // los de la consola (aquí para TypeScript y en vite.config.ts para Vite).
        cfg.compilerOptions.paths = {
          ...cfg.compilerOptions.paths,
          svelte: ["../node_modules/svelte"],
          "svelte/*": ["../node_modules/svelte/*"],
          "@lucide/svelte": ["../node_modules/@lucide/svelte"],
        };
        cfg.include.push("../../ui/src/**/*.ts", "../../ui/src/**/*.svelte");
        return cfg;
      },
    },
  },
};

export default config;
