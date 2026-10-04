// «Todos los clientes» (v1.3x): lo que se enseña de todos los clientes de la
// cuenta, y lo que está en marcha en ellos casi en vivo.
//
// - Todo, con `GET /api/panel` (una petición): al entrar, cada 30 s con la
//   pestaña visible y en cuanto termina algo que estaba en marcha.
// - Lo que está en marcha, con `GET /api/panel/progreso`: cada 3 s mientras
//   hay algo y cada 12 s si no (como en cada cliente, lib/progreso.svelte.ts).
// - Con un servidor anterior (404), de las rutas de cada cliente (resumen,
//   informes y progreso), igual que si se abriera cada uno.
import * as api from "./api";
import { app, cargarClientes } from "./estado.svelte";
import { enFondo } from "./actividad.svelte";
import { claveTarea } from "./progreso.svelte";
import { anotar, type Ritmo } from "$ui/ritmos";
import type { PanelCliente, ProgresoPanel } from "./global";
import type { TareaEnMarcha } from "./tipos";

export const todos = $state({
  clientes: [] as PanelCliente[],
  omitidos: 0,
  progreso: [] as ProgresoPanel[],
  /** Marca de tiempo de la última carga (0: aún sin la primera). */
  cargado: 0,
  error: "",
  errorCodigo: "",
  /** ¿Lo da el servidor de una vez (v1.3x)? Si no, cliente a cliente. */
  agregado: true,
});

const TODO = 30_000;
const RAPIDO = 3_000;
const LENTO = 12_000;

const ritmos = new Map<string, Ritmo>();
/** Cambia al llegar ritmos nuevos (para que las ondas se repinten). */
export const ritmosTodos = $state({ n: 0 });
export function ritmoTodos(equipo: string, t: TareaEnMarcha): Ritmo | undefined {
  void ritmosTodos.n;
  return ritmos.get(claveTarea(equipo, t));
}

/** Las tareas en marcha de un equipo (de cualquier cliente). */
export function tareasTodos(equipo: string, filtro: { repo?: string; tipos?: TareaEnMarcha["tipo"][] } = {}): TareaEnMarcha[] {
  return todos.progreso
    .filter((x) => x.equipo === equipo)
    .flatMap((x) => x.tareas)
    .filter((t) => (filtro.repo === undefined || t.repo === filtro.repo) && (!filtro.tipos || filtro.tipos.includes(t.tipo)));
}

/** ¿Algo en marcha en ese cliente? */
export const enMarchaEn = (cliente: string) => todos.progreso.some((x) => x.cliente === cliente && x.tareas.length);

async function leerTodo(): Promise<{ clientes: PanelCliente[]; omitidos: number; progreso: ProgresoPanel[] }> {
  if (todos.agregado) {
    try {
      const p = await api.panel({ invisible: !!todos.cargado });
      return p;
    } catch (e) {
      if (!(e instanceof api.ApiError && e.estado === 404)) throw e;
      todos.agregado = false;
    }
  }
  // Servidor anterior: cliente a cliente.
  await cargarClientes();
  const xs = await Promise.all(
    app.clientes.map(async (c) => {
      const [resumen, informes, progreso] = await Promise.all([api.resumen(c.id), api.ultimosInformes(c.id).catch(() => []), api.progreso(c.id).catch(() => [])]);
      const pc: PanelCliente = {
        id: c.id,
        nombre: c.nombre,
        rol: c.rol,
        marca: c.marca ?? null,
        equipos: resumen.equipos,
        avisos_abiertos: resumen.avisos_abiertos,
        pendientes: resumen.pendientes,
        informes: informes.map((x) => ({ equipo: x.equipo, recibido: x.recibido, datos: x.datos })),
        informes_completos: true,
      };
      return { pc, progreso: progreso.map((p) => ({ ...p, cliente: c.id })) };
    }),
  );
  return { clientes: xs.map((x) => x.pc).sort((a, b) => a.nombre.localeCompare(b.nombre)), omitidos: 0, progreso: xs.flatMap((x) => x.progreso) };
}

async function leerProgreso(): Promise<ProgresoPanel[]> {
  if (todos.agregado) return api.panelProgreso();
  const xs = await Promise.all(todos.clientes.map((c) => api.progreso(c.id).then((ps) => ps.map((p) => ({ ...p, cliente: c.id }))).catch(() => [])));
  return xs.flat();
}

function ponerProgreso(nuevo: ProgresoPanel[]): boolean {
  const quedan = new Set(nuevo.flatMap((x) => x.tareas.map((t) => claveTarea(x.equipo, t))));
  const antes = todos.progreso.flatMap((x) => x.tareas.map((t) => claveTarea(x.equipo, t)));
  const terminadas = antes.some((k) => !quedan.has(k));
  const ahora = Date.now();
  for (const k of [...ritmos.keys()]) if (!quedan.has(k)) ritmos.delete(k);
  for (const x of nuevo)
    for (const t of x.tareas) {
      const k = claveTarea(x.equipo, t);
      const cuando = Date.parse(t.actualizado ?? "");
      ritmos.set(k, anotar(ritmos.get(k), Number.isFinite(cuando) ? cuando : ahora, { ...t, tipo: t.tipo }));
    }
  ritmosTodos.n++;
  todos.progreso = nuevo.filter((x) => x.tareas.length);
  return terminadas;
}

let vuelta = 0;

/** Carga todo (en silencio si ya había algo). */
export async function cargarTodos(n = vuelta) {
  try {
    const t = await (todos.cargado ? enFondo(leerTodo) : leerTodo());
    if (n !== vuelta) return;
    todos.clientes = t.clientes;
    todos.omitidos = t.omitidos;
    ponerProgreso(t.progreso);
    todos.cargado = Date.now();
    todos.error = "";
    todos.errorCodigo = "";
  } catch (e) {
    if (n !== vuelta) return;
    todos.error = (e as Error).message;
    todos.errorCodigo = (e as { codigo?: string }).codigo ?? "";
  }
}

/** Empieza a seguir todos los clientes. Devuelve cómo parar. */
export function vigilarTodos(): () => void {
  const n = ++vuelta;
  let tTodo: ReturnType<typeof setTimeout> | null = null;
  let tVivo: ReturnType<typeof setTimeout> | null = null;
  const visible = () => typeof document === "undefined" || document.visibilityState === "visible";
  // Con la pestaña oculta no se pregunta (salvo la primera vez, para tener algo que enseñar).
  const todoOtraVez = async () => {
    if (visible() || !todos.cargado) await cargarTodos(n);
    if (n === vuelta) tTodo = setTimeout(todoOtraVez, TODO);
  };
  const vivo = async () => {
    let siguiente = LENTO;
    try {
      if (visible() && todos.cargado) {
        const terminadas = ponerProgreso(await leerProgreso());
        if (n !== vuelta) return;
        if (terminadas) void cargarTodos(n);
      }
      siguiente = todos.progreso.length ? RAPIDO : LENTO;
    } catch {
      /* sin conexión: se reintenta despacio */
    }
    if (n === vuelta) tVivo = setTimeout(vivo, siguiente);
  };
  void todoOtraVez().then(() => {
    if (n === vuelta) tVivo = setTimeout(vivo, todos.progreso.length ? RAPIDO : LENTO);
  });
  const alVolver = () => {
    if (visible() && n === vuelta) void cargarTodos(n);
  };
  document.addEventListener("visibilitychange", alVolver);
  return () => {
    document.removeEventListener("visibilitychange", alVolver);
    if (n === vuelta) vuelta++;
    if (tTodo) clearTimeout(tTodo);
    if (tVivo) clearTimeout(tVivo);
  };
}
