// Pruebas funcionales (simulacros de recuperación; plan 0.8.0, E3, 14k) y pruebas de
// restauración desde un espejo (E4, 14j): textos, comprobaciones del formulario,
// cifras (RTO y RPO reales) y lo que se puede probar desde cada espejo.
// Sin dependencias de Svelte (lo prueban los vectores).
import { bytes, lista, plural } from "./formato";
import { textoDesde, textoDestino } from "./restauraciones";
import { idPruebaEspejo, type PasoVista } from "./regla321";
import type { Equipo, Restauracion } from "./tipos";
import type { DesdePrueba, EntradaPruebaFuncional, MotivoPrueba, PlazoPrueba, PruebaEspejoOrden, PruebaFuncional, ResultadoPrueba } from "./tiposPruebas";

export const MOTIVOS_PRUEBA: { id: MotivoPrueba; texto: string }[] = [
  { id: "programada", texto: "La que toca (según el plazo)" },
  { id: "auditoria", texto: "Auditoría o la pide el cliente" },
  { id: "cambio", texto: "Tras un cambio (equipos, programas, destinos)" },
  { id: "incidente", texto: "Tras un incidente" },
  { id: "otro", texto: "Otro" },
];

export const RESULTADOS_PRUEBA: { id: ResultadoPrueba; texto: string; tono: "ok" | "warn" | "bad" }[] = [
  { id: "ok", texto: "Correcta", tono: "ok" },
  { id: "parcial", texto: "Con incidencias", tono: "warn" },
  { id: "fallo", texto: "Falló", tono: "bad" },
];

export const DESDE_PRUEBA: { id: DesdePrueba; texto: string }[] = [
  { id: "repositorio", texto: "Su repositorio (el almacén o su destino)" },
  { id: "espejo", texto: "Un espejo" },
  { id: "derivada", texto: "Una copia derivada" },
  { id: "otro", texto: "Otro (un kit, otro sitio)" },
];

export const textoMotivoPrueba = (m: string | null | undefined) => MOTIVOS_PRUEBA.find((x) => x.id === m)?.texto ?? "—";
export const resultadoPrueba = (r: string | null | undefined) => RESULTADOS_PRUEBA.find((x) => x.id === r) ?? RESULTADOS_PRUEBA[2];

/** Minutos en palabras cortas: «45 min», «2 h», «2 h 30 min», «3 días». */
export function textoMinutos(m: number | null | undefined): string {
  if (m == null || !Number.isFinite(m) || m < 0) return "—";
  const x = Math.round(m);
  if (x < 60) return `${x} min`;
  if (x < 48 * 60) return x % 60 ? `${Math.floor(x / 60)} h ${x % 60} min` : `${x / 60} h`;
  return plural(Math.round(x / 1440), "día", "días");
}

/** Lo que escribe la persona («2», «1,5», «90») en minutos según la unidad; null si está vacío; NaN si no vale. */
export function aMinutos(texto: string, unidad: "min" | "h" | "dias"): number | null {
  const t = texto.trim().replace(",", ".");
  if (!t) return null;
  const n = Number(t);
  if (!Number.isFinite(n) || n < 0) return NaN;
  return Math.round(n * (unidad === "min" ? 1 : unidad === "h" ? 60 : 1440));
}

/** Un valor en minutos en la unidad que mejor se lee (para volver a editarlo). */
export function deMinutos(m: number | null | undefined): { texto: string; unidad: "min" | "h" | "dias" } {
  if (m == null || !Number.isFinite(m)) return { texto: "", unidad: "h" };
  if (m >= 1440 && m % 1440 === 0) return { texto: String(m / 1440), unidad: "dias" };
  if (m >= 60 && m % 30 === 0) return { texto: String(m / 60).replace(".", ","), unidad: "h" };
  return { texto: String(m), unidad: "min" };
}

/** Hoy (o la fecha dada) como AAAA-MM-DD, en hora local. */
export function diaLocal(t: number | Date = Date.now()): string {
  const d = new Date(t);
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
}

export const MAX_TEXTO = 2000;
export const MAX_TEXTO_CORTO = 200;
export const MAX_RTO_MIN = 525_600;
export const MAX_RPO_MIN = 5_256_000;

/** Por qué no vale lo escrito en el formulario (el servidor lo vuelve a comprobar), o null. */
export function errorPrueba(e: EntradaPruebaFuncional, hoy: string): string | null {
  if (!/^\d{4}-\d{2}-\d{2}$/.test(e.fecha)) return "Elige la fecha de la prueba.";
  if (e.fecha > hoy) return "La fecha de la prueba no puede ser futura.";
  if (e.alcance === "repos" && !e.repos.length) return "Elige qué repositorios se probaron (o «Todo el cliente»).";
  if (e.rto_min != null && (!Number.isFinite(e.rto_min) || e.rto_min > MAX_RTO_MIN)) return "El tiempo hasta tener los datos de vuelta no vale (como mucho un año).";
  if (e.rpo_min != null && (!Number.isFinite(e.rpo_min) || e.rpo_min > MAX_RPO_MIN)) return "La antigüedad de lo recuperado no vale (como mucho diez años).";
  if (e.donde.length > MAX_TEXTO_CORTO) return `«Dónde se restauró», como mucho ${MAX_TEXTO_CORTO} caracteres.`;
  for (const [nombre, t] of [
    ["Qué se restauró", e.que],
    ["Incidencias", e.incidencias ?? ""],
    ["Notas", e.notas ?? ""],
  ] as const) {
    if (t.length > MAX_TEXTO) return `«${nombre}», como mucho ${MAX_TEXTO} caracteres.`;
  }
  if (e.resultado !== "ok" && !(e.incidencias ?? "").trim()) return "Cuenta qué falló o qué incidencias hubo.";
  return null;
}

/** Vacía, para el formulario. */
export function pruebaNueva(hoy: string): EntradaPruebaFuncional {
  return { fecha: hoy, motivo: "programada", alcance: "cliente", repos: [], que: "", donde: "", desde: null, quien: "", resultado: "ok", incidencias: "", rto_min: null, rpo_min: null, restauracion: null, notas: "" };
}

/** La copia limpia para editar una guardada. */
export function paraEditar(p: PruebaFuncional): EntradaPruebaFuncional {
  return {
    fecha: p.fecha,
    motivo: p.motivo,
    alcance: p.alcance,
    repos: [...p.repos],
    que: p.que ?? "",
    donde: p.donde ?? "",
    desde: p.desde ?? null,
    quien: p.quien ?? "",
    resultado: p.resultado,
    incidencias: p.incidencias ?? "",
    rto_min: p.rto_min ?? null,
    rpo_min: p.rpo_min ?? null,
    restauracion: p.restauracion ?? null,
    notas: p.notas ?? "",
  };
}

/**
 * Una prueba a partir de una restauración registrada (13d, 14l): el día, el repositorio,
 * lo restaurado, a dónde y desde dónde, el resultado, el tiempo (RTO real) y, si se sabe
 * la hora de la versión restaurada, la antigüedad de los datos (RPO).
 */
export function deRestauracion(r: Restauracion, versionHora?: string | null): EntradaPruebaFuncional {
  const cuanto = [r.archivos ? plural(r.archivos, "archivo", "archivos") : "", r.bytes ? bytes(r.bytes) : ""].filter(Boolean).join(", ");
  const inicio = Date.parse(r.inicio ?? r.hora);
  const version = versionHora ? Date.parse(versionHora) : NaN;
  return {
    ...pruebaNueva(diaLocal(Date.parse(r.hora))),
    alcance: "repos",
    repos: [{ equipo: r.equipo, repo: r.repo }],
    que: `Versión ${r.version ?? "—"}${cuanto ? ` (${cuanto})` : ""}`,
    donde: textoDestino(r.destino),
    desde: { tipo: r.desde === "derivada" || r.desde === "externa" ? "derivada" : "repositorio", nombre: r.desde && r.desde !== "principal" ? textoDesde(r) : null },
    resultado: r.resultado === "ok" ? "ok" : "fallo",
    incidencias: r.resultado === "ok" ? "" : (r.mensaje ?? ""),
    rto_min: r.duracion_s != null ? Math.max(1, Math.ceil(r.duracion_s / 60)) : null,
    rpo_min: Number.isFinite(version) && Number.isFinite(inicio) && inicio >= version ? Math.round((inicio - version) / 60_000) : null,
    restauracion: { equipo: r.equipo, id: r.id },
  };
}

export interface CifrasPruebas {
  total: number;
  correctas: number;
  conIncidencias: number;
  fallidas: number;
  /** Minutos: media y peor de las que lo dicen. */
  rtoMedio: number | null;
  rtoPeor: number | null;
  rpoMedio: number | null;
  rpoPeor: number | null;
  /** De las que tienen el dato y hay objetivo: cuántas lo cumplen. */
  rtoCumplen: number;
  rtoConObjetivo: number;
  rpoCumplen: number;
  rpoConObjetivo: number;
}

export function cifrasPruebas(ps: PruebaFuncional[]): CifrasPruebas {
  const media = (xs: number[]) => (xs.length ? Math.round(xs.reduce((a, b) => a + b, 0) / xs.length) : null);
  const rto = ps.map((p) => p.rto_min).filter((x): x is number => typeof x === "number");
  const rpo = ps.map((p) => p.rpo_min).filter((x): x is number => typeof x === "number");
  return {
    total: ps.length,
    correctas: ps.filter((p) => p.resultado === "ok").length,
    conIncidencias: ps.filter((p) => p.resultado === "parcial").length,
    fallidas: ps.filter((p) => p.resultado === "fallo").length,
    rtoMedio: media(rto),
    rtoPeor: rto.length ? Math.max(...rto) : null,
    rpoMedio: media(rpo),
    rpoPeor: rpo.length ? Math.max(...rpo) : null,
    rtoCumplen: ps.filter((p) => p.cumple_rto === true).length,
    rtoConObjetivo: ps.filter((p) => typeof p.cumple_rto === "boolean").length,
    rpoCumplen: ps.filter((p) => p.cumple_rpo === true).length,
    rpoConObjetivo: ps.filter((p) => typeof p.cumple_rpo === "boolean").length,
  };
}

const fmtDia = new Intl.DateTimeFormat("es", { day: "numeric", month: "long", year: "numeric" });
/** «8 de octubre de 2026» para un AAAA-MM-DD (a mediodía: sin saltos de zona). */
export const diaLargo = (d: string) => (/^\d{4}-\d{2}-\d{2}$/.test(d) ? fmtDia.format(new Date(`${d}T12:00:00`)) : d);

/** El plazo en una frase y su tono. */
export function textoPlazo(p: PlazoPrueba): { texto: string; tono: "ok" | "warn" | "bad" } {
  const vence = fmtDia.format(new Date(p.vence));
  const cada = `cada ${plural(p.meses, "mes", "meses")}`;
  switch (p.fase) {
    case "vencida":
      return { texto: `Toca una: el plazo (${cada}) venció el ${vence}.`, tono: "bad" };
    case "pronto":
      return { texto: `Se acerca el plazo: antes del ${vence} (${cada}).`, tono: "warn" };
    default:
      return { texto: `Al día: la siguiente, antes del ${vence} (${cada}).`, tono: "ok" };
  }
}

/** El alcance en palabras, con los nombres de los repositorios. */
export function textoAlcance(p: Pick<EntradaPruebaFuncional, "alcance" | "repos">, nombre: (equipo: string, repo: string) => string): string {
  if (p.alcance === "cliente") return "Todo el cliente";
  const ns = p.repos.map((r) => nombre(r.equipo, r.repo));
  return ns.length > 3 ? `${ns.slice(0, 3).join(", ")} y ${ns.length - 3} más` : lista(ns);
}

// ---------- E4 (14j): probar desde un espejo ----------

/** Lo que se puede hacer con un espejo para probar desde él. */
export interface OpcionPruebaEspejo {
  /** El id de la prueba (estable por espejo). */
  id: string;
  nombre: string;
  paso: PasoVista;
  /**
   * `listo`: se manda tal cual; `carpeta`: hay que decir dónde está el disco del espejo en
   * el equipo dueño; `nube`: hay que elegir una nube conectada en él; `no`: no se puede.
   */
  modo: "listo" | "carpeta" | "nube" | "no";
  /** La orden (para `listo`) o la base (para los demás). */
  orden: PruebaEspejoOrden;
  /** La carpeta propuesta (para `carpeta` y `nube`). */
  sugerida: string;
  /** Por qué no se puede, o qué hacer antes. */
  nota: string;
}

/** `a\b` o `a/b` según el sistema del equipo. */
function unir(a: string, b: string, windows: boolean): string {
  const sep = windows ? "\\" : "/";
  const x = a.trim().replace(/[\\/]+$/, "");
  const y = b.replace(/^[\\/]+/, "").replace(/[\\/]+/g, sep);
  return x ? `${x}${sep}${y}` : y;
}

/** Qué espejos de una copia se pueden probar desde el equipo dueño `e` y cómo. */
export function opcionesPruebaEspejo(pasos: PasoVista[], e: Equipo, equipos: Equipo[]): OpcionPruebaEspejo[] {
  const windows = (e.so ?? "").toLowerCase().startsWith("win");
  const nubes = e.resumen?.nubes ?? [];
  return pasos
    .filter((p) => p.tipo === "espejo" && p.espejo)
    .map((p) => {
      const x = p.espejo!;
      const id = idPruebaEspejo(p);
      const base: PruebaEspejoOrden = { id, nombre: p.nombre.slice(0, 80), destino: p.clave ?? null };
      const almacen = equipos.find((q) => q.id === x.almacen);
      const nombreAlmacen = almacen?.nombre ?? "el almacén";
      if (x.quien === "equipo" && x.trabajo) return { id, nombre: p.nombre, paso: p, modo: "listo", orden: { ...base, trabajo: x.trabajo }, sugerida: "", nota: "" };
      const ruta = x.ruta ?? "";
      if (!ruta) return { id, nombre: p.nombre, paso: p, modo: "no", orden: base, sugerida: "", nota: "No se sabe en qué carpeta del espejo está este repositorio." };
      if (x.tipo === "nube") {
        const sugerida = `${x.carpeta.trim().replace(/^\/+|\/+$/g, "")}/${ruta}`.replace(/^\/+/, "");
        if (!nubes.length)
          return { id, nombre: p.nombre, paso: p, modo: "no", orden: base, sugerida, nota: `Esa nube está conectada en «${nombreAlmacen}». Para probar desde ella, conéctala también en «${e.nombre}» (con permiso para leer).` };
        return { id, nombre: p.nombre, paso: p, modo: "nube", orden: base, sugerida, nota: `Elige la nube de «${e.nombre}» que llega a esa misma cuenta y confirma la carpeta del repositorio dentro de ella.` };
      }
      const carpeta = x.tipo === "zona" ? x.carpeta : x.carpeta;
      const sugerida = carpeta ? unir(carpeta, ruta, windows) : "";
      if (x.almacen === e.id && sugerida) return { id, nombre: p.nombre, paso: p, modo: "listo", orden: { ...base, carpeta: sugerida }, sugerida, nota: "" };
      return {
        id,
        nombre: p.nombre,
        paso: p,
        modo: "carpeta",
        orden: base,
        sugerida,
        nota: `Está en un disco de «${nombreAlmacen}», al que «${e.nombre}» no llega. Conecta ese disco a «${e.nombre}» (o copia allí la carpeta del repositorio) y di dónde está.`,
      };
    });
}

/** La orden final para una opción (con la carpeta o la nube que diga la persona). */
export function ordenPruebaEspejo(o: OpcionPruebaEspejo, extra: { carpeta?: string; nube?: string }, cadaDias?: number | null): PruebaEspejoOrden | null {
  let orden: PruebaEspejoOrden;
  if (o.modo === "listo") orden = { ...o.orden };
  else if (o.modo === "carpeta" && extra.carpeta?.trim()) orden = { ...o.orden, carpeta: extra.carpeta.trim() };
  else if (o.modo === "nube" && extra.nube && extra.carpeta?.trim()) orden = { ...o.orden, nube: extra.nube, carpeta: extra.carpeta.trim().replace(/^\/+|\/+$/g, "") };
  else return null;
  return cadaDias ? { ...orden, cada_dias: cadaDias } : orden;
}

/** El resultado de una prueba desde un espejo en una frase. */
export function textoPruebaEspejo(p: { resultado?: string | null; ultima?: string | null; archivos?: number | null; bytes?: number | null; mensaje_corto?: string | null } | null | undefined): string {
  if (!p?.resultado) return "Aún no se ha probado.";
  if (p.resultado === "ok") {
    const cuanto = [p.archivos ? plural(p.archivos, "archivo", "archivos") : "", p.bytes ? bytes(p.bytes) : ""].filter(Boolean).join(", ");
    return `Bien${cuanto ? `: ${cuanto} restaurados y comprobados` : ""}.`;
  }
  return `Falló${p.mensaje_corto ? `: ${p.mensaje_corto}` : "."}`;
}
