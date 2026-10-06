// Pruebas de «Añadir equipo»: que abrir o recargar la página no crea códigos
// (src/lib/emparejar.ts) y que la lista de preparados no pide en bucle
// (src/lib/preparados.svelte.ts, con runas: se compila aquí con Svelte y se
// ejecuta su efecto de verdad, contando las peticiones).
//
//   npm run test:vectores
import { mkdirSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import * as compilador from "svelte/compiler";
// Según cómo lo cargue tsx (ESM o CommonJS), viene suelto o en `default`.
const { compileModule } = ((compilador as { default?: typeof compilador }).default ?? compilador) as typeof compilador;
import ts from "typescript";
import { frenar } from "../src/lib/freno";
import { mensajePausa, pausaTras } from "../src/lib/pausa429";
import { codigoAlCargar, esperaDe, lineaVincular, mensajeAlPedir, pedirCodigo, podrasPedirEn, sirve, type CodigoAbierto } from "../src/lib/emparejar";
import type { Preparado } from "../src/lib/tipos";
import { ALFABETO, Codigos, CLAVE_ALMACEN, codigoDeHash, generarCodigo, LARGO_PREPARADO, PLAZO_GUARDADO_MS, type Almacen } from "../src/lib/codigo";
import { cola, nombreArchivo, validarDatos, type DatosInstalador } from "../src/lib/cola";
import { hashCodigo } from "../src/lib/cripto/claves";
import { aB64 } from "../src/lib/cripto/bytes";

let total = 0;
let fallos = 0;
function igual(nombre: string, real: unknown, esperado: unknown) {
  total++;
  const ok = JSON.stringify(real) === JSON.stringify(esperado);
  if (!ok) {
    fallos++;
    console.error(`✗ ${nombre}\n   real:     ${JSON.stringify(real)}\n   esperado: ${JSON.stringify(esperado)}`);
  } else console.log(`✓ ${nombre}`);
}
const cierto = (nombre: string, v: boolean) => igual(nombre, v, true);
const dormir = (ms: number) => new Promise((r) => setTimeout(r, ms));

// --- Códigos: solo se crean al pulsar ---------------------------------------
console.log("\n— Códigos de «Añadir equipo» (lib/emparejar.ts) —");
const ahora = Date.parse("2026-10-04T12:00:00Z");
const en = (min: number) => new Date(ahora + min * 60_000).toISOString();

/** Un servidor de mentira: guarda el código abierto de la cuenta y cuenta las peticiones. */
function servidorFalso() {
  const s = { posts: 0, gets: 0, abierto: null as CodigoAbierto | null };
  const api = {
    codigoAbierto: async () => {
      s.gets++;
      return s.abierto;
    },
    abrir: async () => {
      s.posts++;
      s.abierto = { id: `e${s.posts}`, codigo: `AAAA-BBBB-C${s.posts}`, caduca: en(15), estado: "abierto" };
      return { ...s.abierto, reutilizado: false };
    },
  };
  return { s, api };
}

{
  const { s, api } = servidorFalso();
  // Abrir la página tres veces (o recargarla): solo se pregunta, nunca se crea.
  for (let i = 0; i < 3; i++) igual(`abrir la página (${i + 1}) no crea código`, await codigoAlCargar(api, ahora), null);
  igual("…ni un POST", s.posts, 0);
  // «Generar el código»: uno.
  const a = await pedirCodigo(api, null, ahora);
  igual("pulsar «Generar» crea uno", [s.posts, a.nuevo, a.codigo], [1, true, "AAAA-BBBB-C1"]);
  // Recargar: vuelve el mismo, sin pedir otro; y pulsar otra vez tampoco crea.
  const tras = await codigoAlCargar(api, ahora + 60_000);
  igual("al recargar se enseña el mismo", tras?.codigo, "AAAA-BBBB-C1");
  const b = await pedirCodigo(api, tras, ahora + 60_000);
  igual("pulsar otra vez tras recargar: el mismo, sin POST", [s.posts, b.nuevo, b.codigo], [1, false, "AAAA-BBBB-C1"]);
  // Con la página recargada sin saberlo aún (yaTengo null), tampoco: pregunta antes.
  const c = await pedirCodigo(api, null, ahora + 60_000);
  igual("sin saberlo, pregunta antes de crear", [s.posts, c.codigo], [1, "AAAA-BBBB-C1"]);
  // Le quedan menos de 2 min: ya no se ofrece; se crea otro al pulsar.
  igual("casi caducado: no se ofrece", await codigoAlCargar(api, ahora + 14 * 60_000), null);
  const d = await pedirCodigo(api, null, ahora + 14 * 60_000);
  igual("casi caducado: pulsar crea otro", [s.posts, d.codigo], [2, "AAAA-BBBB-C2"]);
  // Ya unido: se ofrece para comprobar hasta que caduca.
  igual("unido: se ofrece", sirve({ id: "x", codigo: "X", caduca: en(1), estado: "unido" }, ahora), true);
  igual("unido y caducado: no", sirve({ id: "x", codigo: "X", caduca: en(-1), estado: "unido" }, ahora), false);
}
{
  // Un servidor anterior (sin la ruta: 404) o sin red: la página sigue, sin crear nada.
  let posts = 0;
  const api = {
    codigoAbierto: async (): Promise<CodigoAbierto | null> => {
      throw Object.assign(new Error("No existe."), { estado: 404 });
    },
    abrir: async () => (posts++, { id: "e1", codigo: "X", caduca: en(15) }),
  };
  igual("servidor anterior: al cargar, nada", await codigoAlCargar(api, ahora), null);
  igual("…y sin POST", posts, 0);
}

console.log("\n— Límite de códigos (429 con retry_after) —");
const err429 = (s: number) => Object.assign(new Error("Demasiados intentos."), { estado: 429, cuerpo: { error: "demasiados_intentos", retry_after: s } });
igual("espera de un 429", esperaDe(err429(1234)), 1234);
igual("otro error: sin espera", esperaDe(Object.assign(new Error("x"), { estado: 422 })), null);
igual("en minutos, hacia arriba", podrasPedirEn(61), "Podrás pedir otro código en 2 min.");
igual("menos de un minuto: 1 min", podrasPedirEn(5), "Podrás pedir otro código en 1 min.");
igual("una hora", podrasPedirEn(3600), "Podrás pedir otro código en 1 h.");
cierto("el mensaje explica por qué (seguridad) y cuándo", /seguridad/.test(mensajeAlPedir(err429(600))) && /en 10 min/.test(mensajeAlPedir(err429(600))));
igual("otro error: el del servidor", mensajeAlPedir(new Error("Sistema no válido.")), "Sistema no válido.");
igual("el límite general no es el de códigos", esperaDe(Object.assign(new Error("Demasiadas peticiones."), { estado: 429, cuerpo: { limite: "cuenta", retry_after: 20 } })), null);
igual("el de códigos, sí", esperaDe(Object.assign(new Error("x"), { estado: 429, cuerpo: { limite: "codigos", retry_after: 20 } })), 20);

console.log("\n— Pausa tras un 429 general (lib/pausa429.ts) —");
igual("límite por cuenta: pausa lo que diga retry_after", pausaTras(429, "GET", { limite: "cuenta", retry_after: 17 }, 1000), 18_000);
igual("límite por IP, también en un POST", pausaTras(429, "POST", { limite: "ip", retry_after: 5 }, 0), 5_000);
igual("límite de una acción (códigos): no para lo demás", pausaTras(429, "POST", { limite: "codigos", retry_after: 3600 }, 0), null);
igual("servidor anterior, GET: 30 s", pausaTras(429, "GET", { error: "demasiados_intentos" }, 0), 30_000);
igual("servidor anterior, POST: no", pausaTras(429, "POST", {}, 0), null);
igual("como mucho 2 min", pausaTras(429, "GET", { limite: "cuenta", retry_after: 99999 }, 0), 120_000);
igual("otro error: nada", pausaTras(500, "GET", {}, 0), null);
cierto("el texto dice cuánto", /en 18 s/.test(mensajePausa(18_000, 0)));

// --- La lista de preparados no pide en bucle ----------------------------------
console.log("\n— Lista de preparados (lib/preparados.svelte.ts) —");
const raiz = join(dirname(fileURLToPath(import.meta.url)), "..");
const salida = join(raiz, "node_modules", ".vectores-emparejar");
mkdirSync(salida, { recursive: true });
/** Compila un módulo con runas para el cliente y lo deja junto a node_modules (para que encuentre `svelte`). */
function compilar(nombre: string, fuente: string): string {
  // Svelte no quita los tipos de un módulo: primero TypeScript → JavaScript.
  const sinTipos = ts.transpileModule(fuente, { compilerOptions: { target: ts.ScriptTarget.ESNext, module: ts.ModuleKind.ESNext } }).outputText;
  const { js } = compileModule(sinTipos, { generate: "client", filename: nombre.replace(/\.ts$/, ".js") });
  // En Node, «svelte» es la versión del servidor; las runas compiladas usan la del cliente.
  const destino = join(salida, nombre.replace(/\.svelte\.ts$/, ".mjs"));
  writeFileSync(destino, js.code.replace(/from ['"]svelte['"]/g, 'from "svelte/internal/client"').replace(/from ['"]\.\/preparados\.svelte['"]/g, 'from "./preparados.mjs"'));
  return pathToFileURL(destino).href;
}
compilar("preparados.svelte.ts", readFileSync(join(raiz, "src/lib/preparados.svelte.ts"), "utf8"));
// La página, como la usa +page.svelte: un $effect que depende solo del cliente.
const pagina = await import(
  compilar(
    "pagina.svelte.ts",
    `import { Preparados } from "./preparados.svelte";
export function montar(pedir, alUnirse) {
  const prep = new Preparados();
  let cliente = $state("c1");
  const fin = $effect.root(() => {
    $effect(() => {
      const c = cliente;
      if (!c) return;
      return prep.seguir({ pedir: () => pedir(c), alUnirse, cadaMs: 200, visible: () => true });
    });
  });
  return { prep, fin, cambiar: (c) => (cliente = c) };
}
// Lo de antes (para comprobar que esta prueba lo habría pillado): el efecto leía la lista.
export function montarAntes(pedir) {
  let lista = $state(null);
  async function cargarLista() {
    try {
      const antes = new Map((lista ?? []).map((p) => [p.id, p.estado]));
      const nueva = await pedir("c1");
      lista = nueva;
    } catch {
      lista ??= [];
    }
  }
  return $effect.root(() => {
    $effect(() => {
      void cargarLista();
      const t = setInterval(() => cargarLista(), 5000);
      return () => clearInterval(t);
    });
  });
}`,
  )
);
const svelte = await import(pathToFileURL(join(raiz, "node_modules/svelte/src/internal/client/index.js")).href);

/** Como fetch: la respuesta llega en otra vuelta del bucle de eventos, siempre una lista nueva. */
function contador(estadoDe: () => Preparado["estado"] = () => "abierto") {
  const n = { peticiones: 0, porCliente: {} as Record<string, number> };
  const pedir = (c: string) =>
    new Promise<Preparado[]>((r) => {
      n.peticiones++;
      n.porCliente[c] = (n.porCliente[c] ?? 0) + 1;
      setTimeout(() => r([{ id: "p1", nombre: "SRV-DATOS", so: "linux", estado: estadoDe(), caduca: en(600), creado: en(0), equipo: null }]), 1);
    });
  return { n, pedir };
}

{
  const antes = contador();
  const fin = montarAntes(antes.pedir);
  for (let i = 0; i < 30; i++) {
    svelte.flush();
    await dormir(10);
  }
  fin();
  cierto(`control: el efecto de antes pedía en bucle (${antes.n.peticiones} peticiones en 300 ms)`, antes.n.peticiones > 10);
}
function montarAntes(pedir: (c: string) => Promise<Preparado[]>): () => void {
  return pagina.montarAntes(pedir);
}
{
  let estado: Preparado["estado"] = "abierto";
  const { n, pedir } = contador(() => estado);
  const unidos: string[] = [];
  const p = pagina.montar(pedir, (x: Preparado) => unidos.push(x.nombre));
  for (let i = 0; i < 30; i++) {
    svelte.flush();
    await dormir(10);
  }
  // 300 ms con el sondeo cada 200 ms: la primera carga y, como mucho, una o dos del sondeo.
  cierto(`abrir la página: sin bucle (${n.peticiones} peticiones en 300 ms)`, n.peticiones >= 1 && n.peticiones <= 3);
  igual("la lista llega", p.prep.lista?.map((x: Preparado) => x.nombre), ["SRV-DATOS"]);
  // Se une: un aviso, una vez.
  estado = "unido";
  await dormir(450);
  svelte.flush();
  igual("al unirse, un aviso", unidos, ["SRV-DATOS"]);
  // Otro cliente: el efecto se rehace (una carga más), sin bucle.
  const antes = n.peticiones;
  p.cambiar("c2");
  for (let i = 0; i < 10; i++) {
    svelte.flush();
    await dormir(10);
  }
  cierto(`cambiar de cliente: una carga (${n.peticiones - antes})`, n.peticiones - antes >= 1 && n.peticiones - antes <= 2 && (n.porCliente.c2 ?? 0) >= 1);
  p.fin();
  const parado = n.peticiones;
  await dormir(450);
  igual("al salir de la página deja de preguntar", n.peticiones, parado);
}
{
  // Dos cargas a la vez (el sondeo y «Anular» a la vez): una sola petición.
  const { n, pedir } = contador();
  const { prep } = pagina.montar(() => new Promise<Preparado[]>(() => {}), undefined);
  await Promise.race([Promise.all([prep.cargar({ pedir: () => pedir("c1") }), prep.cargar({ pedir: () => pedir("c1") })]), dormir(50)]);
  igual("dos cargas a la vez: una petición", n.peticiones, 1);
}

// --- Freno de las cargas por avisos del canal (lib/freno.ts) ----------------
console.log("\n— Freno de las cargas de fondo (lib/freno.ts) —");
{
  // Un equipo que parpadea: 500 avisos en ~1 s. Con 100 ms entre cargas: unas 10, no 500.
  let cargas = 0;
  let aLaVez = 0;
  let maxALaVez = 0;
  const f = frenar(async () => {
    cargas++;
    maxALaVez = Math.max(maxALaVez, ++aLaVez);
    await dormir(30);
    aLaVez--;
  }, 100);
  for (let i = 0; i < 500; i++) {
    f.pedir();
    if (i % 25 === 0) await dormir(50);
  }
  await dormir(300);
  cierto(`500 avisos seguidos: ${cargas} cargas (≤ 1 cada 100 ms)`, cargas >= 2 && cargas <= 14);
  igual("nunca dos cargas a la vez", maxALaVez, 1);
  const antes = cargas;
  f.parar();
  f.pedir();
  await dormir(200);
  igual("parado, no carga más", cargas, antes);
}
{
  // Lo último que se pidió siempre se carga (al final), aunque llegue en medio de una carga.
  let cargas = 0;
  const f = frenar(async () => {
    cargas++;
    await dormir(50);
  }, 0);
  f.pedir();
  await dormir(10);
  f.pedir();
  f.pedir();
  await dormir(200);
  igual("lo pedido durante una carga: una más al final", cargas, 2);
}

// --- Ningún $effect lanza una carga siguiendo lo que lee -------------------
// El fallo de «Añadir equipo»: un $effect que llamaba a una carga que leía (y luego
// escribía) estado. Toda carga que lance un efecto va dentro de `untrack(…)` o en un
// helper que ya lo hace (seguir, seguirCambios, conectarVivo, vigilarProgreso).
console.log("\n— Cargas lanzadas desde $effect (sin seguir lo que leen) —");
{
  const src = join(raiz, "src");
  const archivos: string[] = [];
  const recorrer = (d: string) => {
    for (const e of readdirSync(d, { withFileTypes: true })) {
      const p = join(d, e.name);
      if (e.isDirectory()) recorrer(p);
      else if (/\.svelte$|\.svelte\.ts$/.test(e.name)) archivos.push(p);
    }
  };
  recorrer(src);
  const malos: string[] = [];
  let efectos = 0;
  for (const a of archivos) {
    const t = readFileSync(a, "utf8");
    for (const m of t.matchAll(/\$effect(?:\.pre)?\(/g)) {
      // El cuerpo del efecto: hasta cerrar el paréntesis.
      let i = m.index! + m[0].length;
      let n = 1;
      const ini = i;
      while (i < t.length && n > 0) {
        if (t[i] === "(") n++;
        else if (t[i] === ")") n--;
        i++;
      }
      efectos++;
      const cuerpo = t.slice(ini, i);
      // Fuera de untrack: quitamos lo que va dentro de untrack(…) y miramos si queda una carga.
      const sinUntrack = cuerpo.replace(/untrack\(\(\)\s*=>[\s\S]*$/m, "");
      if (/\b(void\s+)?(cargar\w*|asegurarIndice|comprobarLlaves|recargar\w*)\(/.test(sinUntrack)) malos.push(`${a.slice(raiz.length + 1)}:${t.slice(0, m.index).split("\n").length}`);
    }
  }
  igual(`${efectos} efectos: ninguno lanza una carga siguiendo lo que lee`, malos, []);
}

// --- La línea de Linux: nada que la shell interprete ---------------------------
console.log("\n— Línea de Linux (lib/emparejar.ts) —");
{
  const huella = Array.from({ length: 32 }, (_, i) => (i * 7).toString(16).padStart(2, "0").toUpperCase()).join(":");
  igual("línea preparada", lineaVincular("ABCD-EFGH-JK", "https://192.168.1.20:8443", huella), `sudo resguardo-agente vincular ABCD-EFGH-JK --servidor https://192.168.1.20:8443 --huella-ca ${huella}`);
  igual("línea sin huella", lineaVincular("ABCD-EFGH-JK", "https://consola.ejemplo.com"), "sudo resguardo-agente vincular ABCD-EFGH-JK --servidor https://consola.ejemplo.com");
  igual("detrás de un proxy, con ruta", lineaVincular("ABCD-EFGH-JK", "https://ejemplo.com/resguardo"), "sudo resguardo-agente vincular ABCD-EFGH-JK --servidor https://ejemplo.com/resguardo");
  igual("http solo a este equipo", lineaVincular("ABCD-EFGH-JK", "http://localhost:5173"), "sudo resguardo-agente vincular ABCD-EFGH-JK --servidor http://localhost:5173");
  igual("IPv6", lineaVincular("ABCD-EFGH-JK", "https://[fd00::1]:8443"), "sudo resguardo-agente vincular ABCD-EFGH-JK --servidor https://[fd00::1]:8443");
  const malos: [string, string, string | undefined][] = [
    ["ABCD;rm -rf /", "https://s:8443", undefined],
    ["ABCD-EFGH-JK", "https://s:8443;curl x|sh", undefined],
    ["ABCD-EFGH-JK", "https://s:8443 $(id)", undefined],
    ["ABCD-EFGH-JK", "https://s:8443\nreboot", undefined],
    ["ABCD-EFGH-JK", "https://s/`id`", undefined],
    ["ABCD-EFGH-JK", "http://192.168.1.20:8443", undefined],
    ["ABCD-EFGH-JK", "http://localhost.otro.com", undefined],
    ["ABCD-EFGH-JK", "https://", undefined],
    ["ABCD-EFGH-JK", "https://s:8443", "AB:CD"],
    ["ABCD-EFGH-JK", "https://s:8443", huella + ";id"],
    ["ABC", "https://s:8443", undefined],
  ];
  for (const [c, s, h] of malos) igual(`sin línea: ${JSON.stringify([c, s, h])}`, lineaVincular(c, s, h), "");
}

// --- v1.4x: el código lo genera el navegador ---------------------------------
console.log("\n— Códigos generados en el navegador (lib/codigo.ts) —");
{
  const a = generarCodigo();
  cierto(`a mano: «ABCD-EFGH-JK» (${a})`, new RegExp(`^[${ALFABETO}]{4}-[${ALFABETO}]{4}-[${ALFABETO}]{2}$`).test(a));
  const b = generarCodigo(LARGO_PREPARADO);
  cierto(`preparado: 16 en 4 grupos (${b})`, new RegExp(`^([${ALFABETO}]{4}-){3}[${ALFABETO}]{4}$`).test(b));
  cierto("cabe en la cola y en la línea (8–20 con guiones)", b.length <= 20 && lineaVincular(b, "https://s:8443") !== "");
  cierto("dos seguidos no se repiten", generarCodigo() !== generarCodigo());
  // Sin sesgo: los bytes ≥ 248 se descartan (aquí, todos los 255 y luego 0 → «A»).
  let llamadas = 0;
  const azar = (n: number) => (llamadas++ === 0 ? new Uint8Array(n).fill(255) : new Uint8Array(n).fill(31 + 1));
  igual("descarta los bytes de más (sin sesgo)", generarCodigo(10, azar), "BBBB-BBBB-BB");
  igual("…y pide más azar", llamadas, 2);
  // Reparto: con 31 000 letras, cada una entre 700 y 1300 veces.
  const cuenta = new Map<string, number>();
  for (let i = 0; i < 1000; i++) for (const ch of generarCodigo(32).replace(/-/g, "")) cuenta.set(ch, (cuenta.get(ch) ?? 0) + 1);
  cierto("todas las letras salen, más o menos igual", cuenta.size === 31 && [...cuenta.values()].every((n) => n > 700 && n < 1400));
  igual("el hash es el del agente (code_hash)", hashCodigo("abcd efgh-jk"), hashCodigo("ABCD-EFGH-JK"));
  cierto("el código escrito a mano vale con su hash", codigoDeHash(" abcd efgh jk ", hashCodigo("ABCD-EFGH-JK")));
  cierto("…otro no", !codigoDeHash("ABCD-EFGH-JL", hashCodigo("ABCD-EFGH-JK")));
  cierto("…ni sin hash", !codigoDeHash("ABCD-EFGH-JK", undefined));
  cierto("…ni con cosas raras", !codigoDeHash("ABCD;EFGH-JK", hashCodigo("ABCDEFGHJK")));
}
{
  // Dónde se guardan: por emparejamiento, comprobados con su hash, y se olvidan.
  const datos = new Map<string, string>();
  const almacen: Almacen = { getItem: (k) => datos.get(k) ?? null, setItem: (k, v) => void datos.set(k, v), removeItem: (k) => void datos.delete(k) };
  let t = ahora;
  const cs = new Codigos(almacen, () => t);
  cs.guardar({ id: "e1", cliente: "c1", codigo: "ABCD-EFGH-JK" });
  cs.guardar({ id: "p1", cliente: "c1", codigo: "WXYZ-2345-6789-ABCD", nombre: "Recepción", so: "windows" });
  igual("se recupera por su id", cs.de("e1"), "ABCD-EFGH-JK");
  igual("…comprobado con su hash", cs.de("e1", hashCodigo("ABCD-EFGH-JK").toUpperCase()), "ABCD-EFGH-JK");
  igual("…con otro hash, no", cs.de("e1", hashCodigo("OTRO-CODI-GO")), null);
  igual("otro id, nada", cs.de("e9"), null);
  igual("el preparado por nombre (sin mayúsculas) y sistema", cs.preparado("c1", " RECEPCIÓN ", "windows")?.id, "p1");
  igual("…de otro sistema, no", cs.preparado("c1", "Recepción", "linux"), null);
  igual("…de otro cliente, no", cs.preparado("c2", "Recepción", "windows"), null);
  cs.olvidar("e1");
  igual("olvidado (alta hecha o anulado)", cs.de("e1"), null);
  t += PLAZO_GUARDADO_MS + 1;
  igual("a los 8 días, fuera", cs.de("p1"), null);
  datos.set(CLAVE_ALMACEN, "{no es json");
  igual("si lo guardado está roto, nada (sin romperse)", new Codigos(almacen, () => t).de("p1"), null);
  const lleno: Almacen = { getItem: () => null, setItem: () => { throw new Error("QuotaExceededError"); }, removeItem: () => {} };
  const cm = new Codigos(lleno, () => ahora);
  cm.guardar({ id: "m1", cliente: "c1", codigo: "MMMM-NNNN-PP" });
  igual("sin sitio en el navegador: en memoria (esta pestaña)", cm.de("m1"), "MMMM-NNNN-PP");
}
{
  // El servidor solo ve el hash: pedirCodigo con `navegador`.
  const guardados = new Map<string, string>();
  const vistos: unknown[] = [];
  let abierto: CodigoAbierto | null = null;
  let repetir = 1;
  const api = {
    codigoAbierto: async () => abierto,
    abrir: async (h?: string) => {
      vistos.push(h);
      if (repetir-- > 0) throw Object.assign(new Error("Ese código ya existe."), { estado: 409 });
      abierto = { id: "n1", codigo: null, codigo_hash: h, codigo_navegador: true, caduca: en(15), estado: "abierto" };
      return { id: "n1", caduca: en(15), reutilizado: false, codigo_navegador: true };
    },
    navegador: { generar: () => generarCodigo(), hash: hashCodigo, de: (id: string, h?: string | null) => (guardados.has(id) && (!h || hashCodigo(guardados.get(id)!) === h) ? guardados.get(id)! : null), guardar: (id: string, c: string) => void guardados.set(id, c) },
  };
  const r = await pedirCodigo(api, null, ahora);
  igual("409 (hash repetido): otro código, una vez", vistos.length, 2);
  cierto("al servidor solo le llega el hash (64 hex), nunca el código", vistos.every((v) => typeof v === "string" && /^[0-9a-f]{64}$/.test(v)) && !vistos.includes(r.codigo));
  igual("el código queda en este navegador", guardados.get("n1"), r.codigo);
  igual("…y su hash es el que tiene el servidor", hashCodigo(r.codigo), vistos[1]);
  igual("al recargar, este navegador lo vuelve a enseñar", (await codigoAlCargar(api, ahora + 60_000))?.codigo, r.codigo);
  guardados.clear();
  igual("en otro navegador (sin el código): no se enseña", await codigoAlCargar(api, ahora + 60_000), null);
  const otra = { ...api, abrir: async (h?: string) => (vistos.push(h), { id: "n2", caduca: en(15), reutilizado: false, codigo_navegador: true }) };
  const r2 = await pedirCodigo(otra, null, ahora + 60_000);
  igual("…y «Generar» da otro, suyo", [r2.id, guardados.get("n2") === r2.codigo], ["n2", true]);
  // Un servidor que no entendiera el hash y diera su propio código: se usa ese.
  const viejo = { ...api, codigoAbierto: async () => null, abrir: async () => ({ id: "v1", codigo: "VVVV-WWWW-XX", caduca: en(15), reutilizado: false }) };
  igual("servidor que ignora el hash: su código", (await pedirCodigo(viejo, null, ahora)).codigo, "VVVV-WWWW-XX");
  // Hay que tener el código para seguir.
  igual("sin el código no «sirve»", sirve({ id: "x", codigo: null, caduca: en(10), estado: "abierto" }, ahora), false);
}

console.log("\n— Cola del instalador en el navegador (lib/cola.ts ↔ crates/protocolo/vectors/instalador.json) —");
{
  const v = JSON.parse(readFileSync(join(dirname(fileURLToPath(import.meta.url)), "..", "..", "crates", "protocolo", "vectors", "instalador.json"), "utf8")) as {
    casos: { datos: DatosInstalador; cola: string }[];
    malos: DatosInstalador[];
  };
  for (const c of v.casos) igual(`los mismos bytes que Rust: «${c.datos.nombre}»`, aB64(cola(c.datos)), c.cola);
  for (const m of v.malos) cierto(`no vale (como en Rust): ${JSON.stringify(m).slice(0, 90)}`, validarDatos(m) !== null);
  igual("los buenos valen", v.casos.map((c) => validarDatos(c.datos)), v.casos.map(() => null));
  let lanzo = false;
  try {
    cola({ ...v.casos[0].datos, codigo: "AB;rm" });
  } catch {
    lanzo = true;
  }
  cierto("una cola con datos que no valen no se arma", lanzo);
  // Como `nombre_archivo` (api/instaladores.rs).
  igual("archivo", nombreArchivo("Altamar", "SERVIDOR-01"), "Resguardo-Agente_Altamar_SERVIDOR-01.exe");
  igual("archivo con tildes", nombreArchivo("Clínica Señora", "Recepción 2"), "Resguardo-Agente_Clinica-Senora_Recepcion-2.exe");
  igual("archivo sin trucos", nombreArchivo('a"b/../c', "x\r\ny"), "Resguardo-Agente_a-b-c_x-y.exe");
  igual("archivo sin cliente", nombreArchivo("", "PC"), "Resguardo-Agente_PC.exe");
  igual("archivo sin nada", nombreArchivo("", "///"), "Resguardo-Agente.exe");
}

console.log(`\n${total - fallos}/${total} bien`);
if (fallos) process.exit(1);
process.exit(0);
