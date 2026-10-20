// Las nubes conectadas en un equipo, a la vista (plan 0.8.1, B3; plan-mejoras 15s):
// en la tarjeta del almacén (sin «ver abajo»), en la página de nubes del equipo
// (`/c/[c]/equipos/[e]/nubes`) y en la página de cada nube (`…/nubes/[n]`).
// Por cada una: tipo, bucket y carpeta base (los del catálogo de esta consola, o la
// carpeta de sus espejos), si es inmutable, qué la usa, lo que ocupa y el estado de
// la conexión según la última subida de sus espejos (el agente no manda otro dato:
// no hay una orden «probar la nube»). Sin nada de Svelte.
import type { DestinoCatalogo, EspacioVolumen, Equipo, RepositorioResumen, TrabajoEspejoResumen } from "./tipos";
import { destinoDe } from "./repo";
import { claveNube, type DestinoVista } from "./destinos";
import { destinosParaPasos } from "./cadenas";
import { motivoNoDesconectar, nubesDelEquipo, type NubeDelEquipo } from "./nubesEquipo";
import { nombreTipoNube } from "./espejo";
import { estadoTrabajo, hrefEspejo, nombrePorDefecto, trabajosDelAlmacen, trabajosDelEquipo } from "./espejoTrabajos";
import { clasificacionDeVista } from "./regla321";
import { reposDeTrabajo, type RepoPorEspejo } from "./fichaDestino";

/** Un repositorio que guarda un espejo en la nube, con su ruta allí (si se sabe) y lo que ocupa. */
export interface RepoEnNube extends RepoPorEspejo {
  /** `<carpeta del espejo>/<usuario>/<repo>` (lo que hace el espejo del almacén); null si no se sabe. */
  ruta: string | null;
  bytes: number | null;
}

/** Un espejo que sube a la nube. */
export interface EspejoEnNube {
  trabajo: TrabajoEspejoResumen;
  quien: "almacen" | "equipo";
  nombre: string;
  estado: ReturnType<typeof estadoTrabajo>;
  href: string;
  carpeta: string;
  repos: RepoEnNube[];
}

/** Un repositorio (o una copia derivada) que el propio equipo guarda directamente en la nube. */
export interface GuardadoDirecto {
  repo: RepositorioResumen;
  /** El id de la derivada (`externa` la de siempre), si lo es. */
  derivada?: string;
  nombre: string;
  /** Su carpeta en la nube (la del destino y la suya). */
  ruta: string | null;
}

export interface Conexion {
  tono: "ok" | "bad" | "neutral";
  texto: string;
  cuando: string | null;
  error: string | null;
}

export interface NubeVista extends NubeDelEquipo {
  clave: string;
  tipoTexto: string;
  bucket: string | null;
  carpetaBase: string | null;
  inmutable: boolean;
  espejos: EspejoEnNube[];
  /** Espacio libre que dijo el proveedor en la última subida (si lo dice). */
  espacio: EspacioVolumen | null;
  /** Lo que ocupan sus espejos allí (si el agente lo cuenta). */
  ocupa: number | null;
  conexion: Conexion;
  /** Lo que el equipo guarda directamente en ella (repositorios y copias derivadas). */
  directos: GuardadoDirecto[];
  /** Por qué no se puede desconectar ahora (algo la usa), o null. */
  motivo: string | null;
  vista: DestinoVista | null;
}

export const hrefNubes = (cliente: string, equipo: string) => `/c/${cliente}/equipos/${encodeURIComponent(equipo)}/nubes`;
export const hrefNube = (cliente: string, equipo: string, nombre: string) => `${hrefNubes(cliente, equipo)}/${encodeURIComponent(nombre)}`;

/** El estado de la conexión, por la última subida de sus espejos. */
export function conexionDe(espejos: Pick<EspejoEnNube, "trabajo">[]): Conexion {
  const ts = espejos.map((x) => x.trabajo).filter((t) => t.ultima);
  if (!ts.length) return { tono: "neutral", texto: espejos.length ? "Sin subidas todavía" : "Ningún espejo sube aquí", cuando: null, error: null };
  const ultima = [...ts].sort((a, b) => (b.ultima ?? "").localeCompare(a.ultima ?? ""))[0];
  const mal = estadoTrabajo(ultima) === "error";
  if (mal) return { tono: "bad", texto: "La última subida falló", cuando: ultima.ultima ?? null, error: (ultima.resultado ?? "").replace(/^ERROR:\s*/, "") || null };
  return { tono: "ok", texto: "Responde", cuando: ultima.ultima ?? null, error: null };
}

/** Las nubes conectadas en `e`, con todo lo que se enseña de cada una. */
export function nubesVista(e: Equipo, equipos: Equipo[], catalogo: DestinoCatalogo[], cliente: string): NubeVista[] {
  const todos = equipos.some((x) => x.id === e.id) ? equipos : [...equipos, e];
  const vistas = destinosParaPasos(todos, catalogo);
  return nubesDelEquipo(e).map((n) => {
    const clave = claveNube(e.id, n.nombre);
    const vista = vistas.find((v) => v.clave === clave || v.alias?.some((a) => a.id === clave) || v.catalogo?.id === clave) ?? null;
    const espejos: EspejoEnNube[] = [];
    for (const [quien, lista] of [
      ["almacen", trabajosDelAlmacen(e)],
      ["equipo", trabajosDelEquipo(e)],
    ] as const)
      for (const t of lista) {
        if (t.adonde.tipo !== "nube" || t.adonde.nube !== n.nombre) continue;
        const carpeta = t.adonde.carpeta.replace(/\/+$/, "");
        espejos.push({
          trabajo: t,
          quien,
          nombre: t.nombre.trim() || nombrePorDefecto(t.adonde),
          estado: estadoTrabajo(t),
          href: hrefEspejo(cliente, e.id, t.id),
          carpeta,
          repos: reposDeTrabajo(t, e, todos).map((r) => ({ ...r, ruta: quien === "almacen" && carpeta ? `${carpeta}/${r.valor}` : null, bytes: t.ocupa?.repos?.[r.valor] ?? null })),
        });
      }
    const cat = vista?.catalogo ?? null;
    const ocupas = espejos.map((x) => x.trabajo.ocupa?.bytes).filter((b): b is number => typeof b === "number");
    const espacio = espejos.map((x) => x.trabajo.espacio).filter((x): x is EspacioVolumen => !!x).sort((a, b) => (b.leido ?? "").localeCompare(a.leido ?? ""))[0] ?? null;
    return {
      ...n,
      clave,
      tipoTexto: nombreTipoNube(n.tipo) || n.tipo || "Nube",
      bucket: cat?.donde?.trim() || null,
      carpetaBase: cat?.carpeta?.trim() || null,
      inmutable: vista ? clasificacionDeVista(vista, todos).inmutable : false,
      espejos,
      espacio,
      ocupa: ocupas.length ? ocupas.reduce((a, b) => a + b, 0) : null,
      conexion: conexionDe(espejos),
      directos: directosEn(e, n.nombre),
      motivo: motivoNoDesconectar(e, n),
      vista,
    };
  });
}

/** Lo que `e` guarda directamente en la nube `nombre` (por un destino «nube» suyo). */
export function directosEn(e: Pick<Equipo, "resumen">, nombre: string): GuardadoDirecto[] {
  const destinos = (e.resumen?.destinos ?? []).filter((d) => d.tipo === "nube" && d.nube === nombre);
  const ruta = (base: string | undefined, propia: string) => [base?.replace(/\/+$/, ""), propia].filter(Boolean).join("/") || null;
  const out: GuardadoDirecto[] = [];
  for (const r of e.resumen?.repositorios ?? []) {
    const d = destinoDe(e.resumen?.destinos, r);
    if (d && destinos.includes(d)) out.push({ repo: r, nombre: r.nombre || r.id, ruta: ruta(d.donde, r.ruta && r.ruta !== "." ? r.ruta : r.id) });
    const dvs = [...(r.externa ? [{ id: "externa", destino_id: r.externa.destino_id, destino: r.externa.destino, carpeta: r.id }] : []), ...(r.derivadas ?? []).map((x) => ({ id: x.id, nombre: x.nombre, destino_id: x.destino_id, destino: x.destino, carpeta: `${r.id}-${x.id}` }))];
    for (const x of dvs) {
      const dd = destinos.find((y) => y.id === x.destino_id) ?? destinos.find((y) => y.nombre === x.destino);
      if (dd) out.push({ repo: r, derivada: x.id, nombre: ("nombre" in x && x.nombre) || `${r.nombre || r.id} (copia derivada)`, ruta: ruta(dd.donde, x.carpeta) });
    }
  }
  return out;
}
