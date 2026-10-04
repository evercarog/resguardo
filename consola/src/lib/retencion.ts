// Retención en el almacén (v1.22, docs/compartir.md «Retención en el almacén»):
// lo que no es pantalla. Un repositorio de un almacén («Este equipo guarda
// copias») es de solo añadir: el equipo no puede podar, lo hace el almacén en
// local con una clave de restic propia que el equipo dueño añade al repositorio.
import { aleatorio } from "./cripto/bytes";
import { usuarioEnAlmacen } from "./direccion";
import { destinoDe } from "./repo";
import { copiasEnFrase } from "./formato";
import { horasDelDia, reglasDe } from "./horario";
import type { CopiaResumen, DestinoResumen, Equipo, Horario, HorarioRetencion, Regla, RepositorioResumen, RetencionAlmacen } from "./tipos";

export const REGLA_POR_DEFECTO: Regla = { diarias: 7, semanales: 4, mensuales: 12, anuales: 2 };
/** Los domingos a las 03:00: fuera de las horas de copia. */
export const HORARIO_POR_DEFECTO: HorarioRetencion = { dias: [7], hora: "03:00" };

// ---------------------------------------------------------------------------
// Reglas: cantidades y, desde v1.28 (agente que manda `admite: ["retencion_plazos"]`),
// horarias, «siempre» y plazos (como `restic forget --keep-within-hourly 15d`).
// Lo mismo que `Retencion` del agente (gestion_v2.rs): validar, el texto y el plan.
// ---------------------------------------------------------------------------

/** Cantidad «todas las de ese tipo» (`unlimited` en restic). */
export const SIEMPRE = -1;
export const PERIODOS = ["horarias", "diarias", "semanales", "mensuales", "anuales"] as const;
export type Periodo = (typeof PERIODOS)[number];
/** «hora», «día»… (para «una por hora»). */
export const UNA_POR: Record<Periodo, string> = { horarias: "hora", diarias: "día", semanales: "semana", mensuales: "mes", anuales: "año" };
/** Lo que el agente dice que entiende (v1.28). */
export const ADMITE_PLAZOS = "retencion_plazos";
export const admitePlazos = (e: Equipo | null | undefined) => !!e?.resumen?.admite?.includes(ADMITE_PLAZOS);

/** Un plazo de restic: años, meses, días y horas. */
export interface Plazo {
  anos: number;
  meses: number;
  dias: number;
  horas: number;
}

/** Como el agente (`Plazo::leer`): «15d», «1y6m», «48h»; nunca cero ni más de 200 años. */
export function leerPlazo(s: string | null | undefined): Plazo | null {
  if (!s || s.length > 16 || !/^(\d+[ymdh])+$/.test(s)) return null;
  const p: Plazo = { anos: 0, meses: 0, dias: 0, horas: 0 };
  for (const [, n, u] of s.matchAll(/(\d+)([ymdh])/g)) {
    const k = ({ y: "anos", m: "meses", d: "dias", h: "horas" } as const)[u as "y" | "m" | "d" | "h"];
    p[k] += Number(n);
  }
  const horas = p.anos * 8766 + p.meses * 730 + p.dias * 24 + p.horas;
  return horas > 0 && horas <= 200 * 8766 && Object.values(p).every((x) => x <= 2 ** 31 - 1) ? p : null;
}

/** «15 días», «1 año», «1 año y 6 meses». */
export function plazoEnPalabras(s: string): string {
  const p = leerPlazo(s);
  if (!p) return s;
  const partes = (
    [
      [p.anos, "año", "años"],
      [p.meses, "mes", "meses"],
      [p.dias, "día", "días"],
      [p.horas, "hora", "horas"],
    ] as const
  )
    .filter(([n]) => n > 0)
    .map(([n, uno, varios]) => `${n} ${n === 1 ? uno : varios}`);
  return partes.length > 1 ? `${partes.slice(0, -1).join(", ")} y ${partes.at(-1)}` : (partes[0] ?? s);
}

/** Horas aproximadas de un plazo (para comparar y estimar). */
export const horasPlazo = (p: Plazo) => p.anos * 8766 + p.meses * 730 + p.dias * 24 + p.horas;

const plazoDe = (r: Regla, k: Periodo) => r.plazos?.[k] || null;
const cantidad = (r: Regla, k: Periodo) => (k === "horarias" ? (r.horarias ?? 0) : r[k]) || 0;

/** ¿Usa algo de v1.28 (horarias, «siempre» o plazos)? Un agente anterior no lo entiende. */
export const usaPlazos = (r: Regla) => PERIODOS.some((k) => plazoDe(r, k) || cantidad(r, k) === SIEMPRE || (k === "horarias" && cantidad(r, k) !== 0));

/** Una copia de la regla (también de un proxy de Svelte, que `structuredClone` no admite). */
export const copiaRegla = (r: Regla): Regla => JSON.parse(JSON.stringify(r)) as Regla;

/** La regla para la orden: sin campos vacíos (como la serializa el agente). */
export function reglaParaOrden(r: Regla): Regla {
  const out: Regla = { diarias: cantidad(r, "diarias"), semanales: cantidad(r, "semanales"), mensuales: cantidad(r, "mensuales"), anuales: cantidad(r, "anuales") };
  if (cantidad(r, "horarias")) out.horarias = cantidad(r, "horarias");
  const plazos = Object.fromEntries(PERIODOS.filter((k) => plazoDe(r, k)).map((k) => [k, plazoDe(r, k)!]));
  if (Object.keys(plazos).length) out.plazos = plazos;
  return out;
}

/** «7 diarias · 4 semanales · 12 mensuales · 2 anuales» (o con comas, o sin alguna) → números. */
export function leerRegla(texto?: string | null): Regla | null {
  if (!texto) return null;
  const n = (palabra: string) => Number(new RegExp(`(\\d+)\\s*${palabra}`, "i").exec(texto)?.[1] ?? 0);
  const r = { diarias: n("diarias"), semanales: n("semanales"), mensuales: n("mensuales"), anuales: n("anuales") };
  return r.diarias + r.semanales + r.mensuales + r.anuales > 0 ? r : null;
}

/** La regla de un repositorio: la del resumen (v1.28) o, si no, leída de su texto. */
export const reglaDe = (r: RepositorioResumen | null | undefined): Regla | null => (r?.retencion_regla ? reglaParaOrden(r.retencion_regla) : leerRegla(r?.retencion));

export const mismaRegla = (a: Regla | null | undefined, b: Regla | null | undefined) => !!a && !!b && JSON.stringify(reglaParaOrden(a)) === JSON.stringify(reglaParaOrden(b));

/** Como el agente: al menos una regla; de 0 a 1000 de cada tipo (o «siempre») y plazos de restic. Sin `admite`, solo lo de antes. */
export function errorRegla(r: Regla, admite = true): string | null {
  for (const k of PERIODOS) {
    const n = cantidad(r, k);
    if (!Number.isInteger(n) || (n !== SIEMPRE && (n < 0 || n > 1000))) return "Entre 0 y 1000 de cada tipo.";
    const p = plazoDe(r, k);
    if (p && !leerPlazo(p)) return `El plazo de las ${k} no vale (por ejemplo 15d, 6m o 1y).`;
  }
  if (!admite && usaPlazos(r)) return "Este agente solo guarda diarias, semanales, mensuales y anuales: actualízalo para usar horarias, plazos o «siempre».";
  if (!PERIODOS.some((k) => cantidad(r, k) !== 0 || plazoDe(r, k))) return "Tiene que guardar al menos una versión.";
  return null;
}

/** Como el agente (`Retencion::texto`): lo de antes igual; con v1.28, solo lo que guarda. */
export function textoRegla(r: Regla): string {
  if (!usaPlazos(r)) return `${r.diarias} diarias · ${r.semanales} semanales · ${r.mensuales} mensuales · ${r.anuales} anuales`;
  const partes: string[] = [];
  for (const k of PERIODOS) {
    const p = plazoDe(r, k);
    if (p && leerPlazo(p)) partes.push(`${k} ${plazoEnPalabras(p)}`);
    const n = cantidad(r, k);
    if (n === SIEMPRE) partes.push(`${k} siempre`);
    else if (n) partes.push(`${n} ${k}`);
  }
  return partes.join(" · ");
}

/** Lista en castellano: «a», «a y b», «a, b y c». */
const enLista = (x: string[]) => (x.length > 1 ? `${x.slice(0, -1).join(", ")} y ${x.at(-1)}` : (x[0] ?? ""));

/** «diaria» / «diarias». */
const SINGULAR: Record<Periodo, string> = { horarias: "horaria", diarias: "diaria", semanales: "semanal", mensuales: "mensual", anuales: "anual" };

/**
 * En palabras llanas (como la app de escritorio): «Conserva una por hora durante
 * 15 días, una por día durante 1 año y una por mes siempre».
 */
export function resumenRegla(r: Regla): string {
  const partes: string[] = [];
  let plazos = false;
  for (const k of PERIODOS) {
    const p = plazoDe(r, k);
    if (p && leerPlazo(p)) {
      plazos = true;
      partes.push(`una por ${UNA_POR[k]} durante ${plazoEnPalabras(p)}`);
    }
    const n = cantidad(r, k);
    if (n === SIEMPRE) partes.push(`una por ${UNA_POR[k]} siempre`);
    else if (n === 1) partes.push(`la última ${SINGULAR[k]}`);
    else if (n) partes.push(`las últimas ${n} ${k}`);
  }
  if (!partes.length) return "No guarda ninguna versión.";
  return `Conserva ${enLista(partes)}.${plazos ? " Los plazos cuentan desde la versión más reciente: si el equipo deja de copiar, no se pierde nada por esperar." : ""}`;
}

// ---------------------------------------------------------------------------
// Qué versiones guarda (como `restic forget` y `planear` del agente, en un solo
// grupo y sin la desconfianza del almacén): para estimar cuántas quedan.
// ---------------------------------------------------------------------------

const HUECOS: ((d: Date) => number)[] = [
  (d) => ((d.getFullYear() * 100 + d.getMonth() + 1) * 100 + d.getDate()) * 100 + d.getHours(),
  (d) => (d.getFullYear() * 100 + d.getMonth() + 1) * 100 + d.getDate(),
  (d) => {
    // Semana ISO (la del jueves de esa semana).
    const t = new Date(Date.UTC(d.getFullYear(), d.getMonth(), d.getDate()));
    t.setUTCDate(t.getUTCDate() + 4 - (t.getUTCDay() || 7));
    const inicio = Date.UTC(t.getUTCFullYear(), 0, 1);
    return t.getUTCFullYear() * 100 + Math.ceil(((t.getTime() - inicio) / 86_400_000 + 1) / 7);
  },
  (d) => d.getFullYear() * 100 + d.getMonth() + 1,
  (d) => d.getFullYear(),
];

/** `t.AddDate(-años, -meses, -días).Add(-horas)` de Go (en hora local). */
export function restarPlazo(t: Date, p: Plazo): Date {
  const d = new Date(t.getFullYear() - p.anos, t.getMonth() - p.meses, t.getDate() - p.dias, t.getHours(), t.getMinutes(), t.getSeconds(), t.getMilliseconds());
  return new Date(d.getTime() - p.horas * 3_600_000);
}

/** Las que se quedan (índices), de unas horas en orden de la más reciente a la más antigua. */
export function seQuedan(horas: Date[], r: Regla): boolean[] {
  const quedan = horas.map(() => false);
  if (!horas.length) return quedan;
  const limites = PERIODOS.map((k) => {
    const p = leerPlazo(plazoDe(r, k));
    return p ? restarPlazo(horas[0], p).getTime() : null;
  });
  const cuantas = PERIODOS.map((k) => cantidad(r, k));
  const anterior: (number | null)[] = PERIODOS.map(() => null);
  const enPlazo: (number | null)[] = PERIODOS.map(() => null);
  const ultima = horas.length - 1;
  horas.forEach((t, n) => {
    for (let i = 0; i < PERIODOS.length; i++) {
      if (cuantas[i] > 0 || cuantas[i] === SIEMPRE) {
        const h = HUECOS[i](t);
        if (anterior[i] !== h || n === ultima) {
          quedan[n] = true;
          anterior[i] = h;
          if (cuantas[i] > 0) cuantas[i]--;
        }
      }
      const lim = limites[i];
      if (lim !== null && t.getTime() > lim) {
        const h = HUECOS[i](t);
        if (enPlazo[i] !== h || n === ultima) {
          quedan[n] = true;
          enPlazo[i] = h;
        }
      }
    }
  });
  return quedan;
}

/**
 * Lo mismo que `seQuedan`, diciendo por qué se queda cada una (la primera
 * regla que la guarda: «diarias», «semanales»…; la más antigua de todas, por
 * la regla que la alcanzó), o null si la próxima retención la quitaría. Para
 * la línea de tiempo de las versiones.
 */
export function motivosQuedan(horas: Date[], r: Regla): (Periodo | null)[] {
  const motivo: (Periodo | null)[] = horas.map(() => null);
  if (!horas.length) return motivo;
  const limites = PERIODOS.map((k) => {
    const p = leerPlazo(plazoDe(r, k));
    return p ? restarPlazo(horas[0], p).getTime() : null;
  });
  const cuantas = PERIODOS.map((k) => cantidad(r, k));
  const anterior: (number | null)[] = PERIODOS.map(() => null);
  const enPlazo: (number | null)[] = PERIODOS.map(() => null);
  const ultima = horas.length - 1;
  horas.forEach((t, n) => {
    for (let i = 0; i < PERIODOS.length; i++) {
      if (cuantas[i] > 0 || cuantas[i] === SIEMPRE) {
        const h = HUECOS[i](t);
        if (anterior[i] !== h || n === ultima) {
          motivo[n] ??= PERIODOS[i];
          anterior[i] = h;
          if (cuantas[i] > 0) cuantas[i]--;
        }
      }
      const lim = limites[i];
      if (lim !== null && t.getTime() > lim) {
        const h = HUECOS[i](t);
        if (enPlazo[i] !== h || n === ultima) {
          motivo[n] ??= PERIODOS[i];
          enPlazo[i] = h;
        }
      }
    }
  });
  return motivo;
}

const HORAS_PERIODO: Record<Periodo, number> ={ horarias: 1, diarias: 24, semanales: 168, mensuales: 730, anuales: 8766 };

export interface Estimacion {
  /** Versiones que quedan, como mucho, al cabo de `meses`. */
  versiones: number;
  meses: number;
  /** Las que se suman cada año después (las «siempre»), o 0. */
  porAno: number;
}

/**
 * Cuántas versiones guarda, como mucho, con estas horas de copia (una por día:
 * `horasDelDia(fecha)`; sin horario conocido, una cada hora). Se simulan las
 * copias del tiempo que abarcan las reglas (de 1 a 5 años) y se cuentan las que
 * quedan; con «siempre», además, cuántas más cada año.
 */
export function estimarVersiones(r: Regla, horasDelDia?: (fecha: Date) => string[], ahora = new Date()): Estimacion {
  let alcance = 30 * 24;
  for (const k of PERIODOS) {
    const p = leerPlazo(plazoDe(r, k));
    if (p) alcance = Math.max(alcance, horasPlazo(p));
    const n = cantidad(r, k);
    if (n > 0) alcance = Math.max(alcance, n * HORAS_PERIODO[k]);
  }
  // Un mes más (lo de «la más antigua») y, con «siempre», un año más para medir lo que crece.
  const siempre = PERIODOS.filter((k) => cantidad(r, k) === SIEMPRE);
  alcance = Math.min(alcance + 730, 5 * 8766);
  const dias = Math.ceil((alcance + (siempre.length ? 8766 : 0)) / 24);
  const horas: Date[] = [];
  for (let d = 0; d < dias; d++) {
    const fecha = new Date(ahora.getFullYear(), ahora.getMonth(), ahora.getDate() - d);
    const lista = horasDelDia ? horasDelDia(fecha) : Array.from({ length: 24 }, (_, h) => `${String(h).padStart(2, "0")}:00`);
    for (const hm of [...lista].reverse()) {
      const [h, m] = hm.split(":").map(Number);
      const t = new Date(fecha.getFullYear(), fecha.getMonth(), fecha.getDate(), h, m);
      if (t <= ahora) horas.push(t);
    }
  }
  const quedan = seQuedan(horas, r);
  const corte = ahora.getTime() - alcance * 3_600_000;
  const versiones = quedan.filter((q, i) => q && horas[i].getTime() > corte).length;
  // Lo que crece cada año con «siempre»: semanales 52, mensuales 12, anuales 1;
  // horarias y diarias, las horas o días con copia de un año (52 semanas).
  const ano = horas.filter((t) => t.getTime() > ahora.getTime() - 364 * 86_400_000);
  const distintas = (i: number) => new Set(ano.map(HUECOS[i])).size;
  const RITMO: Record<Periodo, () => number> = { horarias: () => distintas(0), diarias: () => distintas(1), semanales: () => 52, mensuales: () => 12, anuales: () => 1 };
  const porAno = Math.max(0, ...siempre.map((k) => RITMO[k]()));
  return { versiones, meses: Math.round(alcance / 730), porAno };
}

/** Las horas de copia de un repositorio (sus copias activas), para estimar cuántas versiones quedan. */
export function horarioDeCopias(copias: CopiaResumen[] | undefined, repo: string): { horasDelDia?: (fecha: Date) => string[]; copias?: string } {
  const reglas = (copias ?? []).filter((k) => k.repo === repo && k.activa !== false && k.horario && typeof k.horario === "object").flatMap((k) => reglasDe(k.horario as Horario));
  if (!reglas.length) return {};
  return { horasDelDia: (f) => horasDelDia(reglas, f), copias: copiasEnFrase(reglas) };
}

export interface Preset {
  id: string;
  texto: string;
  regla: Regla;
  /** Necesita un agente con `retencion_plazos`. */
  plazos: boolean;
}

/** Las reglas de un clic (la de los programas contables: horarias 15 días, diarias 1 año, mensuales siempre). */
export const PRESETS: Preset[] = [
  { id: "contables", texto: "Programas contables: horarias 15 días, diarias 1 año, mensuales siempre", regla: { diarias: 0, semanales: 0, mensuales: SIEMPRE, anuales: 0, plazos: { horarias: "15d", diarias: "1y" } }, plazos: true },
  { id: "30d6m", texto: "Diarias 30 días, semanales 6 meses", regla: { diarias: 0, semanales: 0, mensuales: 0, anuales: 0, plazos: { diarias: "30d", semanales: "6m" } }, plazos: true },
  { id: "clasica", texto: "7 diarias, 4 semanales, 12 mensuales y 2 anuales", regla: { ...REGLA_POR_DEFECTO }, plazos: false },
];

/** El preset que es exactamente esta regla, si hay. */
export const presetDe = (r: Regla) => PRESETS.find((p) => mismaRegla(p.regla, r))?.id ?? "personalizada";

export function errorHorario(h: HorarioRetencion): string | null {
  if (!h.dias.length || h.dias.some((d) => !(d >= 1 && d <= 7))) return "Elige al menos un día.";
  if (!/^([01]\d|2[0-3]):[0-5]\d$/.test(h.hora)) return "Hora no válida (HH:MM).";
  return null;
}

const DIAS_PLURAL = ["lunes", "martes", "miércoles", "jueves", "viernes", "sábados", "domingos"];
export const DIAS_CORTOS = ["L", "M", "X", "J", "V", "S", "D"];

/** «los domingos a las 03:00», «lunes y jueves a las 22:30», «cada día a las 03:00» (como el agente). */
export function textoHorario(h: HorarioRetencion): string {
  const dias = [...new Set(h.dias)].filter((d) => d >= 1 && d <= 7).sort((a, b) => a - b);
  const nombres = dias.map((d) => DIAS_PLURAL[d - 1]);
  const cuando = dias.length === 7 ? "cada día" : dias.length === 1 ? `los ${nombres[0]}` : `${nombres.slice(0, -1).join(", ")} y ${nombres[nombres.length - 1]}`;
  return `${cuando} a las ${h.hora}`;
}

/** ¿Es `d` el destino de ese almacén? (El mismo criterio que «Nuevo repositorio».) */
export const esDeAlmacen = (d: DestinoResumen, a: Equipo) =>
  d.equipo_almacen === a.id || d.id === `almacen-${a.id.slice(0, 8)}` || (!d.equipo_almacen && d.tipo === "rest" && d.nombre === a.nombre);

/** v1.28: ¿puede tener un repositorio en su propio almacén? (`guarda_copias { anadir, local: true }`). */
export const admiteAlmacenPropio = (e: Equipo | null | undefined) => !!e?.resumen?.guarda_copias?.activo && !!e.resumen.admite?.includes("almacen_propio");

export const TEXTO_ALMACEN_PROPIO =
  "Un repositorio en el propio almacén de este equipo (por localhost, con su propio usuario de solo añadir): para lo que vive aquí, como las copias de la consola. Así entra en su espejo y en la nube. Ojo: sin espejo, sigue en el mismo disco.";

export interface EnAlmacen {
  almacen: Equipo;
  /** El usuario del equipo dueño en el almacén (la carpeta `/<usuario>/`). */
  usuario: string;
  /** La carpeta del repositorio dentro de la del usuario. */
  carpeta: string;
  /** La retención que ya aplica el almacén para él, si hay. */
  retencion: RetencionAlmacen | null;
  /** ¿El agente del almacén sabe aplicarla? (Manda `guarda_copias.retenciones`.) */
  admite: boolean;
}

/** El almacén donde está un repositorio (de este cliente), o null si no está en ninguno. */
export function almacenDe(repo: RepositorioResumen, destino: DestinoResumen | undefined, equipos: Equipo[]): EnAlmacen | null {
  if (!destino || destino.tipo !== "rest" || repo.solo_lectura) return null;
  const almacen = equipos.find((a) => a.resumen?.guarda_copias?.activo && a.modo !== "trasladado" && esDeAlmacen(destino, a));
  const usuario = usuarioEnAlmacen(destino.donde);
  if (!almacen || !usuario) return null;
  const carpeta = repo.ruta || repo.id;
  const lista = almacen.resumen?.guarda_copias?.retenciones;
  return {
    almacen,
    usuario,
    carpeta,
    retencion: lista?.find((r) => r.usuario === usuario && r.repo === carpeta) ?? null,
    admite: Array.isArray(lista),
  };
}

/** El repositorio (y su equipo) de una retención de un almacén, si es de este cliente. */
export function repoDeRetencion(r: RetencionAlmacen, almacen: Equipo, equipos: Equipo[]): { equipo: Equipo; repo: RepositorioResumen } | null {
  for (const e of equipos) {
    for (const repo of e.resumen?.repositorios ?? []) {
      const d = destinoDe(e.resumen?.destinos, repo);
      if (d && esDeAlmacen(d, almacen) && usuarioEnAlmacen(d.donde) === r.usuario && (repo.ruta || repo.id) === r.repo) return { equipo: e, repo };
    }
  }
  return null;
}

/** La clave de restic propia del almacén: 32 bytes al azar en base64url (sin relleno). */
export function nuevaClave(): string {
  const b = aleatorio(32);
  const t = btoa(String.fromCharCode(...b)).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
  b.fill(0);
  return t;
}
