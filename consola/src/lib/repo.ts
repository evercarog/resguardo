// Lo que se muestra de un repositorio: el resumen del equipo
// (`resumen.repositorios[]`) y, si el equipo ya lo envía, su informe
// detallado (`informe.datos.repos[]`, api-servidor.md v1.7 §6): versiones de
// 60 días, vueltas de las copias, espacio, verificación, prueba de
// restauración, copia externa y salud de la protección. Sin informe, todo
// sigue funcionando con el resumen.
import type { ComprobacionProteccion, CopiaResumen, EjecucionInforme, EstadoComprobacion, Informe, RepoInforme, RepositorioResumen, TareaInforme, VersionInforme } from "./tipos";
import { copiaAtrasada, type Tono } from "./salud";
import { bytes, cuandoFrase, numero, plural, relativo } from "./formato";

/** El informe detallado de un repositorio, del último informe de su equipo. */
export const informeDe = (informe: Informe | null | undefined, repo: string): RepoInforme | null =>
  informe?.datos?.repos?.find((x) => x.id === repo) ?? null;

/**
 * El destino de un repositorio en el resumen de su equipo. El agente pone en
 * `repositorios[].destino` el **nombre** del destino (api-servidor.md §4); el
 * modo de demostración y algún agente de pruebas, su id. Se acepta lo uno y lo
 * otro (primero el id): buscarlo solo por id dejaba sin destino a todos los
 * repositorios de verdad (sin «Retención en el almacén», ni su almacén en
 * Repositorios, ni el destino en su página).
 */
export function destinoDe<D extends { id: string; nombre: string }>(destinos: D[] | null | undefined, repo: { destino: string } | null | undefined): D | undefined {
  if (!repo || !destinos) return undefined;
  return destinos.find((d) => d.id === repo.destino) ?? destinos.find((d) => d.nombre === repo.destino);
}

export const versionesDe = (inf: RepoInforme | null): VersionInforme[] => inf?.versiones ?? [];

/** Cuántas versiones tiene (el resumen cuenta todas; el informe, las de 60 días). */
export const nVersiones = (r: RepositorioResumen, inf?: RepoInforme | null): number => r.versiones ?? inf?.versiones.length ?? 0;

/** Lo que ocupan los archivos protegidos: del resumen o, si no lo trae, de la última versión del informe. */
export const bytesRepo = (r: RepositorioResumen, inf?: RepoInforme | null): number | null => r.bytes ?? inf?.versiones[0]?.total_bytes ?? null;

export const ultimaVersion = (r: RepositorioResumen, inf?: RepoInforme | null): string | null => r.ultima_version ?? inf?.versiones[0]?.hora ?? null;

/** Verificación y prueba de restauración: del informe con su resultado, o solo la fecha del resumen. */
export const verificacion = (r: RepositorioResumen, inf?: RepoInforme | null): TareaInforme | null =>
  inf?.verificacion ?? (r.verificado ? { ultima: r.verificado, resultado: "ok", mensaje_corto: null } : null);
export const pruebaRestauracion = (r: RepositorioResumen, inf?: RepoInforme | null): TareaInforme | null =>
  inf?.prueba_restauracion ?? (r.prueba_restauracion ? { ultima: r.prueba_restauracion, resultado: "ok", mensaje_corto: null } : null);

export const TONO_TAREA: Record<TareaInforme["resultado"], Tono> = { ok: "ok", aviso: "warn", fallo: "bad" };
export const TEXTO_TAREA: Record<TareaInforme["resultado"], string> = { ok: "Correcta", aviso: "Con avisos", fallo: "Falló" };

// ---------------------------------------------------------------------------
// Salud de la protección (protection.rs del agente)
// ---------------------------------------------------------------------------

export interface Proteccion {
  puntuacion: number;
  total: number;
  items: ComprobacionProteccion[];
  /** 0–1, para el anillo. */
  ratio: number;
  tono: "ok" | "warn" | "bad";
  pendientes: number;
}

export const TONO_COMPROBACION: Record<EstadoComprobacion, Tono> = { ok: "ok", aviso: "warn", fallo: "bad", desconocido: "neutral" };

/**
 * La salud de la protección que manda el agente. `extra` (v1.41): una
 * comprobación de la consola que el agente no hace (p. ej. «Fuera de este
 * equipo», de lib/dondeGuarda.ts): se suma si el agente no manda ya una con
 * ese id.
 */
export function proteccion(inf: RepoInforme | null | undefined, extra?: ComprobacionProteccion | null): Proteccion | null {
  const p0 = inf?.proteccion;
  if (!p0?.items?.length) return null;
  const p = extra && !p0.items.some((i) => i.id === extra.id) ? { puntuacion: p0.puntuacion + (extra.estado === "ok" ? 1 : 0), total: p0.total + 1, items: [...p0.items, extra] } : p0;
  const pendientes = p.items.filter((i) => i.estado !== "ok").length;
  return {
    puntuacion: p.puntuacion,
    total: p.total,
    items: p.items,
    ratio: p.puntuacion / Math.max(1, p.total),
    tono: p.items.some((i) => i.estado === "fallo") ? "bad" : pendientes ? "warn" : "ok",
    pendientes,
  };
}

// ---------------------------------------------------------------------------
// Cuadros por día (14 o 60): el más reciente a la derecha
// ---------------------------------------------------------------------------

export type EstadoDia = "datos" | "igual" | "aviso" | "mal" | "nada";
export interface Dia {
  clave: string;
  fecha: Date;
  estado: EstadoDia;
  /** Versiones de ese día. */
  n: number;
  titulo: string;
}

const p2 = (n: number) => String(n).padStart(2, "0");
export const claveDia = (d: Date) => `${d.getFullYear()}-${p2(d.getMonth() + 1)}-${p2(d.getDate())}`;
const fmtDiaLargo = new Intl.DateTimeFormat("es", { weekday: "long", day: "numeric", month: "long" });

const TEXTO_DIA: Record<EstadoDia, (n: number) => string> = {
  datos: (n) => plural(n, "versión", "versiones"),
  igual: () => "sin cambios (se revisó y estaba al día)",
  aviso: () => "con avisos",
  mal: () => "falló",
  nada: () => "sin copia",
};

/**
 * Estado de cada día a partir de las versiones y de las vueltas: con
 * versiones, bien (aviso si alguna vuelta tuvo avisos o falló); solo «sin
 * cambios», bien suave; solo fallos, mal; nada, vacío.
 */
export function dias(inf: RepoInforme | null, cuantos = 60, ahora = Date.now()): Dia[] {
  const porDia = new Map<string, { n: number; igual: boolean; fallo: boolean; aviso: boolean }>();
  const de = (k: string) => porDia.get(k) ?? (porDia.set(k, { n: 0, igual: false, fallo: false, aviso: false }), porDia.get(k)!);
  for (const v of versionesDe(inf)) de(claveDia(new Date(v.hora))).n++;
  for (const e of inf?.ejecuciones ?? []) {
    const x = de(claveDia(new Date(e.hora)));
    if (e.resultado === "sin_cambios") x.igual = true;
    else if (e.resultado === "fallo") x.fallo = true;
    else if (e.resultado === "aviso") x.aviso = true;
  }
  const hoy = new Date(ahora);
  return Array.from({ length: cuantos }, (_, i) => {
    const fecha = new Date(hoy.getFullYear(), hoy.getMonth(), hoy.getDate() - (cuantos - 1 - i));
    const clave = claveDia(fecha);
    const x = porDia.get(clave) ?? { n: 0, igual: false, fallo: false, aviso: false };
    const bien = x.n > 0 || x.igual;
    const estado: EstadoDia = x.fallo && !bien ? "mal" : x.aviso || (x.fallo && bien) ? "aviso" : x.n ? "datos" : x.igual ? "igual" : "nada";
    return { clave, fecha, estado, n: x.n, titulo: `${fmtDiaLargo.format(fecha)}: ${TEXTO_DIA[estado](x.n)}` };
  });
}

// ---------------------------------------------------------------------------
// Cifras y frase de resumen
// ---------------------------------------------------------------------------

/** Duración en palabras: «45 s», «3 min», «1 h 20 min». */
export function duracion(s: number | null | undefined): string {
  if (s == null) return "—";
  if (s < 60) return `${Math.round(s)} s`;
  const m = Math.round(s / 60);
  if (m < 60) return `${m} min`;
  return `${Math.floor(m / 60)} h${m % 60 ? ` ${m % 60} min` : ""}`;
}

export function duracionMedia(inf: RepoInforme | null): number | null {
  const xs = versionesDe(inf)
    .map((v) => v.duracion_s)
    .filter((x): x is number => x != null);
  return xs.length ? xs.reduce((a, b) => a + b, 0) / xs.length : null;
}

/** Lo que ocupó cada versión en el repositorio: el empaquetado si lo hay (comprimido y sin duplicados). */
export const anadidoDe = (v: VersionInforme) => v.anadido_empaquetado ?? v.anadido;

/**
 * La versión que dejó una vuelta: misma copia y la hora de la versión (cuando
 * empezó) entre el principio y el final de la vuelta (con margen; la vuelta
 * lleva la hora en que terminó).
 */
export function versionDeVuelta(versiones: VersionInforme[], e: EjecucionInforme): VersionInforme | null {
  if (e.resultado === "fallo" || e.resultado === "sin_cambios") return null;
  const fin = Date.parse(e.hora);
  // Sin la duración (agentes anteriores a 0.7.4), una ventana amplia: gana la más cercana.
  const margen = e.duracion_s != null ? (e.duracion_s + 120) * 1000 : 6 * 3600_000;
  let mejor: VersionInforme | null = null;
  for (const v of versiones) {
    if (v.copia !== e.copia) continue;
    const t = Date.parse(v.hora);
    const d = fin - t;
    if (d >= -60_000 && d <= margen && (!mejor || Math.abs(fin - Date.parse(mejor.hora)) > Math.abs(d))) mejor = v;
  }
  return mejor;
}

export const ultimaEjecucion = (inf: RepoInforme | null): EjecucionInforme | null => inf?.ejecuciones[0] ?? null;

export const TONO_RESULTADO: Record<EjecucionInforme["resultado"], Tono> = { ok: "ok", sin_cambios: "ok", aviso: "warn", fallo: "bad" };
export const TEXTO_RESULTADO: Record<EjecucionInforme["resultado"], string> = { ok: "Correcta", sin_cambios: "Sin cambios", aviso: "Con avisos", fallo: "Falló" };

/**
 * Una frase que lo resume todo, como en la app de escritorio: cuánto guarda,
 * cuándo fue la última y cómo está la protección.
 */
export function fraseRepo(r: RepositorioResumen, inf: RepoInforme | null, copias: CopiaResumen[], ahora = Date.now(), extra?: ComprobacionProteccion | null): string {
  const n = nVersiones(r, inf);
  const suyas = copias.filter((k) => k.repo === r.id);
  if (!n) {
    if (r.solo_lectura) return `«${r.nombre}» es un repositorio importado de otro equipo, todavía sin versiones que mostrar.`;
    const prox = suyas
      .map((k) => k.proxima)
      .filter((x): x is string => !!x && Date.parse(x) > ahora - 3600_000)
      .sort()[0];
    return prox ? `Todavía sin versiones: la primera copia será ${cuandoFrase(prox, ahora)}.` : "Todavía sin versiones: pulsa «Copiar ahora» para hacer la primera.";
  }
  const ult = ultimaVersion(r, inf);
  const tam = bytesRepo(r, inf);
  const partes = [`Guarda ${plural(n, "versión", "versiones")}${tam ? ` de ${bytes(tam)}` : ""}`];
  if (ult) partes.push(`la última, ${relativo(ult, ahora)}`);
  const ej = ultimaEjecucion(inf);
  if (ej?.resultado === "fallo") partes.push(`pero la última copia falló${ej.mensaje_corto ? ` (${ej.mensaje_corto.replace(/\.$/, "")})` : ""}`);
  const p = proteccion(inf, extra);
  const fin = p ? (p.pendientes ? `Protección ${p.puntuacion} de ${p.total}: ${p.pendientes === 1 ? "falta una cosa" : `faltan ${p.pendientes} cosas`}.` : "Protegido por todos los frentes.") : "";
  return `${partes.join("; ")}.${fin ? ` ${fin}` : ""}`;
}

export const ratioTexto = (x: number | null | undefined) => (x ? `${numero(Math.round(x * 10) / 10)}×` : "—");

/** Los cuadros de una sola copia (sus versiones y sus vueltas). */
export function diasCopia(inf: RepoInforme | null, copia: string, cuantos = 14, ahora = Date.now()): Dia[] {
  if (!inf) return [];
  return dias({ ...inf, versiones: inf.versiones.filter((v) => v.copia === copia), ejecuciones: inf.ejecuciones.filter((x) => x.copia === copia) }, cuantos, ahora);
}

/** El estado de un repositorio en una palabra (tarjetas y listas): falló, atrasado, al día… */
export function estadoRepo(r: RepositorioResumen, inf: RepoInforme | null, copias: CopiaResumen[], ahora = Date.now()): { tono: Tono; texto: string } {
  const suyas = copias.filter((k) => k.repo === r.id);
  const ej = ultimaEjecucion(inf);
  if (suyas.some((k) => k.ultima?.estado === "fallo") || ej?.resultado === "fallo") return { tono: "bad", texto: "Falló" };
  if (suyas.some((k) => copiaAtrasada(k, ahora))) return { tono: "warn", texto: "Atrasado" };
  if (ej) return { tono: TONO_RESULTADO[ej.resultado], texto: ej.resultado === "ok" ? "Al día" : TEXTO_RESULTADO[ej.resultado] };
  if (!nVersiones(r, inf)) return { tono: "neutral", texto: r.solo_lectura ? "Importado" : "Sin copias todavía" };
  return { tono: "ok", texto: "Al día" };
}
