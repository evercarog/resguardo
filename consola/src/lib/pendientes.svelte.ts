// Órdenes en camino, enseñadas donde aparecerá su resultado.
//
// Al mandar una orden que crea o cambia algo visible (un repositorio, las
// copias, el Servidor de copias, el espejo, una nube, la retención…), la
// pantalla que lo lista pinta un hueco «en camino» con su estado: enviada,
// esperando al equipo (o sin conexión), aplicándose, lista o con su error.
// Cuando el resumen del equipo ya trae lo nuevo, el hueco desaparece y queda
// lo real. Así nadie ve una lista vacía y cree que no ha pasado nada.
import * as api from "./api";
import { enFondo } from "./actividad.svelte";
import { actual, cargarCliente } from "./estado.svelte";
import type { Equipo, Orden } from "./tipos";

/** Dónde se enseña: cada pantalla pide los suyos. */
export type Lugar = "repositorios" | "destinos" | "copias" | "guarda" | "espejo" | "nubes";

export interface Pendiente {
  orden: Orden;
  cliente: string;
  equipo: string;
  equipoNombre: string;
  lugar: Lugar;
  /** Lo que se verá en el hueco: «Repositorio «Documentos»». */
  titulo: string;
  /** Id de lo que aparecerá (repositorio, copia, nube…), para saber cuándo ya está. */
  objetivo?: string;
  /** Si crea también un destino nuevo (crear_repositorio con datos de destino): su nombre. */
  destinoNuevo?: string;
  /** Cuándo terminó bien (se quita al verse lo real, o como mucho a los 45 s). */
  hechaEn?: number;
}

export const pendientes = $state({ lista: [] as Pendiente[] });

const FINALES = ["hecha", "fallida", "rechazada", "cancelada", "caducada"];
export const terminada = (o: Orden) => FINALES.includes(o.estado);
export const conError = (o: Orden) => ["fallida", "rechazada", "caducada"].includes(o.estado);

/** El nombre del destino nuevo que trae `crear_repositorio` (si no usa uno que ya existe). */
function destinoNuevoDe(cuerpo: Record<string, unknown>): string | undefined {
  const d = cuerpo.destino as { nombre?: unknown; tipo?: unknown } | undefined;
  return d && typeof d === "object" && typeof d.tipo === "string" && typeof d.nombre === "string" ? d.nombre : undefined;
}

/** Qué se ve y dónde, según el tipo y el cuerpo de la orden (null: no se enseña aparte). */
export function lugarDe(tipo: string, cuerpo: Record<string, unknown>): { lugar: Lugar; titulo: string; objetivo?: string; destinoNuevo?: string } | null {
  const repo = typeof cuerpo.repo === "string" ? cuerpo.repo : undefined;
  switch (tipo) {
    case "crear_repositorio":
      return { lugar: "repositorios", titulo: `Repositorio «${String(cuerpo.nombre ?? cuerpo.id ?? "")}»`, objetivo: String(cuerpo.id ?? ""), destinoNuevo: destinoNuevoDe(cuerpo) };
    case "importar_repositorio":
      return { lugar: "repositorios", titulo: `Repositorio «${String(cuerpo.nombre ?? cuerpo.id ?? "")}» (importado)`, objetivo: String(cuerpo.id ?? "") };
    case "cambiar_retencion":
      return { lugar: "repositorios", titulo: "Nueva retención", objetivo: repo };
    case "cambiar_copia_externa":
      return { lugar: "repositorios", titulo: cuerpo.hora === null ? "Quitar la copia externa" : "Copia externa", objetivo: repo };
    case "cambiar_derivada":
      return { lugar: "repositorios", titulo: "Copia derivada", objetivo: repo };
    case "quitar_derivada":
      return { lugar: "repositorios", titulo: "Quitar una copia derivada", objetivo: repo };
    case "cambiar_destino":
      return { lugar: "destinos", titulo: "Nuevas credenciales del destino", objetivo: String(cuerpo.destino ?? "") };
    case "config":
      return { lugar: "copias", titulo: "Cambios en las copias" };
    case "guarda_copias":
      if ("espejo" in cuerpo) return { lugar: "espejo", titulo: cuerpo.espejo === null ? "Quitar el espejo" : "Espejo de lo que guarda" };
      // Plan 0.7.26: los espejos que hace el propio equipo.
      if ("espejo_equipo" in cuerpo) return { lugar: "espejo", titulo: cuerpo.espejo_equipo === null ? "Quitar los espejos del equipo" : "Espejos del equipo" };
      if ("espejo_freno" in cuerpo) return { lugar: "espejo", titulo: "Confirmar el freno del espejo" };
      if ("anadir" in cuerpo) return { lugar: "guarda", titulo: `Dar acceso a otro equipo` };
      if ("quitar" in cuerpo) return { lugar: "guarda", titulo: `Quitar el acceso de «${String(cuerpo.quitar ?? "")}»` };
      return { lugar: "guarda", titulo: cuerpo.activo === false ? "Dejar de guardar copias" : "Este equipo guarda copias" };
    // v1.22: en la tarjeta «Guarda copias» del almacén.
    case "retencion_almacen":
      return { lugar: "guarda", titulo: `${cuerpo.quitar === true ? "Dejar de aplicar la retención de" : "Retención de"} «${String(cuerpo.usuario ?? "")}/${String(cuerpo.repo ?? "")}»` };
    case "aplicar_retencion_almacen":
      return { lugar: "guarda", titulo: `Aplicar la retención de «${String(cuerpo.usuario ?? "")}/${String(cuerpo.repo ?? "")}»` };
    case "conectar_nube":
      return { lugar: "nubes", titulo: `Conectar «${String(cuerpo.nombre ?? "")}»`, objetivo: String(cuerpo.nombre ?? "") };
    case "quitar_nube":
      return { lugar: "nubes", titulo: `Desconectar «${String(cuerpo.nombre ?? "")}»`, objetivo: String(cuerpo.nombre ?? "") };
    default:
      return null;
  }
}

/** Empieza a seguir una orden recién mandada (si es de las que se enseñan aparte). */
export function seguirOrden(cliente: string, equipo: Equipo, o: Orden, cuerpo: Record<string, unknown>) {
  const l = lugarDe(o.tipo, cuerpo);
  if (!l) return;
  pendientes.lista = [...pendientes.lista.filter((p) => p.orden.id !== o.id), { orden: o, cliente, equipo: equipo.id, equipoNombre: equipo.nombre, ...l }];
  arrancar();
}

/** Los de una pantalla (del cliente abierto). */
export function pendientesDe(lugar: Lugar | Lugar[], equipo?: string): Pendiente[] {
  const lugares = Array.isArray(lugar) ? lugar : [lugar];
  // Lo que crea algo nuevo (un repositorio, una nube) deja su hueco en cuanto se ve lo real: nunca los dos a la vez.
  const creaAlgo = (p: Pendiente) => ["crear_repositorio", "importar_repositorio", "conectar_nube"].includes(p.orden.tipo);
  return pendientes.lista.filter((p) => p.cliente === actual.id && lugares.includes(p.lugar) && (!equipo || p.equipo === equipo) && !(p.hechaEn && creaAlgo(p) && yaEsta(p)));
}

export function quitarPendiente(id: string) {
  pendientes.lista = pendientes.lista.filter((p) => p.orden.id !== id);
}

/** ¿Ya se ve lo real en el resumen del equipo? */
function yaEsta(p: Pendiente): boolean {
  const e = actual.equipos.find((x) => x.id === p.equipo);
  const r = e?.resumen;
  if (!r || !p.hechaEn) return false;
  switch (p.orden.tipo) {
    case "crear_repositorio":
    case "importar_repositorio":
      return !!r.repositorios?.some((x) => x.id === p.objetivo);
    // Tarea 4a: también en un equipo que no es almacén (`resumen.nubes`).
    case "conectar_nube":
      return [...(r.nubes ?? []), ...(r.guarda_copias?.nubes ?? [])].some((n) => n.nombre === p.objetivo);
    case "quitar_nube":
      return ![...(r.nubes ?? []), ...(r.guarda_copias?.nubes ?? [])].some((n) => n.nombre === p.objetivo);
    default:
      // Lo demás cambia algo que ya estaba: basta con un resumen recibido después.
      return actual.cargado > p.hechaEn;
  }
}

let sondeo: ReturnType<typeof setInterval> | null = null;

function arrancar() {
  if (sondeo || typeof window === "undefined") return;
  sondeo = setInterval(() => void vuelta(), 2000);
}

async function vuelta() {
  // Las fallidas se quedan hasta que alguien las quite, pero ya no hace falta preguntar por ellas.
  if (!pendientes.lista.some((p) => !terminada(p.orden) || p.hechaEn)) {
    if (sondeo) clearInterval(sondeo);
    sondeo = null;
    return;
  }
  // Una consulta por equipo con órdenes en camino.
  const porEquipo = new Map<string, Pendiente[]>();
  for (const p of pendientes.lista) if (!terminada(p.orden)) porEquipo.set(`${p.cliente}|${p.equipo}`, [...(porEquipo.get(`${p.cliente}|${p.equipo}`) ?? []), p]);
  let refrescar = false;
  await Promise.all(
    [...porEquipo.entries()].map(async ([k, ps]) => {
      const [c, e] = k.split("|");
      try {
        const ordenes = await enFondo(() => api.ordenesEquipo(c, e, 30));
        for (const p of ps) {
          const o = ordenes.find((x) => x.id === p.orden.id);
          if (!o) continue;
          const antes = p.orden.estado;
          p.orden = o;
          if (o.estado !== antes && terminada(o)) {
            refrescar = true;
            if (o.estado === "hecha") p.hechaEn = Date.now();
            if (o.estado === "cancelada") quitarPendiente(o.id);
          }
        }
      } catch {
        /* se reintenta en la siguiente vuelta */
      }
    }),
  );
  if (refrescar && actual.id) await cargarCliente(actual.id, { silencioso: true });
  // Las hechas se van cuando ya se ve lo real (o, como mucho, a los 45 s).
  // «Listo» se ve al menos un par de segundos, para que se note el cambio.
  pendientes.lista = pendientes.lista.filter((p) => !(p.hechaEn && Date.now() - p.hechaEn > 2500 && (yaEsta(p) || Date.now() - p.hechaEn > 45_000)));
  // Mientras haya hechas esperando a verse, se pide el resumen.
  if (pendientes.lista.some((p) => p.hechaEn) && actual.id && !refrescar) void cargarCliente(actual.id, { silencioso: true });
}
