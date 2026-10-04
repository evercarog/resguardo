// @ts-check
/// <reference no-default-lib="true"/>
/// <reference lib="esnext" />
/// <reference lib="webworker" />
/// <reference types="@sveltejs/kit" />
// Service worker mínimo de la consola (docs/diseno.md §8, PWA). Guarda SOLO el
// armazón de la aplicación:
// - al instalarse, lo de `static/` (iconos, manifiesto y la página «Sin
//   conexión»): unos 60 KB;
// - al usarse, los archivos de `_app/immutable` (llevan su hash en el nombre:
//   nunca cambian) según se van pidiendo.
// Nunca guarda respuestas de la API (`/api/…`) ni ninguna página HTML: los
// datos de los clientes no se quedan en el navegador. Si una navegación falla
// por falta de red, enseña `sin-conexion.html`.
// En JavaScript (con tipos JSDoc): la compilación del service worker no lee el
// tsconfig de la consola y daría con el de la raíz del monorepo.
import { build, files, version } from "$service-worker";

const sw = /** @type {ServiceWorkerGlobalScope} */ (/** @type {unknown} */ (self));

const SIN_CONEXION = "/sin-conexion.html";
/** Rutas exactas, todas del propio servidor y ninguna de la API ni HTML de la consola. */
const propia = (/** @type {string} */ r) => r.startsWith("/") && !r.startsWith("/api/") && !r.endsWith("/index.html");
const ESTATICOS = files.filter(propia);
const INMUTABLES = new Set(build.filter(propia));

// El nombre cambia con cada build distinto: así se tira el armazón anterior.
/** @param {string[]} xs */
function huella(xs) {
  let h = 2166136261;
  for (const c of xs.join("\n")) h = Math.imul(h ^ c.charCodeAt(0), 16777619);
  return (h >>> 0).toString(36);
}
const PREFIJO = "resguardo-consola-";
const CACHE = PREFIJO + huella([version, ...ESTATICOS, ...INMUTABLES]);

sw.addEventListener("install", (e) => {
  e.waitUntil(
    caches
      .open(CACHE)
      .then((c) => c.addAll(ESTATICOS))
      .then(() => sw.skipWaiting()),
  );
});

sw.addEventListener("activate", (e) => {
  e.waitUntil(
    caches
      .keys()
      .then((ks) => Promise.all(ks.filter((k) => k.startsWith(PREFIJO) && k !== CACHE).map((k) => caches.delete(k))))
      .then(() => sw.clients.claim()),
  );
});

/** Un archivo inmutable: de la caché o, la primera vez, de la red (y se guarda). */
async function inmutable(/** @type {Request} */ req, /** @type {string} */ ruta) {
  const c = await caches.open(CACHE);
  const guardada = await c.match(ruta);
  if (guardada) return guardada;
  const r = await fetch(req);
  if (r.ok && r.type === "basic") await c.put(ruta, r.clone());
  return r;
}

sw.addEventListener("fetch", (e) => {
  const req = e.request;
  if (req.method !== "GET") return;
  const url = new URL(req.url);
  // Otros orígenes y la API: siempre la red, sin pasar por aquí.
  if (url.origin !== sw.location.origin || url.pathname.startsWith("/api/")) return;

  if (req.mode === "navigate") {
    // Las páginas, siempre de la red (no se guardan). Sin red: «Sin conexión».
    e.respondWith(fetch(req).catch(async () => (await caches.match(SIN_CONEXION)) ?? Response.error()));
  } else if (INMUTABLES.has(url.pathname)) {
    e.respondWith(inmutable(req, url.pathname));
  } else if (ESTATICOS.includes(url.pathname)) {
    e.respondWith(caches.match(url.pathname).then((r) => r ?? fetch(req)));
  }
});
