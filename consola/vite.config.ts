import { defineConfig, type Plugin } from "vite";
import { sveltekit } from "@sveltejs/kit/vite";
import { fileURLToPath } from "node:url";

// Modos:
// - `npm run dev:mock`: API simulada en el propio Vite (src/mock), con un
//   «agente» de mentira que abre los sobres de verdad. Para diseñar sin servidor.
// - `npm run dev`: reenvía /api a un Resguardo Server de verdad
//   (RESGUARDO_SERVIDOR, por defecto https://127.0.0.1:8443, con su CA propia).
export default defineConfig(async ({ mode }) => {
  const plugins: Plugin[] = [...(await sveltekit())];
  if (mode === "mock") {
    const { mockApi } = await import("./src/mock/plugin");
    plugins.push(mockApi());
  }
  const target = process.env.RESGUARDO_SERVIDOR ?? "https://127.0.0.1:8443";
  return {
    plugins,
    // Un solo tsconfig para todo (también ui/, que está fuera de la consola).
    tsconfig: "./tsconfig.json",
    resolve: {
      // ui/ vive fuera de la consola y no tiene node_modules: sus importaciones
      // usan las de aquí.
      dedupe: ["svelte", "@lucide/svelte"],
    },
    server: {
      host: "127.0.0.1",
      port: 5180,
      fs: { allow: [fileURLToPath(new URL("..", import.meta.url))] },
      proxy: mode === "mock" ? undefined : { "/api": { target, changeOrigin: false, secure: false } },
    },
    // Sin preempaquetado de dependencias en desarrollo: todas son ESM, y el
    // optimizador buscaría el tsconfig de la raíz del monorepo (el de la app).
    optimizeDeps: { disabled: true } as never,
    ssr: { optimizeDeps: { disabled: true } as never },
    worker: { format: "es" as const },
    build: { target: "es2022", sourcemap: false },
  };
});
