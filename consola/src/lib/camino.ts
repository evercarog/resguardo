// El camino de cada copia, a la vista (plan 0.8.0, C1; plan-mejoras 14kk y 14h):
// «copia → repositorio → después → derivada → espejo», cada eslabón con el tipo
// de su destino, su nombre, su estado y su enlace. Lo enseñan la cabecera de la
// página de la copia, la tarjeta del repositorio en la ficha del equipo y la
// página «Copias» (una línea por paso, que cuenta en «Necesitan atención»).
//
// No calcula nada nuevo: los pasos son los de la regla 3-2-1-1-0 de la copia
// (`reglaDeCopia`, que ya sabe qué espejos del almacén incluyen el repositorio
// con la misma lógica que el mapa, sus copias derivadas y los espejos del propio
// equipo), así que el camino, la tira 3-2-1 y la salud dicen lo mismo. Lo que
// está en marcha llega como una función (`enVivo`, la misma que el mapa). Sin
// runas (lo prueban scripts/vectores-camino.ts).
import type { Tono } from "./salud";
import type { ReglaCopia, PasoVista } from "./regla321";
import { pasoAlDia } from "./regla321";
import { corta, type TipoCorto } from "./tipoDestino";
import { atrasada, estadoCopia, ultimaVuelta } from "./copia";
import { enPausa } from "./salud";
import { hrefDestino } from "./fichaDestino";
import { relativo } from "./formato";
import { hrefEspejo, trabajosDelAlmacen, trabajosDelEquipo } from "./espejoTrabajos";
import type { Equipo, Informe, TrabajoEspejoResumen } from "./tipos";

/** Un eslabón del camino: el repositorio de la copia, un espejo o una copia derivada. */
export interface Eslabon {
  /** El id del paso en la regla («destino», «espejo-1», «derivada-d1», «externa»…). */
  id: string;
  /** «copia»: el repositorio donde guarda la copia; «espejo»: el mismo repositorio en otro destino; «derivada»: versiones de él en otro repositorio. */
  clase: "copia" | "espejo" | "derivada";
  /** El destino, con su nombre («Almacén · Disco D», «Backblaze Oficina»). */
  nombre: string;
  /** Quién lo hace, si no es el propio equipo (un espejo lo hace el almacén). */
  quien: string | null;
  /** Tipo y marcas del destino (icono). */
  tipo: TipoCorto;
  /** Empieza «después» (de cada copia o de la anterior). */
  despues: boolean;
  tono: Tono;
  /** El estado, en una o dos palabras: «Al día», «Subiendo 45 %», «Falló», «Atrasado», «Sin conexión», «En pausa», «Todavía sin ninguna». */
  estado: string;
  /** Cuándo pasó lo que dice el estado (la última vez bien o la que falló), si se sabe. */
  cuando: string | null;
  /** Algo en marcha ahora. */
  vivo: boolean;
  href: string | null;
  /** 14h: de un espejo, lo que ocupa este repositorio en su destino (si se sabe), cuándo se contó y su última comprobación. */
  ocupa?: number | null;
  ocupaCuando?: string | null;
  verificacion?: { ultima: string; archivos: number; mal: number } | null;
}

export interface OpcionesCamino {
  cliente: string;
  ahora: number;
  /** Lo que está en marcha (la misma que el mapa): «Copiando 40 %», «Subiendo 12 %», o null. (14p: de un espejo, `repo` es su trabajo.) */
  enVivo?: (equipo: string, repo: string, tipo: "copia" | "copia_externa" | "espejo") => string | null;
  /** Para saber si quien hace el paso (el almacén) está conectado. */
  equipos?: Equipo[];
  informe?: Informe | null;
}

const CLASE: Record<PasoVista["tipo"], Eslabon["clase"]> = { copia: "copia", espejo: "espejo", externa: "derivada", derivada: "derivada", paso: "derivada" };
const minus = (t: string) => t.charAt(0).toLowerCase() + t.slice(1);

/** El enlace de un paso: un espejo, su página (14i); si no, la página de su destino si la tiene; si no, los espejos de quien lo hace o el repositorio. */
function enlace(p: PasoVista, rc: ReglaCopia, cliente: string): string | null {
  if (p.tipo === "espejo" && p.equipoId && p.trabajo) return hrefEspejo(cliente, p.equipoId, p.trabajo);
  if (p.clave && !p.clave.startsWith("espejo:")) return hrefDestino(cliente, p.clave);
  if (p.tipo === "espejo" && p.equipoId) return `/c/${cliente}/equipos/${p.equipoId}/espejos`;
  return `/c/${cliente}/equipos/${rc.equipo.id}/repositorios/${encodeURIComponent(rc.repo.id)}`;
}

/** El estado de un paso que no es la copia: lo que dice su última vuelta y si está al día. */
function estadoPaso(p: PasoVista, hace: Equipo | undefined, ahora: number): Pick<Eslabon, "tono" | "estado" | "cuando"> {
  const i = p.intento;
  if (p.pausado) return { tono: "paused", estado: "En pausa", cuando: i?.cuando ?? null };
  if (i?.mal) return { tono: "bad", estado: "Falló", cuando: i.cuando };
  if (pasoAlDia(p, ahora)) return { tono: i?.aviso ? "warn" : "ok", estado: i?.aviso ? "Con avisos" : "Al día", cuando: p.ultima_ok ?? null };
  // Quien lo hace no se conecta: lo más probable es eso, no que falle.
  if (hace?.conectado === false) return { tono: "warn", estado: "Sin conexión", cuando: p.ultima_ok ?? null };
  if (p.ultima_ok) return { tono: "warn", estado: "Atrasado", cuando: p.ultima_ok };
  return { tono: "neutral", estado: "Todavía sin ninguna", cuando: null };
}

/**
 * El camino de una copia: su repositorio y, debajo, cada espejo y cada copia
 * derivada (con los espejos del almacén primero, como en la regla). Vacío si
 * la copia no tiene regla (desactivada, o un repositorio importado).
 */
export function caminoDeCopia(rc: ReglaCopia | null, o: OpcionesCamino): Eslabon[] {
  if (!rc) return [];
  const e = rc.equipo;
  const vivo = o.enVivo ?? (() => null);
  const equipos = o.equipos ?? [];
  return rc.pasos.map((p): Eslabon => {
    const clase = CLASE[p.tipo] ?? "derivada";
    const base = {
      id: p.id,
      clase,
      nombre: p.nombre,
      quien: p.equipoId && p.equipoId !== e.id && clase === "espejo" ? p.equipoNombre : null,
      tipo: corta(p.clasificacion),
      despues: !!p.despues,
      href: enlace(p, rc, o.cliente),
    };
    if (clase === "copia") {
      const enMarcha = vivo(e.id, rc.repo.id, "copia");
      if (enMarcha) return { ...base, tono: "info", estado: enMarcha, cuando: null, vivo: true };
      const vuelta = ultimaVuelta(rc.copia, o.informe ?? null);
      const est = estadoCopia(rc.copia, vuelta, enPausa(e, o.ahora), o.ahora, e.resumen?.movil);
      const texto = est.texto === "Con error" ? "Falló" : est.texto === "Atrasada" ? "Atrasado" : est.texto === "Sin copias todavía" ? "Todavía sin ninguna" : est.texto;
      const sinConexion = est.tono === "warn" && atrasada(rc.copia, vuelta, false, o.ahora) && e.conectado === false;
      return { ...base, tono: est.tono, estado: sinConexion ? "Sin conexión" : texto, cuando: vuelta?.cuando ?? null, vivo: false };
    }
    if (clase === "derivada") {
      const enMarcha = vivo(e.id, rc.repo.id, "copia_externa");
      if (enMarcha) return { ...base, tono: "info", estado: enMarcha.replace(/^Copiando/, "Subiendo"), cuando: null, vivo: true };
      return { ...base, ...estadoPaso(p, e, o.ahora), vivo: false };
    }
    const hace = equipos.find((x) => x.id === p.equipoId) ?? (p.equipoId === e.id ? e : undefined);
    // 14h/14i: su trabajo (lo que ocupa este repositorio allí y su última comprobación) y, 14p, su subida en vivo.
    const t: TrabajoEspejoResumen | undefined = hace && p.trabajo ? [...trabajosDelAlmacen(hace), ...trabajosDelEquipo(hace)].find((x) => x.id === p.trabajo) : undefined;
    const extra = { ocupa: p.ocupaClave ? (t?.ocupa?.repos?.[p.ocupaClave] ?? null) : null, ocupaCuando: t?.ocupa?.cuando ?? null, verificacion: t?.verificacion ?? null };
    const enMarcha = hace && p.trabajo ? vivo(hace.id, p.trabajo, "espejo") : null;
    if (enMarcha) return { ...base, ...extra, tono: "info", estado: enMarcha, cuando: null, vivo: true };
    return { ...base, ...extra, ...estadoPaso(p, hace, o.ahora), vivo: false };
  });
}

/** El estado de un eslabón en una frase corta: «al día, hace 2 h», «subiendo 45 %», «falló hace 3 h». */
export function textoEslabon(x: Pick<Eslabon, "estado" | "cuando" | "vivo" | "tono">, ahora: number): string {
  if (x.vivo || !x.cuando) return minus(x.estado);
  const r = relativo(x.cuando, ahora);
  if (x.tono === "bad") return `falló ${r}`;
  if (x.estado === "Atrasado" || x.estado === "Sin conexión") return `${minus(x.estado)} · la última, ${r}`;
  return `${minus(x.estado)}, ${r}`;
}

/** La línea de un paso para la lista de «Copias»: «→ Backblaze · al día, hace 2 h». */
export function lineaEslabon(x: Eslabon, ahora: number): string {
  return `→ ${x.nombre} · ${textoEslabon(x, ahora)}`;
}

/** ¿Pide atención? (Falló, atrasado o sin conexión; en marcha o en pausa, no.) */
export const eslabonMal = (x: Pick<Eslabon, "tono" | "vivo">) => !x.vivo && (x.tono === "bad" || x.tono === "warn");

export interface ResumenCamino {
  /** Pasos después del repositorio (espejos y derivadas). */
  pasos: number;
  espejos: number;
  derivadas: number;
  /** Pasos que fallan, van atrasados o no se conectan. */
  mal: number;
  /** El peor tono de los pasos (sin el repositorio de la copia). */
  tono: Tono | null;
}

const PESO_TONO: Record<Tono, number> = { bad: 0, warn: 1, info: 2, paused: 3, neutral: 4, ok: 5 };

/** Lo que hay después del repositorio de la copia, en cifras. */
export function resumenCamino(xs: Eslabon[]): ResumenCamino {
  const pasos = xs.filter((x) => x.clase !== "copia");
  const peor = [...pasos].sort((a, b) => PESO_TONO[a.tono] - PESO_TONO[b.tono])[0];
  return {
    pasos: pasos.length,
    espejos: pasos.filter((x) => x.clase === "espejo").length,
    derivadas: pasos.filter((x) => x.clase === "derivada").length,
    mal: pasos.filter(eslabonMal).length,
    tono: peor?.tono ?? null,
  };
}

/** El camino entero en una frase (para lectores de pantalla y el globo): «Almacén · Disco D (al día) → después, Backblaze (falló hace 3 h)». */
export function caminoEnTexto(xs: Eslabon[], ahora: number): string {
  return xs.map((x, i) => `${i && x.despues ? "después, " : ""}${x.nombre}${x.clase === "espejo" ? " (espejo" : x.clase === "derivada" ? " (copia derivada" : " ("}${x.clase === "copia" ? "" : ", "}${textoEslabon(x, ahora)})`).join(" → ");
}
