// El editor de copias guiado (plan 0.7.26, bloque 3; docs/editor-de-copias.md
// «Guiado y avanzado»). Sin estado ni red: se prueba en
// scripts/vectores-editor-guiado.ts.
//
// Las dos formas de editar una copia (guiada, paso a paso, y «Avanzado», la
// tarjeta entera) cambian la misma `CopiaConfig` con **estas mismas
// funciones**, así que producen la misma configuración. Los vectores lo
// comprueban: cada atajo del modo guiado da lo mismo que su equivalente en el
// editor completo.
import { horarioParaEnviar, reglasDe } from "./horario";
import { horarioEnFrase, plural } from "./formato";
import type { CopiaConfig, Horario, ReglaHorario } from "./tipos";

// --- Cuándo empieza (contrato: `inicio`, `tras`, `retraso_min`) --------------

/** Lo que entiende el agente de «después de la anterior» aunque falle y del retraso. */
export const ADMITE_INICIO_DESPUES = "inicio_despues";

/** «Con horario», «En cadena» (solo si la anterior sale bien) o «Después de la anterior» (siempre). */
export type ModoInicio = "horario" | "cadena" | "despues";

export const TEXTO_INICIO: Record<ModoInicio, string> = {
  horario: "Con horario",
  cadena: "En cadena",
  despues: "Después de la anterior",
};
export const AYUDA_INICIO: Record<ModoInicio, string> = {
  horario: "Empieza a sus horas",
  cadena: "Cuando la anterior termina bien. Si falla, esta no se hace y se avisa",
  despues: "Cuando la anterior termina, salga bien o mal",
};

/** Retrasos que se ofrecen (minutos). */
export const RETRASOS = [5, 10, 15, 30, 60] as const;
export const MAX_RETRASO = 1440;

/** Cómo empieza una copia. Sin `inicio` y con `tras`: en cadena (lo de siempre). */
export function inicioDe(k: Pick<CopiaConfig, "tras" | "inicio">): ModoInicio {
  if (!k.tras) return "horario";
  return k.inicio === "despues" ? "despues" : "cadena";
}

/** El horario que se pone al volver a «Con horario» (de lunes a viernes a las 13:00, como una copia nueva). */
export const HORARIO_POR_DEFECTO: Horario = { dias: [1, 2, 3, 4, 5], horas: ["13:00"] };

const conHorario = (h: Horario | null | undefined) => !!h && (!!h.reglas?.length || (!!h.dias?.length && !!h.horas?.length));
const copiar = <T>(x: T): T => JSON.parse(JSON.stringify(x)) as T;

/**
 * Cambia cómo empieza la copia `i`. En cadena o después, sin horario propio
 * (solo «Inmediatamente» o «Con retraso de N min»); la primera no puede.
 * `horarioAlVolver`: el que tenía antes (si se sabe), para «Con horario».
 */
export function ponerInicio(copias: CopiaConfig[], i: number, modo: ModoInicio, horarioAlVolver?: Horario | null): void {
  const k = copias[i];
  if (!k) return;
  if (modo === "horario" || i === 0) {
    k.tras = null;
    delete k.inicio;
    delete k.retraso_min;
    if (!conHorario(k.horario)) k.horario = copiar(conHorario(horarioAlVolver) ? horarioAlVolver! : HORARIO_POR_DEFECTO);
    return;
  }
  k.tras = copias[i - 1].id;
  k.horario = { dias: [], horas: [] };
  if (modo === "despues") k.inicio = "despues";
  else delete k.inicio;
}

/** «Inmediatamente» (0) o «Con retraso de N min». */
export function ponerRetraso(k: CopiaConfig, min: number): void {
  const n = Math.round(Number(min));
  if (!k.tras || !Number.isFinite(n) || n <= 0) delete k.retraso_min;
  else k.retraso_min = Math.min(n, MAX_RETRASO);
}

/** Tras ordenar o quitar: la que ya no va después de ninguna pierde `inicio` y el retraso (y toma horario si no tenía). */
export function limpiarInicio(copias: CopiaConfig[]): void {
  for (const k of copias) {
    if (k.tras) continue;
    delete k.inicio;
    delete k.retraso_min;
    if (!conHorario(k.horario)) k.horario = copiar(HORARIO_POR_DEFECTO);
  }
}

/** Lo que falla en cómo empieza una copia (para «Antes de enviar»), o null. */
export function errorInicio(copias: Pick<CopiaConfig, "id" | "nombre" | "tras" | "inicio" | "retraso_min">[], i: number, a: { cadenas: boolean; despues: boolean }): string | null {
  const k = copias[i];
  if (!k?.tras) return null;
  if (i === 0) return `«${k.nombre}» es la primera: empieza con su horario.`;
  if (!a.cadenas) return `«${k.nombre}» va después de otra copia y el agente de este equipo aún no lo admite.`;
  if ((k.inicio === "despues" || (k.retraso_min ?? 0) > 0) && !a.despues) return `«${k.nombre}»: actualiza el agente para «Después de la anterior» o un retraso.`;
  const r = k.retraso_min ?? 0;
  if (!Number.isInteger(r) || r < 0 || r > MAX_RETRASO) return `«${k.nombre}»: el retraso va de 0 a ${MAX_RETRASO} minutos.`;
  return null;
}

/** «Inmediatamente», «Con 15 min de retraso», «Con 2 h de retraso». */
export function retrasoEnFrase(min: number | null | undefined): string {
  const n = min ?? 0;
  if (n <= 0) return "Inmediatamente";
  if (n % 60 === 0) return `Con ${n / 60} h de retraso`;
  return `Con ${n} min de retraso`;
}

/** Cuándo empieza, en una línea: «Cada día laborable a las 13:00», «En cadena tras «Documentos» · inmediatamente». */
export function cuandoEnFrase(copias: Pick<CopiaConfig, "id" | "nombre" | "tras" | "inicio" | "retraso_min" | "horario">[], i: number): string {
  const k = copias[i];
  if (!k) return "";
  const modo = inicioDe(k);
  if (modo === "horario") return horarioEnFrase(k.horario);
  const anterior = copias.find((x) => x.id === k.tras)?.nombre ?? "la anterior";
  const r = retrasoEnFrase(k.retraso_min).toLowerCase();
  return modo === "cadena" ? `En cadena tras «${anterior}» · ${r}` : `Después de «${anterior}» · ${r}`;
}

// --- Plantillas rápidas del horario --------------------------------------------

export type PlantillaHorario = "cada_hora" | "cada_dia" | "laborables";
export const TEXTO_PLANTILLA: Record<PlantillaHorario, string> = {
  cada_hora: "Cada hora",
  cada_dia: "Cada día a las…",
  laborables: "Laborables a las…",
};
const TODOS = [1, 2, 3, 4, 5, 6, 7];
const LABORABLES = [1, 2, 3, 4, 5];

/** Las reglas de una plantilla (lo mismo que se elegiría en el editor completo). */
export function reglasDePlantilla(p: PlantillaHorario, hora = "21:00"): ReglaHorario[] {
  if (p === "cada_hora") return [{ tipo: "intervalo", dias: [...TODOS], cada_min: 60, desde: "00:00", hasta: "23:00" }];
  return [{ tipo: "horas", dias: p === "cada_dia" ? [...TODOS] : [...LABORABLES], horas: [hora] }];
}

/** Pone en la copia el horario de una plantilla, como lo guardaría el editor completo. */
export function aplicarPlantilla(k: CopiaConfig, p: PlantillaHorario, hora: string, admiteReglas: boolean): void {
  k.horario = horarioParaEnviar(reglasDePlantilla(p, hora), admiteReglas);
}

/** ¿Es el horario una de las plantillas? Con cuál y a qué hora; si no, «personalizado». */
export function plantillaDe(h: Horario | null | undefined): { plantilla: PlantillaHorario; hora: string } | { plantilla: "personalizado" } {
  const r = reglasDe(h, true);
  if (r.length !== 1) return { plantilla: "personalizado" };
  const x = r[0];
  const dias = (d: number[]) => [...new Set(d)].sort((a, b) => a - b).join();
  if (x.tipo === "intervalo" && x.cada_min === 60 && x.desde === "00:00" && x.hasta === "23:00" && dias(x.dias) === TODOS.join()) return { plantilla: "cada_hora", hora: "" };
  if (x.tipo === "horas" && x.horas.length === 1) {
    if (dias(x.dias) === TODOS.join()) return { plantilla: "cada_dia", hora: x.horas[0] };
    if (dias(x.dias) === LABORABLES.join()) return { plantilla: "laborables", hora: x.horas[0] };
  }
  return { plantilla: "personalizado" };
}

// --- Copias nuevas y duplicadas ----------------------------------------------------

const idNuevo = () => `copia-${crypto.randomUUID().slice(0, 8)}`;

/** Una copia nueva (la de siempre: de lunes a viernes a las 13:00, con las exclusiones habituales). */
export function copiaNueva(repo: string, id = idNuevo()): CopiaConfig {
  return {
    id,
    nombre: "Nueva copia",
    repo,
    carpetas: [],
    exclusiones: [...EXCLUSIONES_POR_DEFECTO],
    horario: copiar(HORARIO_POR_DEFECTO),
    activa: true,
    gancho: [],
    solo_si_cambios: true,
  };
}

/** «Duplicar»: lo mismo con otro id y nombre, con su propio horario (nunca en cadena: va justo debajo y se decide después). */
export function duplicarCopia(k: CopiaConfig, id = idNuevo()): CopiaConfig {
  const d = copiar(k);
  d.id = id;
  d.nombre = `${k.nombre} (copia)`.slice(0, 60);
  d.tras = null;
  delete d.inicio;
  delete d.retraso_min;
  if (!conHorario(d.horario)) d.horario = copiar(HORARIO_POR_DEFECTO);
  return d;
}

// --- Exclusiones ----------------------------------------------------------------

/** Las exclusiones que se ponen por defecto (las de siempre en una copia nueva). */
export const EXCLUSIONES_POR_DEFECTO = ["*.tmp", "~$*", "Thumbs.db"];
/** Las habituales, para añadirlas con un toque. */
export const EXCLUSIONES_HABITUALES: { regla: string; texto: string }[] = [
  { regla: "*.tmp", texto: "Temporales" },
  { regla: "~$*", texto: "Bloqueos de Office" },
  { regla: "Thumbs.db", texto: "Miniaturas de Windows" },
  { regla: ".DS_Store", texto: "Miniaturas de Mac" },
  { regla: "node_modules", texto: "node_modules" },
  { regla: "$RECYCLE.BIN", texto: "Papelera" },
];
/** Pone o quita una exclusión (sin repetir, en el orden en que estaban). */
export function alternarExclusion(k: CopiaConfig, regla: string): void {
  k.exclusiones = k.exclusiones.includes(regla) ? k.exclusiones.filter((x) => x !== regla) : [...k.exclusiones, regla];
}

// --- Los pasos -------------------------------------------------------------------

export type PasoCopia = "cuando" | "que" | "donde" | "resumen";
export const PASOS_COPIA: PasoCopia[] = ["cuando", "que", "donde", "resumen"];
export const TITULO_PASO: Record<PasoCopia, string> = { cuando: "Cuándo", que: "Qué", donde: "Dónde", resumen: "Resumen" };

/** ¿Está hecho el paso? (Para saber hasta dónde se puede avanzar.) */
export function pasoHecho(k: Pick<CopiaConfig, "carpetas" | "repo" | "horario" | "tras">, p: PasoCopia): boolean {
  if (p === "cuando") return !!k.tras || conHorario(k.horario);
  if (p === "que") return k.carpetas.length > 0;
  if (p === "donde") return !!k.repo;
  return false;
}

/** El primer paso por hacer (una copia nueva empieza en «Cuándo»; una completa, en el resumen). */
export function primerPaso(k: Pick<CopiaConfig, "carpetas" | "repo" | "horario" | "tras">): PasoCopia {
  return PASOS_COPIA.find((p) => p !== "resumen" && !pasoHecho(k, p)) ?? "resumen";
}

/** Qué copia, en una línea: «1 carpeta», «3 carpetas · sin 4 tipos de archivo». */
export function queEnFrase(k: Pick<CopiaConfig, "carpetas" | "exclusiones">): string {
  if (!k.carpetas.length) return "Sin carpetas";
  const n = plural(k.carpetas.length, "carpeta", "carpetas");
  return k.exclusiones.length ? `${n} · ${plural(k.exclusiones.length, "exclusión", "exclusiones")}` : n;
}

// --- «+ Añadir» -------------------------------------------------------------------

export type QueAnadir = "carpetas" | "espejo" | "derivada";
export const TEXTO_ANADIR: Record<QueAnadir, { titulo: string; detalle: string }> = {
  carpetas: { titulo: "Copiar carpetas de este equipo", detalle: "Una copia nueva" },
  espejo: { titulo: "Espejo", detalle: "El mismo repositorio en otro destino" },
  derivada: { titulo: "Copia derivada", detalle: "Versiones de un repositorio a otro" },
};

/** Los pasos de la copia derivada guiada (el repositorio de origen se elige antes, o ya viene de la copia). */
export type PasoDerivada = "origen" | "versiones" | "cuando" | "donde" | "resumen";
export const PASOS_DERIVADA: PasoDerivada[] = ["origen", "versiones", "cuando", "donde", "resumen"];
export const TITULO_PASO_DERIVADA: Record<PasoDerivada, string> = { origen: "De qué repositorio", versiones: "Qué versiones", cuando: "Cuándo", donde: "Dónde", resumen: "Resumen" };

// --- Lo que propone el resumen para la regla 3-2-1-1-0 -------------------------

export type Sugerencia = "verificacion" | "prueba" | "espejo" | "derivada";
export const TEXTO_SUGERENCIA: Record<Sugerencia, string> = {
  verificacion: "Activar la verificación semanal",
  prueba: "Activar la prueba de restauración mensual",
  espejo: "Añadir un espejo",
  derivada: "Añadir una copia derivada",
};

/**
 * Las sugerencias de un clic para lo que le falta a la copia en la regla
 * (por los códigos de acción de `regla321`), sin repetir y solo las que se
 * pueden hacer con este equipo.
 */
export function sugerencias(acciones: string[], puede: { verificacion: boolean; prueba: boolean; espejo: boolean; derivada: boolean }): Sugerencia[] {
  const out: Sugerencia[] = [];
  const poner = (s: Sugerencia) => puede[s] && !out.includes(s) && out.push(s);
  for (const a of acciones) {
    if (a === "programar_verificacion") poner("verificacion");
    else if (a === "programar_prueba") poner("prueba");
    else if (a === "anadir_destino" || a === "otro_soporte") {
      poner("espejo");
      poner("derivada");
    } else if (a === "anadir_fuera" || a === "anadir_inmutable") {
      // Fuera del sitio o inmutable: lo más directo es la nube (copia derivada); el espejo, a una nube del almacén.
      poner("derivada");
      poner("espejo");
    }
  }
  return out;
}

// --- La preferencia «Avanzado» (por persona, en este navegador) -----------------

const CLAVE_AVANZADO = "rg.copias.avanzado";
/** ¿Quiere esta persona ver las copias enteras? (Sin poder leer el navegador: guiado.) */
export function leerAvanzado(persona: string | null | undefined): boolean {
  try {
    return localStorage.getItem(`${CLAVE_AVANZADO}.${persona ?? ""}`) === "1";
  } catch {
    return false;
  }
}
export function guardarAvanzado(persona: string | null | undefined, si: boolean): void {
  try {
    localStorage.setItem(`${CLAVE_AVANZADO}.${persona ?? ""}`, si ? "1" : "0");
  } catch {
    /* sin almacenamiento (ventana privada): solo para esta visita */
  }
}
