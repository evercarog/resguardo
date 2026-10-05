// Canal en vivo con el servidor (v1.39, docs/api-servidor.md §3 «Canal en
// vivo»): un WebSocket por cliente abierto por el que el servidor avisa de lo
// que cambió (un informe, el progreso, una orden, un aviso, el historial, la
// configuración de un equipo…). Solo trae pistas (tipo e ids): cada pantalla
// vuelve a pedir lo suyo por la API de siempre, sin recargar la página.
//
// - Se abre en el layout del cliente (`conectarVivo`) y se reabre solo, con
//   espera creciente (1 s, 2 s, 4 s… hasta 30 s) y algo de azar.
// - El servidor manda un latido cada 25 s: sin nada en 70 s, se da por caído.
// - Sin canal (servidor anterior, un proxy que no deja pasar WebSocket, sin
//   red), las pantallas siguen preguntando cada pocos segundos como antes
//   (`seguirCambios` lo decide); con canal, solo de respaldo cada minuto.
// - Con la pestaña oculta, lo que llega se acumula y se aplica al volver.
// - Tras reconectar (o si el servidor dice `resync`), se refresca todo: lo que
//   pasó mientras tanto no llegó.
import { app } from "./estado.svelte";

export type TipoCambio = "informe" | "progreso" | "orden" | "avisos" | "historial" | "config" | "equipo" | "resync";

export interface Cambio {
  t: TipoCambio;
  /** El equipo al que se refiere (null o ausente: a todo el cliente). */
  equipo?: string | null;
  orden?: string;
  /** progreso: «empieza», «cambia» o «termina»; orden: su estado nuevo. */
  estado?: string;
}

const TIPOS = new Set<string>(["informe", "progreso", "orden", "avisos", "historial", "config", "equipo", "resync"]);

export const vivo = $state({
  /** El cliente del canal abierto (o que se intenta abrir). */
  cliente: "",
  /** ¿Está abierto y el servidor ya saludó? */
  conectado: false,
  /** Instante (Date.now()) del último mensaje del servidor. */
  ultimo: 0,
});

/** Sin noticias del servidor en este tiempo, el canal se da por caído (latido: 25 s). */
const SIN_LATIDO = 70_000;
/** Se juntan los cambios que llegan seguidos (un informe y el fin del progreso, p. ej.). */
const JUNTAR = 250;
const ESPERA_MAX = 30_000;
/** Con el canal abierto, las pantallas preguntan solo de respaldo, cada tanto. */
export const RESPALDO_CON_VIVO = 60_000;

const oyentes = new Set<(cs: Cambio[]) => void>();
let ws: WebSocket | null = null;
let vuelta = 0;
let intentos = 0;
let reintento: ReturnType<typeof setTimeout> | null = null;
let vigia: ReturnType<typeof setInterval> | null = null;
let pendientes: Cambio[] = [];
let juntando: ReturnType<typeof setTimeout> | null = null;
/** ¿Ya hubo un saludo en este cliente? */
let saludado = false;
/** Cuándo se abrió este cliente (el primer saludo, si llega enseguida, no necesita refrescar nada). */
let abierto = 0;
/** Lo que tarda como mucho el primer saludo para no volver a pedir lo que la pantalla acaba de cargar. */
const RECIEN_CARGADO = 3_000;

/** Oye los cambios del cliente abierto (ya juntados). Devuelve cómo dejar de oír. */
export function alCambiar(f: (cs: Cambio[]) => void): () => void {
  oyentes.add(f);
  return () => oyentes.delete(f);
}

const visible = () => typeof document === "undefined" || document.visibilityState === "visible";

function repartir() {
  juntando = null;
  if (!visible() || !pendientes.length) return;
  const cs = pendientes;
  pendientes = [];
  for (const f of [...oyentes]) {
    try {
      f(cs);
    } catch (e) {
      console.error(e);
    }
  }
}

/** Anota un cambio (también para las pruebas y el modo de demostración). */
export function recibir(c: Cambio) {
  pendientes.push(c);
  // Lo mismo repetido (el progreso, cada 5 s) cuenta una vez.
  if (pendientes.length > 200) pendientes = [{ t: "resync" }];
  juntando ??= setTimeout(repartir, JUNTAR);
}

function alMensaje(n: number, datos: unknown) {
  if (n !== vuelta || typeof datos !== "string") return;
  let m: { t?: string; equipo?: string | null; orden?: string; estado?: string };
  try {
    m = JSON.parse(datos);
  } catch {
    return;
  }
  vivo.ultimo = Date.now();
  if (m.t === "hola") {
    vivo.conectado = true;
    intentos = 0;
    // Tras un corte (o si el canal se abrió más tarde, p. ej. con la pestaña oculta al
    // cargar): lo que pasó mientras tanto no llegó.
    if (saludado || Date.now() - abierto > RECIEN_CARGADO) recibir({ t: "resync" });
    saludado = true;
  } else if (m.t && TIPOS.has(m.t)) {
    recibir({ t: m.t as TipoCambio, equipo: m.equipo ?? null, orden: m.orden, estado: m.estado });
  }
  // «latido» y lo desconocido (un servidor más nuevo): solo cuentan como noticia.
}

function cerrar() {
  if (ws) {
    ws.onopen = ws.onmessage = ws.onclose = ws.onerror = null;
    try {
      ws.close();
    } catch {
      /* ya cerrado */
    }
  }
  ws = null;
  vivo.conectado = false;
}

function programar(n: number, ms: number) {
  if (reintento) clearTimeout(reintento);
  reintento = setTimeout(() => {
    reintento = null;
    if (n === vuelta) abrir(n);
  }, ms);
}

function abrir(n: number) {
  if (n !== vuelta || ws) return;
  const cliente = vivo.cliente;
  if (!cliente || typeof WebSocket === "undefined") return;
  // Hasta saber qué servidor es; uno anterior a v1.39 no tiene canal (se sigue preguntando).
  if (!app.servidor) return programar(n, 2_000);
  if (app.servidor.vivo !== true) return;
  if (!visible()) return; // se abre al volver a la pestaña
  const url = `${location.protocol === "https:" ? "wss" : "ws"}://${location.host}/api/clientes/${encodeURIComponent(cliente)}/vivo`;
  let s: WebSocket;
  try {
    s = new WebSocket(url);
  } catch {
    return programar(n, espera());
  }
  ws = s;
  s.onmessage = (ev) => alMensaje(n, ev.data);
  s.onclose = (ev) => {
    if (n !== vuelta || ws !== s) return;
    ws = null;
    const estaba = vivo.conectado;
    vivo.conectado = false;
    // 4401 / 4403: la sesión terminó o ya no es miembro. Un refresco lo enseña (y lleva a «Entrar»).
    if (ev.code === 4401 || ev.code === 4403) {
      recibir({ t: "resync" });
      return programar(n, ESPERA_MAX);
    }
    if (estaba) intentos = 0;
    programar(n, espera());
  };
}

/** 1 s, 2 s, 4 s… hasta 30 s, con ±20 % de azar (para que no vuelvan todas a la vez). */
function espera() {
  const ms = Math.min(ESPERA_MAX, 1_000 * 2 ** intentos++);
  return Math.round(ms * (0.8 + Math.random() * 0.4));
}

/** Abre el canal en vivo de un cliente. Devuelve cómo cerrarlo. */
export function conectarVivo(cliente: string): () => void {
  const n = ++vuelta;
  cerrar();
  if (reintento) clearTimeout(reintento);
  vivo.cliente = cliente;
  intentos = 0;
  saludado = false;
  abierto = Date.now();
  pendientes = [];
  abrir(n);
  const alVolver = () => {
    if (n !== vuelta) return;
    if (visible()) {
      repartir();
      if (!ws) {
        intentos = 0;
        abrir(n);
      }
    }
  };
  const enLinea = () => {
    if (n === vuelta && !ws) {
      intentos = 0;
      abrir(n);
    }
  };
  document.addEventListener("visibilitychange", alVolver);
  window.addEventListener("online", enLinea);
  if (vigia) clearInterval(vigia);
  vigia = setInterval(() => {
    if (n === vuelta && ws && vivo.conectado && Date.now() - vivo.ultimo > SIN_LATIDO) {
      // Callado demasiado tiempo (un proxy que cortó sin avisar): se cierra y se reabre.
      cerrar();
      programar(n, espera());
    }
  }, 10_000);
  return () => {
    document.removeEventListener("visibilitychange", alVolver);
    window.removeEventListener("online", enLinea);
    if (n !== vuelta) return;
    vuelta++;
    if (reintento) clearTimeout(reintento);
    if (vigia) clearInterval(vigia);
    if (juntando) clearTimeout(juntando);
    reintento = vigia = juntando = null;
    pendientes = [];
    cerrar();
    vivo.cliente = "";
  };
}

/** ¿Le toca a un equipo un cambio? (Los de todo el cliente, y `resync`, a todos.) */
export const tocaEquipo = (c: Cambio, equipo: string) => c.t === "resync" || !c.equipo || c.equipo === equipo;

/**
 * Vuelve a pedir lo de una pantalla cuando llegan cambios que le tocan y, de
 * respaldo, cada `ms` sin canal (como antes) o cada minuto con él. Siempre
 * con la pestaña visible. `ms: 0`: sin respaldo (solo los cambios). Devuelve
 * cómo parar (para `onMount` o `$effect`).
 */
export function seguirCambios(cargar: () => unknown, opciones: { ms: number; toca: (c: Cambio) => boolean }): () => void {
  let ultima = Date.now();
  const hacer = () => {
    ultima = Date.now();
    void cargar();
  };
  const dejar = alCambiar((cs) => {
    if (cs.some((c) => c.t === "resync" || opciones.toca(c))) hacer();
  });
  const t = opciones.ms
    ? setInterval(() => {
        if (!visible()) return;
        const cada = vivo.conectado ? Math.max(opciones.ms, RESPALDO_CON_VIVO) : opciones.ms;
        if (Date.now() - ultima >= cada - 500) hacer();
      }, opciones.ms)
    : null;
  return () => {
    dejar();
    if (t) clearInterval(t);
  };
}
