// «Copias» (la sección del cliente): todas las copias de todos sus equipos en
// una lista, con lo que ya dicen el resumen de cada equipo y su último
// informe (nada nuevo del servidor). Lo que no es pantalla: cada fila, los
// filtros, la búsqueda y el orden. C1 (plan 0.8.0, 14h): cada fila trae los
// pasos de su camino (espejos y copias derivadas, lib/camino.ts), que cuentan
// en «Necesitan atención» y en la cifra de las que están al día.
import type { CopiaResumen, DestinoCatalogo, DestinoResumen, Equipo, Informe, RepositorioResumen } from "./tipos";
import { caminoDeCopia, eslabonMal, type Eslabon, type OpcionesCamino } from "./camino";
import { reglaDeCopia } from "./regla321";
import { estadoCopia, infCopia, proximaDe, ultimaVuelta, type EstadoCopia, type Vuelta } from "./copia";
import { destinoDe, informeDe } from "./repo";
import { enPausa, PESO } from "./salud";

export interface FilaCopia {
  /** «equipo|copia»: única en el cliente. */
  clave: string;
  equipo: Equipo;
  copia: CopiaResumen;
  repo: RepositorioResumen | null;
  destino: DestinoResumen | undefined;
  vuelta: Vuelta | null;
  estado: EstadoCopia;
  proxima: string | null;
  /** Lo que ocupan sus archivos (su última versión), si se sabe. */
  protegido: number | null;
  /** Se le pueden mandar órdenes desde aquí (equipo confirmado y no trasladado, copia activa). */
  ordenable: boolean;
  /** C1: los pasos de su camino después del repositorio (espejos y copias derivadas), con su estado. */
  pasos: Eslabon[];
  /** C1: algún paso falla, va atrasado o no se conecta. */
  pasoMal: boolean;
  /** Pide atención: la copia (falló, atrasada, con avisos) o algún paso de su camino. */
  atencion: boolean;
}

/**
 * Las filas de «Copias». `catalogo` (lo marcado en los destinos) y `enVivo` (lo
 * que está en marcha) son para el camino; `todos`: todos los equipos del cliente
 * (para encontrar el almacén aunque un filtro lo deje fuera).
 */
export function filasCopias(
  equipos: Equipo[],
  informes: Record<string, Informe | null | undefined>,
  ahora = Date.now(),
  catalogo: DestinoCatalogo[] = [],
  o: { cliente?: string; enVivo?: OpcionesCamino["enVivo"]; todos?: Equipo[] } = {},
): FilaCopia[] {
  const todos = o.todos ?? equipos;
  return equipos.flatMap((e) => {
    const informe = informes[e.id] ?? null;
    const pausada = enPausa(e, ahora);
    return (e.resumen?.copias ?? []).map((k): FilaCopia => {
      const repo = e.resumen?.repositorios?.find((r) => r.id === k.repo) ?? null;
      const vuelta = ultimaVuelta(k, informe);
      const ultimaVersion = infCopia(informeDe(informe, k.repo), k.id)?.versiones[0];
      const estado = estadoCopia(k, vuelta, pausada, ahora);
      const pasos = caminoDeCopia(reglaDeCopia(e, k, todos, informe, catalogo, ahora), { cliente: o.cliente ?? "", ahora, enVivo: o.enVivo, equipos: todos, informe }).filter((x) => x.clase !== "copia");
      const pasoMal = pasos.some(eslabonMal);
      return {
        clave: `${e.id}|${k.id}`,
        equipo: e,
        copia: k,
        repo,
        destino: destinoDe(e.resumen?.destinos, repo),
        vuelta,
        estado,
        proxima: k.activa === false || pausada ? null : proximaDe(k, informe, ahora),
        protegido: ultimaVersion?.total_bytes ?? k.ultima?.bytes ?? null,
        ordenable: e.confirmado && e.modo !== "trasladado" && k.activa !== false,
        pasos,
        pasoMal,
        atencion: estado.tono === "bad" || estado.tono === "warn" || pasoMal,
      };
    });
  });
}

export type EstadoFiltro = "todos" | "atencion" | "al_dia" | "paradas";
export const ESTADOS_FILTRO: { id: EstadoFiltro; texto: string }[] = [
  { id: "todos", texto: "Todas" },
  { id: "atencion", texto: "Necesitan atención" },
  { id: "al_dia", texto: "Al día" },
  { id: "paradas", texto: "En pausa o desactivadas" },
];
/** C1: con o sin espejos y copias derivadas después de su repositorio. */
export type EspejoFiltro = "todos" | "con" | "sin";
export const ESPEJO_FILTRO: { id: EspejoFiltro; texto: string }[] = [
  { id: "todos", texto: "Con o sin espejo" },
  { id: "con", texto: "Con espejo o copia derivada" },
  { id: "sin", texto: "Sin espejo ni copia derivada" },
];

/** C1: la cifra de arriba, con los pasos: «5 al día · 1 con un paso fallando». */
export function cuentaCopias(filas: FilaCopia[]): { total: number; alDia: number; conPasoMal: number; atencion: number } {
  return {
    total: filas.length,
    alDia: filas.filter((f) => f.estado.tono === "ok" && !f.pasoMal).length,
    conPasoMal: filas.filter((f) => f.estado.tono === "ok" && f.pasoMal).length,
    atencion: filas.filter((f) => f.atencion).length,
  };
}

export type Orden = "estado" | "nombre" | "equipo" | "ultima" | "proxima" | "protegido";
export const ORDENES: { id: Orden; texto: string }[] = [
  { id: "estado", texto: "Lo que falla, primero" },
  { id: "nombre", texto: "Por nombre" },
  { id: "equipo", texto: "Por equipo" },
  { id: "ultima", texto: "La última, más reciente" },
  { id: "proxima", texto: "La próxima, más cercana" },
  { id: "protegido", texto: "Lo que más protege" },
];

const sinTildes = (s: string) => s.normalize("NFD").replace(/[̀-ͯ]/g, "").toLowerCase();

export interface FiltroCopias {
  buscar?: string;
  estado?: EstadoFiltro;
  equipo?: string | null;
  /** «equipo|repositorio». */
  repo?: string | null;
  /** C1: con o sin espejos y derivadas. */
  espejo?: EspejoFiltro;
}

export function filtrarCopias<F extends FilaCopia>(filas: F[], f: FiltroCopias): F[] {
  const palabras = sinTildes(f.buscar ?? "")
    .split(/\s+/)
    .filter(Boolean);
  return filas.filter((x) => {
    if (f.equipo && x.equipo.id !== f.equipo) return false;
    if (f.repo && `${x.equipo.id}|${x.copia.repo}` !== f.repo) return false;
    const t = x.estado.tono;
    if (f.estado === "atencion" && !x.atencion) return false;
    // Una copia al día con un paso fallando no está del todo al día.
    if (f.estado === "al_dia" && (t !== "ok" || x.pasoMal)) return false;
    if (f.espejo === "con" && !x.pasos.length) return false;
    if (f.espejo === "sin" && x.pasos.length) return false;
    if (f.estado === "paradas" && t !== "paused" && x.copia.activa !== false) return false;
    if (palabras.length) {
      const texto = sinTildes([x.copia.nombre, x.equipo.nombre, x.repo?.nombre ?? x.copia.repo, x.destino?.nombre ?? "", ...x.pasos.map((p) => p.nombre), ...(x.equipo.etiquetas ?? [])].join(" "));
      if (!palabras.every((p) => texto.includes(p))) return false;
    }
    return true;
  });
}

const ms = (x: string | null | undefined) => (x ? Date.parse(x) : NaN);
const tonoOrden = (f: FilaCopia) => (f.estado.tono === "ok" && f.pasoMal ? (f.pasos.some((p) => p.tono === "bad") ? "bad" : "warn") : f.estado.tono);
export function ordenarCopias<F extends FilaCopia>(filas: F[], orden: Orden): F[] {
  const porNombre = (a: FilaCopia, b: FilaCopia) => a.copia.nombre.localeCompare(b.copia.nombre, "es") || a.equipo.nombre.localeCompare(b.equipo.nombre, "es");
  // Lo que no tiene dato, al final.
  const num = (a: number, b: number, asc: boolean) => (Number.isNaN(a) ? (Number.isNaN(b) ? 0 : 1) : Number.isNaN(b) ? -1 : asc ? a - b : b - a);
  const cmp: Record<Orden, (a: FilaCopia, b: FilaCopia) => number> = {
    // Una copia al día con un paso fallando va con las que tienen avisos.
    estado: (a, b) => PESO[tonoOrden(a)] - PESO[tonoOrden(b)] || porNombre(a, b),
    nombre: porNombre,
    equipo: (a, b) => a.equipo.nombre.localeCompare(b.equipo.nombre, "es") || porNombre(a, b),
    ultima: (a, b) => num(ms(a.vuelta?.cuando), ms(b.vuelta?.cuando), false) || porNombre(a, b),
    proxima: (a, b) => num(ms(a.proxima), ms(b.proxima), true) || porNombre(a, b),
    protegido: (a, b) => num(a.protegido ?? NaN, b.protegido ?? NaN, false) || porNombre(a, b),
  };
  return [...filas].sort(cmp[orden]);
}
