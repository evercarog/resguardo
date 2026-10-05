// «Copias» (la sección del cliente): todas las copias de todos sus equipos en
// una lista, con lo que ya dicen el resumen de cada equipo y su último
// informe (nada nuevo del servidor). Lo que no es pantalla: cada fila, los
// filtros, la búsqueda y el orden.
import type { CopiaResumen, DestinoResumen, Equipo, Informe, RepositorioResumen } from "./tipos";
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
}

export function filasCopias(equipos: Equipo[], informes: Record<string, Informe | null | undefined>, ahora = Date.now()): FilaCopia[] {
  return equipos.flatMap((e) => {
    const informe = informes[e.id] ?? null;
    const pausada = enPausa(e, ahora);
    return (e.resumen?.copias ?? []).map((k): FilaCopia => {
      const repo = e.resumen?.repositorios?.find((r) => r.id === k.repo) ?? null;
      const vuelta = ultimaVuelta(k, informe);
      const ultimaVersion = infCopia(informeDe(informe, k.repo), k.id)?.versiones[0];
      return {
        clave: `${e.id}|${k.id}`,
        equipo: e,
        copia: k,
        repo,
        destino: destinoDe(e.resumen?.destinos, repo),
        vuelta,
        estado: estadoCopia(k, vuelta, pausada, ahora),
        proxima: k.activa === false || pausada ? null : proximaDe(k, informe, ahora),
        protegido: ultimaVersion?.total_bytes ?? k.ultima?.bytes ?? null,
        ordenable: e.confirmado && e.modo !== "trasladado" && k.activa !== false,
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
export type Orden = "estado" | "nombre" | "equipo" | "ultima" | "proxima" | "protegido";
export const ORDENES: { id: Orden; texto: string }[] = [
  { id: "estado", texto: "Primero lo que necesita atención" },
  { id: "nombre", texto: "Por nombre" },
  { id: "equipo", texto: "Por equipo" },
  { id: "ultima", texto: "La última copia más reciente" },
  { id: "proxima", texto: "La próxima más cercana" },
  { id: "protegido", texto: "Lo que más protege" },
];

const sinTildes = (s: string) => s.normalize("NFD").replace(/[̀-ͯ]/g, "").toLowerCase();

export interface FiltroCopias {
  buscar?: string;
  estado?: EstadoFiltro;
  equipo?: string | null;
  /** «equipo|repositorio». */
  repo?: string | null;
}

export function filtrarCopias(filas: FilaCopia[], f: FiltroCopias): FilaCopia[] {
  const palabras = sinTildes(f.buscar ?? "")
    .split(/\s+/)
    .filter(Boolean);
  return filas.filter((x) => {
    if (f.equipo && x.equipo.id !== f.equipo) return false;
    if (f.repo && `${x.equipo.id}|${x.copia.repo}` !== f.repo) return false;
    const t = x.estado.tono;
    if (f.estado === "atencion" && t !== "bad" && t !== "warn") return false;
    if (f.estado === "al_dia" && t !== "ok") return false;
    if (f.estado === "paradas" && t !== "paused" && x.copia.activa !== false) return false;
    if (palabras.length) {
      const texto = sinTildes([x.copia.nombre, x.equipo.nombre, x.repo?.nombre ?? x.copia.repo, x.destino?.nombre ?? "", ...(x.equipo.etiquetas ?? [])].join(" "));
      if (!palabras.every((p) => texto.includes(p))) return false;
    }
    return true;
  });
}

const ms = (x: string | null | undefined) => (x ? Date.parse(x) : NaN);
export function ordenarCopias(filas: FilaCopia[], orden: Orden): FilaCopia[] {
  const porNombre = (a: FilaCopia, b: FilaCopia) => a.copia.nombre.localeCompare(b.copia.nombre, "es") || a.equipo.nombre.localeCompare(b.equipo.nombre, "es");
  // Lo que no tiene dato, al final.
  const num = (a: number, b: number, asc: boolean) => (Number.isNaN(a) ? (Number.isNaN(b) ? 0 : 1) : Number.isNaN(b) ? -1 : asc ? a - b : b - a);
  const cmp: Record<Orden, (a: FilaCopia, b: FilaCopia) => number> = {
    estado: (a, b) => PESO[a.estado.tono] - PESO[b.estado.tono] || porNombre(a, b),
    nombre: porNombre,
    equipo: (a, b) => a.equipo.nombre.localeCompare(b.equipo.nombre, "es") || porNombre(a, b),
    ultima: (a, b) => num(ms(a.vuelta?.cuando), ms(b.vuelta?.cuando), false) || porNombre(a, b),
    proxima: (a, b) => num(ms(a.proxima), ms(b.proxima), true) || porNombre(a, b),
    protegido: (a, b) => num(a.protegido ?? NaN, b.protegido ?? NaN, false) || porNombre(a, b),
  };
  return [...filas].sort(cmp[orden]);
}
