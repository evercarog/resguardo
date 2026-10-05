// Lo que se muestra de una copia (un plan del equipo): su resumen
// (`resumen.copias[]`), la fila del último informe (`informe.datos.copias[]`,
// con sus ganchos), la próxima vez que toca (`informe.datos.proximas`, v1.12)
// y, del informe detallado de su repositorio, solo sus vueltas y versiones.
// Sin informe detallado, la página sigue funcionando con el resumen.
import type { CopiaResumen, EjecucionInforme, Informe, RepoInforme } from "./tipos";
import type { Tono } from "./salud";
import { cuandoFrase, horarioEnFrase, lista, plural, relativo } from "./formato";
import { informeDe } from "./repo";
import { proximaVez, reglasDe, ultimaVez } from "./horario";

/** El informe del repositorio con solo las versiones y vueltas de esta copia. */
export function infCopia(inf: RepoInforme | null, copia: string): RepoInforme | null {
  if (!inf) return null;
  return { ...inf, versiones: inf.versiones.filter((v) => v.copia === copia), ejecuciones: inf.ejecuciones.filter((e) => e.copia === copia) };
}

/** La fila de la copia en el último informe del equipo (resultado y ganchos). */
export const filaInforme = (informe: Informe | null | undefined, k: Pick<CopiaResumen, "id" | "repo">) =>
  informe?.datos.copias?.find((x) => x.id === k.id && (!x.repo || x.repo === k.repo)) ?? null;

/**
 * Cuándo vuelve a tocar: del resumen, del informe (v1.12) o, con un agente
 * anterior que no lo dice, calculado del horario (null si está desactivada).
 */
export function proximaDe(k: CopiaResumen, informe: Informe | null | undefined, ahora = Date.now()): string | null {
  const dicha = k.proxima ?? informe?.datos.proximas?.[k.id];
  if (dicha !== undefined) return dicha;
  if (k.activa === false) return null;
  const t = proximaProgramada(k.horario, ahora);
  return t == null ? null : new Date(t).toISOString();
}

export type ResultadoVuelta = EjecucionInforme["resultado"];
export interface Vuelta {
  cuando: string;
  resultado: ResultadoVuelta;
  mensaje: string | null;
}

const resultadoDe = (estado: string | undefined): ResultadoVuelta => (estado === "ok" ? "ok" : estado === "aviso" ? "aviso" : "fallo");

/**
 * La última vuelta de la copia, de lo más reciente que haya: las vueltas del
 * informe detallado (distinguen «sin cambios»), la fila del informe o el
 * resumen (que solo se renueva cuando cambia la configuración).
 */
export function ultimaVuelta(k: CopiaResumen, informe: Informe | null | undefined): Vuelta | null {
  const candidatas: Vuelta[] = [];
  const ej = infCopia(informeDe(informe, k.repo), k.id)?.ejecuciones[0];
  if (ej) candidatas.push({ cuando: ej.hora, resultado: ej.resultado, mensaje: ej.mensaje_corto });
  const fila = filaInforme(informe, k);
  if (fila?.cuando && fila.estado) candidatas.push({ cuando: fila.cuando, resultado: resultadoDe(fila.estado), mensaje: fila.mensaje ?? null });
  if (k.ultima) candidatas.push({ cuando: k.ultima.cuando, resultado: resultadoDe(k.ultima.estado), mensaje: k.ultima.mensaje ?? null });
  // La más reciente; a igual hora, la primera (la del informe detallado).
  return candidatas.reduce<Vuelta | null>((a, b) => (!a || Date.parse(b.cuando) > Date.parse(a.cuando) ? b : a), null);
}

/**
 * La última hora programada que ya pasó (hasta 8 días atrás; más con «cada N
 * días» o «cada mes»), con la hora del navegador. Null si el horario viene en
 * frase o está vacío.
 */
export function ultimaProgramada(horario: CopiaResumen["horario"], ahora = Date.now()): number | null {
  if (!horario || typeof horario === "string") return null;
  const reglas = reglasDe(horario);
  return reglas.length ? ultimaVez(reglas, ahora) : null;
}

/** La próxima hora programada, con la hora del navegador; null sin horario. */
export function proximaProgramada(horario: CopiaResumen["horario"], ahora = Date.now()): number | null {
  if (!horario || typeof horario === "string") return null;
  const reglas = reglasDe(horario);
  return reglas.length ? proximaVez(reglas, ahora) : null;
}

/** Margen antes de llamar atrasada a una copia (lo que puede tardar en empezar y terminar). */
const MARGEN = 2 * 3600_000;

/**
 * ¿Atrasada? Activa, sin pausa, y desde su última hora programada (con margen)
 * no ha terminado ninguna vuelta. Una copia que nunca se hizo no cuenta (no se
 * sabe desde cuándo existe): sale como «Sin copias todavía».
 */
export function atrasada(k: CopiaResumen, vuelta: Vuelta | null, pausada: boolean, ahora = Date.now()): boolean {
  if (k.activa === false || pausada || !vuelta) return false;
  const slot = ultimaProgramada(k.horario, ahora);
  if (slot == null || slot > ahora - MARGEN) return false;
  return Date.parse(vuelta.cuando) < slot;
}

export interface EstadoCopia {
  tono: Tono;
  texto: string;
}

export function estadoCopia(k: CopiaResumen, vuelta: Vuelta | null, pausada: boolean, ahora = Date.now()): EstadoCopia {
  if (k.activa === false) return { tono: "neutral", texto: "Desactivada" };
  if (pausada) return { tono: "paused", texto: "En pausa" };
  if (vuelta?.resultado === "fallo") return { tono: "bad", texto: "Con error" };
  if (atrasada(k, vuelta, pausada, ahora)) return { tono: "warn", texto: "Atrasada" };
  if (vuelta?.resultado === "aviso") return { tono: "warn", texto: "Con avisos" };
  if (vuelta) return { tono: "ok", texto: "Al día" };
  return { tono: "neutral", texto: "Sin copias todavía" };
}

const TEXTO_VUELTA: Record<ResultadoVuelta, string> = { ok: "fue correcta", sin_cambios: "no encontró cambios", aviso: "terminó con avisos", fallo: "falló" };

/** Una frase que lo resume: qué, de dónde, a dónde, cuándo, y cómo fue la última. */
export function fraseCopia(
  k: CopiaResumen,
  o: { equipo: string; repo: string; vuelta: Vuelta | null; proxima: string | null; pausada: boolean; ganchos?: string[] },
  ahora = Date.now(),
): string {
  const que = k.carpetas != null ? plural(k.carpetas, "carpeta", "carpetas") : "sus carpetas";
  const antes = o.ganchos?.length ? ` (antes, ${lista(o.ganchos)})` : "";
  const partes = [`Copia ${que} de ${o.equipo} en «${o.repo}»${antes}: ${horarioEnFrase(k.horario).replace(/^./, (c) => c.toLowerCase())}.`];
  if (k.activa === false) partes.push("Ahora está desactivada: no se hará hasta que la actives.");
  else if (o.pausada) partes.push("Las copias automáticas están en pausa.");
  if (o.vuelta) partes.push(`La última copia, ${relativo(o.vuelta.cuando, ahora)}, ${TEXTO_VUELTA[o.vuelta.resultado]}.`);
  else partes.push("Todavía no se ha hecho ninguna vez.");
  if (o.proxima && k.activa !== false && Date.parse(o.proxima) > ahora) partes.push(`La próxima será ${cuandoFrase(o.proxima, ahora)}.`);
  return partes.join(" ");
}

export interface CifrasCopia {
  vueltas: number;
  correctas: number;
  fallidas: number;
  conAvisos: number;
  sinCambios: number;
  /** Media de las vueltas que tienen duración (de las vueltas o, si no, de las versiones). */
  duracionMedia: number | null;
  /** Lo que añadieron sus versiones en estos 60 días (empaquetado si se sabe). */
  anadido: number | null;
  /** Lo que ocupan sus carpetas, según su última versión. */
  tamano: number | null;
}

export function cifrasCopia(inf: RepoInforme | null): CifrasCopia {
  const ej = inf?.ejecuciones ?? [];
  const vs = inf?.versiones ?? [];
  const cuenta = (r: ResultadoVuelta) => ej.filter((e) => e.resultado === r).length;
  const durs = (ej.some((e) => e.duracion_s != null) ? ej.map((e) => e.duracion_s) : vs.map((v) => v.duracion_s)).filter((x): x is number => x != null);
  const anadidos = vs.map((v) => v.anadido_empaquetado ?? v.anadido).filter((x): x is number => x != null);
  return {
    vueltas: ej.length,
    correctas: cuenta("ok") + cuenta("sin_cambios"),
    fallidas: cuenta("fallo"),
    conAvisos: cuenta("aviso"),
    sinCambios: cuenta("sin_cambios"),
    duracionMedia: durs.length ? durs.reduce((a, b) => a + b, 0) / durs.length : null,
    anadido: anadidos.length ? anadidos.reduce((a, b) => a + b, 0) : null,
    tamano: vs[0]?.total_bytes ?? null,
  };
}

// ---------------------------------------------------------------------------
// Errores en palabras sencillas
// ---------------------------------------------------------------------------

export interface Explicacion {
  titulo: string;
  texto: string;
  /** Ancla del centro de ayuda (/ayuda#…). */
  ayuda: string;
}

const EXPLICACIONES: [RegExp, Explicacion][] = [
  [
    /volcado|sql ?server|sqlcmd|backup database/i,
    { titulo: "Falló el volcado de la base de datos", texto: "El paso «Antes de copiar» no pudo volcar la base de SQL Server. Suele ser un permiso de la cuenta del equipo o el nombre de la base o de la instancia.", ayuda: "si-volcado" },
  ],
  [
    /contrase(ñ|n)a|password|wrong key|no se pudo abrir el repositorio/i,
    { titulo: "El repositorio no se abrió", texto: "El equipo no pudo abrir el repositorio con la contraseña que tiene guardada. Lo ya copiado sigue a salvo; hace falta la contraseña de su kit.", ayuda: "si-contrasena-repo" },
  ],
  [
    /bloque|lock/i,
    { titulo: "El repositorio estaba bloqueado", texto: "Otra copia o comprobación lo tenía ocupado, o quedó un bloqueo de una copia que se cortó. En la ficha del equipo, «Quitar bloqueos antiguos» lo arregla sin borrar nada.", ayuda: "si-copia-falla" },
  ],
  [
    /espacio|no space|disk full|lleno|quota|cuota/i,
    { titulo: "No queda sitio en el destino", texto: "El disco o el servicio donde se guardan las copias está lleno. Libera espacio o revisa la retención para que guarde menos versiones.", ayuda: "si-copia-falla" },
  ],
  [
    /conectar|conexi(ó|o)n|connection|timeout|tiempo de espera|unreachable|no se encuentra el servidor|10061|10060|dns|red\b/i,
    { titulo: "No se pudo llegar al destino", texto: "El equipo no alcanzó el sitio donde guarda las copias (otro equipo, un NAS o la nube). Comprueba que está encendido y con red; la próxima copia lo vuelve a intentar sola.", ayuda: "si-copia-falla" },
  ],
  [
    /en uso|no se pudieron leer|no se pudo leer|acceso denegado|access denied|permiso/i,
    { titulo: "Algunos archivos no se pudieron leer", texto: "Estaban abiertos por otro programa o sin permiso. El resto se copió; esos entran en la próxima copia si están libres.", ayuda: "si-copia-falla" },
  ],
  [
    /carpeta.*(no existe|no se encuentra)|not found|no existe/i,
    { titulo: "Falta una carpeta", texto: "Alguna de las carpetas de la copia ya no está (se movió, se borró o es de un disco que no está conectado). Revísalas en «Cambiar».", ayuda: "si-copia-falla" },
  ],
];

const GENERICA: Explicacion = { titulo: "La copia no terminó bien", texto: "El mensaje del equipo dice qué pasó. Lo ya copiado sigue a salvo; la próxima copia lo vuelve a intentar.", ayuda: "si-copia-falla" };

export function explicarError(mensaje: string | null | undefined): Explicacion {
  const m = mensaje ?? "";
  return EXPLICACIONES.find(([re]) => re.test(m))?.[1] ?? GENERICA;
}
