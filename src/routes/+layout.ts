// Tauri doesn't have a Node.js server to do proper SSR
// so we use adapter-static with a fallback to index.html to put the site in SPA mode
// See: https://svelte.dev/docs/kit/single-page-apps
// See: https://v2.tauri.app/start/frontend/sveltekit/ for more info
import { isTauri } from "@tauri-apps/api/core";
import { initAppearance } from "$lib/settings.svelte";

export const ssr = false;

export async function load() {
  // `npm run dev` abierto en un navegador normal: backend simulado para poder
  // diseñar la interfaz sin Rust ni restic. Nunca se incluye en la app final.
  if (import.meta.env.DEV && !isTauri()) {
    const { installMocks } = await import("$lib/mock");
    installMocks();
  }
  initAppearance();
}
