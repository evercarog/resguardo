// 16q (plan 0.8.3): mide la consola con muchos equipos en un navegador sin ventana
// (Edge o Chrome por el protocolo de depuración). Resultado en docs/rendimiento.md.
//
//   npm run build
//   MOCK_EQUIPOS=200 MOCK_LATENCIA=0 npx vite preview --mode mock --port 4180 --strictPort
//   npx tsx scripts/medir-rendimiento.ts            (otra terminal)
//
// Variables: URL (http://127.0.0.1:4180), NAVEGADOR (ruta de msedge/chrome),
// CLIENTE (el grande del simulador), REPOSO_S (segundos quieto en cada página, 60),
// SALIDA (archivo JSON con todo lo medido). Solo lee: no manda órdenes.
import { spawn } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

const URL_BASE = process.env.URL ?? "http://127.0.0.1:4180";
const CLIENTE = process.env.CLIENTE ?? "0a0e1b2c-0000-4000-8000-0000000000b1";
const REPOSO_S = Number(process.env.REPOSO_S ?? 60);
/** Qué medir: `carga`, `navegacion` y `reposo` (por defecto, todo). */
const PARTES = new Set((process.env.PARTES ?? "carga,navegacion,reposo").split(","));
const NAVEGADOR =
  process.env.NAVEGADOR ??
  ["C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe", "C:/Program Files/Microsoft/Edge/Application/msedge.exe", "/usr/bin/chromium", "/usr/bin/google-chrome"].find((p) => fs.existsSync(p));
if (!NAVEGADOR) throw new Error("No encuentro Edge ni Chrome: pon su ruta en NAVEGADOR.");

const perfil = fs.mkdtempSync(path.join(os.tmpdir(), "medir-consola-"));
const proc = spawn(NAVEGADOR, ["--headless=new", "--remote-debugging-port=0", `--user-data-dir=${perfil}`, "--window-size=1440,900", "--no-first-run", "--no-default-browser-check", "about:blank"], { stdio: "ignore" });
const esperar = (ms: number) => new Promise((r) => setTimeout(r, ms));

let puerto = 0;
for (let i = 0; i < 100 && !puerto; i++) {
  await esperar(100);
  try {
    puerto = Number(fs.readFileSync(path.join(perfil, "DevToolsActivePort"), "utf8").split("\n")[0]);
  } catch {
    /* aún no */
  }
}
if (!puerto) throw new Error("El navegador no abrió su puerto de depuración.");
const paginas = (await (await fetch(`http://127.0.0.1:${puerto}/json/list`)).json()) as { type: string; webSocketDebuggerUrl: string }[];
const ws = new WebSocket(paginas.find((p) => p.type === "page")!.webSocketDebuggerUrl);
await new Promise((ok, mal) => ((ws.onopen = ok), (ws.onerror = mal)));

let siguiente = 0;
const pendientes = new Map<number, (r: { result?: unknown; error?: unknown }) => void>();
const oyentes: ((m: { method: string; params: Record<string, unknown> }) => void)[] = [];
ws.onmessage = (e) => {
  const m = JSON.parse(String(e.data));
  if (m.id && pendientes.has(m.id)) {
    pendientes.get(m.id)!(m);
    pendientes.delete(m.id);
  } else if (m.method) for (const o of oyentes) o(m);
};
async function cdp<T = Record<string, unknown>>(method: string, params: Record<string, unknown> = {}): Promise<T> {
  const id = ++siguiente;
  ws.send(JSON.stringify({ id, method, params }));
  const r = await new Promise<{ result?: unknown; error?: unknown }>((ok, mal) => {
    pendientes.set(id, ok);
    setTimeout(() => pendientes.has(id) && (pendientes.delete(id), mal(new Error(`${method}: sin respuesta en 120 s`))), 120_000);
  });
  if (r.error) throw new Error(`${method}: ${JSON.stringify(r.error)}`);
  return r.result as T;
}
async function evaluar<T>(expresion: string): Promise<T> {
  const r = await cdp<{ result: { value: T }; exceptionDetails?: unknown }>("Runtime.evaluate", { expression: expresion, awaitPromise: true, returnByValue: true });
  if (r.exceptionDetails) throw new Error(JSON.stringify(r.exceptionDetails));
  return r.result.value;
}

// Lo que pasa por la red: cada respuesta con lo transferido (comprimido o no) y lo descomprimido.
interface Peticion {
  url: string;
  tipo: string;
  transferido: number;
  bytes: number;
  codificacion: string;
  inicio: number;
}
const red = new Map<string, Peticion>();
let registro: Peticion[] = [];
oyentes.push((m) => {
  const p = m.params as Record<string, never>;
  if (m.method === "Network.requestWillBeSent") red.set(p.requestId, { url: (p.request as { url: string }).url, tipo: p.type ?? "", transferido: 0, bytes: 0, codificacion: "", inicio: Date.now() });
  else if (m.method === "Network.responseReceived") {
    const x = red.get(p.requestId);
    const h = (p.response as { headers: Record<string, string> }).headers;
    if (x) x.codificacion = h["content-encoding"] ?? h["Content-Encoding"] ?? "";
  } else if (m.method === "Network.dataReceived") {
    const x = red.get(p.requestId);
    if (x) x.bytes += p.dataLength as number;
  } else if (m.method === "Network.loadingFinished") {
    const x = red.get(p.requestId);
    if (x) {
      x.transferido = p.encodedDataLength as number;
      registro.push(x);
      red.delete(p.requestId);
    }
  }
});

await cdp("Network.enable");
await cdp("Page.enable");
await cdp("Performance.enable");
await cdp("Runtime.enable");
// Que la página se crea visible y con el foco (sin ventana, si no, la consola no refresca: pestaña oculta).
await cdp("Emulation.setFocusEmulationEnabled", { enabled: true });
await cdp("Page.bringToFront");

const metricas = async () => Object.fromEntries((await cdp<{ metrics: { name: string; value: number }[] }>("Performance.getMetrics")).metrics.map((x) => [x.name, x.value]));
const esApi = (p: Peticion) => new URL(p.url).pathname.startsWith("/api/");
const suma = (xs: Peticion[], k: "transferido" | "bytes") => xs.reduce((s, x) => s + x[k], 0);

/** Espera a que no queden esqueletos ni peticiones en vuelo (1 s de calma). */
async function listo(max = 60_000) {
  const t0 = Date.now();
  let calma = 0;
  while (Date.now() - t0 < max) {
    const ocupado = await evaluar<boolean>(`document.readyState !== "complete" || !!document.querySelector('[aria-busy="true"]') || !document.querySelector("main")`);
    // Lo que lleva más de 20 s abierto no cuenta (una petición larga que espera cambios).
    const enVuelo = [...red.values()].filter((x) => Date.now() - x.inicio < 20_000).length;
    if (!ocupado && enVuelo === 0) {
      if (!calma) calma = Date.now();
      if (Date.now() - calma >= 1000) return Date.now() - t0 - 1000;
    } else calma = 0;
    await esperar(50);
  }
  const que = await evaluar<string>(`JSON.stringify({ estado: document.readyState, ruta: location.pathname, main: !!document.querySelector("main"), ocupados: [...document.querySelectorAll('[aria-busy="true"]')].map((x) => x.outerHTML.slice(0, 120)) })`);
  throw new Error(`La página no terminó de cargar en ${max / 1000} s: ${que}; en vuelo: ${[...red.values()].map((x) => x.url).join(", ")}`);
}

async function entrar() {
  await cdp("Page.navigate", { url: `${URL_BASE}/entrar` });
  await esperar(1500);
  const r = await evaluar<string>(`(async () => {
    const h = { "Content-Type": "application/json", "X-Resguardo": "1" };
    const a = await fetch("/api/sesion", { method: "POST", headers: h, body: JSON.stringify({ correo: "ana@ejemplo.com", contrasena: "resguardo" }) });
    const b = await fetch("/api/sesion/totp", { method: "POST", headers: h, body: JSON.stringify({ codigo: "123456" }) });
    return a.status + "/" + b.status;
  })()`);
  if (r !== "200/200") throw new Error(`No se pudo entrar en el simulador (${r}).`);
}

interface Medida {
  pagina: string;
  ms: number;
  peticiones: number;
  api: { n: number; transferido: number; bytes: number; mayores: { ruta: string; bytes: number; transferido: number }[] };
  estaticos: { n: number; transferido: number; bytes: number };
  heapMB: number;
  nodos: number;
  scriptMs: number;
  layoutMs: number;
  estiloMs: number;
}
function resumir(pagina: string, ms: number, xs: Peticion[], antes: Record<string, number>, despues: Record<string, number>): Medida {
  const api = xs.filter(esApi);
  const est = xs.filter((x) => !esApi(x));
  return {
    pagina,
    ms,
    peticiones: xs.length,
    api: {
      n: api.length,
      transferido: suma(api, "transferido"),
      bytes: suma(api, "bytes"),
      mayores: api
        .map((x) => ({ ruta: new URL(x.url).pathname.replace(CLIENTE, "{c}"), bytes: x.bytes, transferido: x.transferido }))
        .sort((a, b) => b.bytes - a.bytes)
        .slice(0, 5),
    },
    estaticos: { n: est.length, transferido: suma(est, "transferido"), bytes: suma(est, "bytes") },
    heapMB: Math.round((despues.JSHeapUsedSize / 1048576) * 10) / 10,
    nodos: despues.Nodes,
    scriptMs: Math.round((despues.ScriptDuration - antes.ScriptDuration) * 1000),
    layoutMs: Math.round((despues.LayoutDuration - antes.LayoutDuration) * 1000),
    estiloMs: Math.round((despues.RecalcStyleDuration - antes.RecalcStyleDuration) * 1000),
  };
}

/** Carga completa (como abrir el enlace en una pestaña nueva), sin caché del navegador. */
async function cargaCompleta(nombre: string, ruta: string, sinCache: boolean) {
  await cdp("Network.setCacheDisabled", { cacheDisabled: sinCache });
  await cdp("Page.navigate", { url: "about:blank" });
  await esperar(300);
  red.clear();
  registro = [];
  const antes = await metricas();
  await cdp("Page.navigate", { url: `${URL_BASE}${ruta}` });
  const ms = await listo();
  const m = resumir(nombre, ms, registro, antes, await metricas());
  console.error(`carga ${sinCache ? "sin caché" : "con caché"}: ${nombre} ${ms} ms, API ${Math.round(m.api.bytes / 1024)} KB`);
  return m;
}

/** Navegación dentro de la consola (pulsar un enlace del menú), ya cargada. */
async function navegar(nombre: string, ruta: string) {
  registro = [];
  const antes = await metricas();
  const t0 = Date.now();
  const ok = await evaluar<boolean>(`(() => { const a = document.querySelector('a[href="${ruta}"]'); if (!a) return false; a.click(); return true; })()`);
  if (!ok) throw new Error(`No hay enlace a ${ruta}`);
  await esperar(50);
  const ms = (await listo()) + 50;
  void t0;
  console.error(`navegar: ${nombre} ${ms} ms`);
  return resumir(nombre, ms, registro, antes, await metricas());
}

/** Pulsar un botón por su texto y medir hasta dos cuadros después (lo que tarda en pintarse). */
async function pulsar(texto: string) {
  return evaluar<number>(`(async () => {
    const b = [...document.querySelectorAll("button")].find((x) => x.textContent.trim() === ${JSON.stringify(texto)});
    if (!b) return -1;
    const t0 = performance.now();
    b.click();
    // Lo que tarda Svelte en cambiar la página y el navegador en colocarla (sin pintar:
    // en un navegador sin ventana requestAnimationFrame puede no llegar nunca).
    await Promise.resolve();
    await new Promise((r) => setTimeout(r, 0));
    void document.body.offsetHeight;
    return Math.round(performance.now() - t0);
  })()`);
}

/** Quieta en la página: lo que pide sola (refrescos) en REPOSO_S segundos. */
async function reposo(nombre: string) {
  const visible = await evaluar<string>("document.visibilityState");
  if (visible !== "visible") console.error(`reposo: la página está «${visible}» (no refresca como una pestaña a la vista)`);
  registro = [];
  await esperar(REPOSO_S * 1000);
  const api = registro.filter(esApi);
  const rutas: Record<string, number> = {};
  for (const x of api) {
    const r = new URL(x.url).pathname.replace(CLIENTE, "{c}");
    rutas[r] = (rutas[r] ?? 0) + 1;
  }
  console.error(`reposo: ${nombre} ${api.length} peticiones, ${Math.round(suma(api, "bytes") / 1024)} KB`);
  return { pagina: nombre, visible, segundos: REPOSO_S, peticiones: api.length, transferido: suma(api, "transferido"), bytes: suma(api, "bytes"), rutas };
}

const C = `/c/${CLIENTE}`;
const resultado: Record<string, unknown> = { url: URL_BASE, fecha: new Date().toISOString(), navegador: path.basename(NAVEGADOR) };
try {
  await entrar();
  const frias: Medida[] = [];
  const templadas: Medida[] = [];
  if (PARTES.has("carga")) for (const [n, r] of [
    ["Estado", C],
    ["Equipos", `${C}/equipos`],
    ["Copias", `${C}/copias`],
    ["Todos los clientes", "/todos"],
  ] as const) {
    frias.push(await cargaCompleta(n, r, true));
    templadas.push(await cargaCompleta(n, r, false));
  }
  resultado.cargaSinCache = frias;
  resultado.cargaConCache = templadas;

  // Dentro de la consola: Estado → Equipos → (Mapa) → Copias → Estado.
  const navs: (Medida | { pagina: string; ms: number })[] = [];
  if (PARTES.has("navegacion")) {
    await cargaCompleta("Estado", C, false);
    navs.push(await navegar("Equipos", `${C}/equipos`));
    navs.push({ pagina: "Equipos: pasar a Mapa", ms: await pulsar("Mapa") });
    await esperar(500);
    navs.push({ pagina: "Equipos: volver a Lista", ms: await pulsar("Lista") });
    navs.push(await navegar("Copias", `${C}/copias`));
    navs.push(await navegar("Estado", C));
  }
  resultado.navegacion = navs;

  const quieta: unknown[] = [];
  if (REPOSO_S > 0 && PARTES.has("reposo")) {
    for (const [n, r] of [
      ["Estado", C],
      ["Equipos", `${C}/equipos`],
      ["Copias", `${C}/copias`],
      ["Todos los clientes", "/todos"],
    ] as const) {
      await cargaCompleta(n, r, false);
      quieta.push(await reposo(n));
    }
  }
  resultado.reposo = quieta;
  resultado.memoriaFinalMB = Math.round(((await metricas()).JSHeapUsedSize / 1048576) * 10) / 10;
} finally {
  const salida = process.env.SALIDA;
  const texto = JSON.stringify(resultado, null, 2);
  if (salida) fs.writeFileSync(salida, texto);
  console.log(texto);
  // Cerrar el navegador entero (en Windows, el proceso lanzado no es el que queda vivo).
  try {
    const v = (await (await fetch(`http://127.0.0.1:${puerto}/json/version`)).json()) as { webSocketDebuggerUrl: string };
    const b = new WebSocket(v.webSocketDebuggerUrl);
    await new Promise((ok, mal) => ((b.onopen = ok), (b.onerror = mal)));
    b.send(JSON.stringify({ id: 1, method: "Browser.close" }));
    await esperar(1000);
  } catch {
    /* ya cerrado */
  }
  ws.close();
  proc.kill();
  await esperar(500);
  try {
    fs.rmSync(perfil, { recursive: true, force: true });
  } catch {
    /* el navegador aún suelta archivos */
  }
}
