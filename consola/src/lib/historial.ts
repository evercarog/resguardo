// «Historial y versiones» (docs/diseno.md §4): lo que no es pantalla. Junta
// en una sola línea de tiempo lo que antes iba en dos listas (la bitácora de
// las versiones y la «Historia» de las vueltas): las versiones guardadas van
// aparte (las pinta la bitácora, con su copia y su retención) y aquí se sacan
// los demás sucesos, ya sin repetir: copias que fallaron o no guardaron nada
// («sin cambios»), comprobaciones, pruebas de restauración, subidas a la nube,
// el espejo, los pasos «Antes de copiar» que fueron mal y lo que el equipo
// resume por día de hace más de un año. Sirve igual para un repositorio, para
// una copia y para un equipo entero (varios repositorios a la vez).
import type { CopiaResumen, EjecucionInforme, EntradaHistorial, Informe, RepoInforme, RepositorioResumen, ResultadoGancho, TareaInforme, VersionInforme } from "./tipos";
import { agruparResultados, NOMBRE_GANCHO, resumenResultados } from "./ganchos";
import { duracion, pruebaRestauracion, TEXTO_RESULTADO, TEXTO_TAREA, TONO_RESULTADO, TONO_TAREA, verificacion, versionDeVuelta, versionesDe } from "./repo";
import { bytes, numero } from "./formato";
import type { Tono } from "./salud";
import type { EntradaRetencion } from "./retencionDetalle";
import { textoRegla } from "./retencion";
import { RESULTADO_ORDEN, resultadoOrden } from "./espera";
import { mensajeOrden } from "./textosEquipo";

/** Qué clase de suceso es (cada una con su icono en la pantalla). */
export type TipoSuceso = "copia" | "sin_cambios" | "fallo" | "gancho" | "resumen" | "verificacion" | "prueba" | "externa" | "espejo" | "aviso" | "historial" | "retencion" | "restauracion";

/**
 * 14o: una fila del detalle de un suceso («Cuánto tardó: 3 min»). Sin valor
 * (`null`), el equipo no lo dijo: se enseña «sin detalle» y nunca se inventa.
 */
export interface FilaFicha {
  k: string;
  v: string | null;
}
/** Lo que se dice de un dato que no llegó: de un agente anterior o, si es nuevo, que no consta. */
export const SIN_DETALLE = "sin detalle (agente anterior)";
export const NO_CONSTA = "no consta";

/**
 * Qué fue, para los filtros y la marca del calendario: un fallo de verdad (una
 * copia, comprobación, prueba o subida que falló), un aviso (algo que mirar
 * que no es un fallo: una copia con avisos, un paso previo con avisos, que el
 * equipo se conectó a otra consola, un cambio inusual…) o algo que fue bien.
 */
export type ClaseSuceso = "fallo" | "aviso" | "ok";

/** Los filtros de arriba: «Todo · Versiones · Fallos · Avisos · Comprobaciones · Subidas». */
export type FiltroHistorial = "todo" | "versiones" | "fallos" | "avisos" | "comprobaciones" | "subidas";
export const FILTROS_HISTORIAL: { id: FiltroHistorial; texto: string }[] = [
  { id: "todo", texto: "Todo" },
  { id: "versiones", texto: "Versiones" },
  { id: "fallos", texto: "Fallos" },
  { id: "avisos", texto: "Avisos" },
  { id: "comprobaciones", texto: "Comprobaciones" },
  { id: "subidas", texto: "Subidas" },
];

/** Un suceso que no es una versión guardada (las versiones las lleva la bitácora). */
export interface Suceso {
  clave: string;
  hora: string;
  t: number;
  tipo: TipoSuceso;
  tono: Tono;
  /** Fallo, aviso o bien (`claseDe`): «Fallos» y la marca del calendario son solo los fallos. */
  clase: ClaseSuceso;
  /** «Copia «Documentos»», «Verificación», «Copia derivada»… */
  titulo: string;
  /** El estado en una palabra, para el chip (siempre con su icono). */
  chip: string;
  /** El motivo (lo que dijo el equipo), si fue mal. */
  detalle: string | null;
  /** «3 min · +12 MB», «antes de copiar»… */
  meta: string | null;
  repo: string | null;
  copia: string | null;
  /** La hora de la vuelta (abre su detalle en el cajón), si la tiene el informe. */
  vuelta: string | null;
  /** 14o: su detalle (se abre con un clic en la bitácora), con lo que dijo el equipo. */
  ficha?: FilaFicha[];
  /** Si el equipo es anterior (no manda estos datos): lo que falta se dice «sin detalle (agente anterior)». */
  anterior?: boolean;
  /** Dónde verlo entero (la retención, en «Retención en detalle»). */
  enlace?: { href: string; texto: string } | null;
}

/** Lo que se dice de una versión aparte de ella misma: si su vuelta tuvo avisos y sus pasos previos. */
export interface NotaVersion {
  tono: Tono | null;
  texto: string | null;
  ganchos: ResultadoGancho[];
}

export interface FuenteHistorial {
  repo: RepositorioResumen;
  inf: RepoInforme | null;
}

export interface EntradaSucesos {
  fuentes: FuenteHistorial[];
  copias: CopiaResumen[];
  /** El historial que guarda el propio equipo (v1.23), de cualquier repositorio. */
  historial?: EntradaHistorial[];
  /** Las copias del último informe, con el resultado de sus pasos previos (v1.10). */
  ultimas?: NonNullable<Informe["datos"]["copias"]>;
  /** Solo lo de esta copia (más lo del repositorio: comprobaciones y subidas). */
  soloCopia?: string | null;
  /** Varios repositorios a la vez (un equipo): los títulos dicen de cuál. */
  conRepo?: boolean;
  /** También lo del equipo que no es de un repositorio (el espejo de un almacén, sus avisos). */
  conEquipo?: boolean;
  /**
   * 14o: lo que el historial solo da si se pide (`tipo`): las vueltas de la retención
   * del equipo y lo que pasó con las órdenes (de ellas, las restauraciones).
   */
  retenciones?: EntradaRetencion[];
  ordenes?: EntradaHistorial[];
  /** La página «Retención en detalle» de un repositorio. */
  hrefRetencion?: (repo: string) => string;
}

const numeroValido = (x: unknown): number | null => (typeof x === "number" && Number.isFinite(x) && x >= 0 ? x : null);
const versionCorta = (x: unknown): string | null => (typeof x === "string" && /^[0-9a-f]{8,64}$/i.test(x) ? x.slice(0, 8).toLowerCase() : null);

/**
 * 14o: el detalle de una verificación, una prueba de restauración, una subida
 * o un espejo del historial del equipo. Los agentes nuevos mandan lo que tardó,
 * lo restaurado y la versión (v1.6x); con uno anterior solo hay el resultado y
 * su mensaje, y lo demás se dice «sin detalle».
 */
export function fichaTarea(tipo: TipoSuceso, h: Pick<EntradaHistorial, "duracion_s" | "archivos" | "bytes" | "version" | "versiones">): { ficha: FilaFicha[]; anterior: boolean } {
  const d = numeroValido(h.duracion_s);
  // El espejo del almacén no anota lo que tarda (ni los agentes nuevos): ahí no es «anterior».
  const anterior = d == null && tipo !== "espejo";
  const ficha: FilaFicha[] = [];
  if (tipo === "prueba") {
    const a = numeroValido(h.archivos);
    const b = numeroValido(h.bytes);
    ficha.push({ k: "Versión probada", v: versionCorta(h.version) });
    ficha.push({ k: "Archivos restaurados", v: a == null ? null : `${numero(a)}${b != null ? ` (${bytes(b)})` : ""}` });
    ficha.push({ k: "Dónde", v: "En una carpeta temporal del equipo, que se borra al terminar." });
    ficha.push({ k: "Cómo se comprueba", v: "El contenido de cada archivo restaurado, con sus huellas, y su tamaño." });
  } else if (tipo === "verificacion") {
    ficha.push({ k: "Cómo se comprueba", v: "La estructura del repositorio y una parte de sus datos, leídos y comprobados con sus huellas (cada vez, otra parte)." });
  } else if (tipo === "externa") {
    const n = numeroValido(h.versiones);
    ficha.push({ k: "Versiones subidas", v: n == null ? null : numero(n) });
  }
  ficha.unshift({ k: "Cuánto tardó", v: d == null ? null : duracion(d) });
  return { ficha, anterior };
}

const QUIEN_RETENCION: Record<string, string> = { equipo: "El propio equipo", almacen: "El almacén", externa: "En la copia derivada" };
const POR_RETENCION: Record<string, string> = { orden: "a mano, desde una consola", ventana: "a mano, desde la ventana del equipo", automatica: "con su horario" };

/** 14o: una vuelta de la retención (v1.45) como suceso, con su detalle. */
export function sucesoRetencion(r: EntradaRetencion, repo: string, deRepo = "", href?: string): Suceso | null {
  const t = ms(r.hora);
  if (!Number.isFinite(t)) return null;
  const fallo = r.resultado === "fallo";
  const quitadas = numeroValido(r.quitadas);
  const inicio = r.inicio ? ms(r.inicio) : NaN;
  const tardo = Number.isFinite(inicio) && t >= inicio ? Math.round((t - inicio) / 1000) : null;
  const antes = numeroValido(r.antes);
  const quedan = numeroValido(r.quedan);
  const liberado = numeroValido(r.liberado);
  const ficha: FilaFicha[] = [
    { k: "Quién la aplicó", v: `${QUIEN_RETENCION[r.origen ?? "equipo"] ?? "El equipo"}${r.por && POR_RETENCION[r.por] ? `, ${POR_RETENCION[r.por]}` : ""}` },
    { k: "Cuánto tardó", v: tardo == null ? null : duracion(tardo) },
    { k: "Versiones", v: antes != null && quedan != null ? `${numero(antes)} antes · ${numero(quedan)} después` : null },
    { k: "Quitadas", v: quitadas == null ? null : numero(quitadas) },
    { k: "Espacio liberado", v: liberado == null ? null : bytes(liberado) },
  ];
  if (r.regla) ficha.push({ k: "Regla", v: textoRegla(r.regla) });
  return {
    clave: `r|${r.id}`,
    hora: r.hora,
    t,
    tipo: "retencion",
    tono: fallo ? "bad" : "ok",
    clase: fallo ? "fallo" : "ok",
    titulo: `Retención${deRepo}`,
    chip: fallo ? "Falló" : "Correcta",
    detalle: r.mensaje ?? null,
    meta: quitadas == null ? null : quitadas ? `${numero(quitadas)} ${quitadas === 1 ? "quitada" : "quitadas"}` : "nada que quitar",
    repo,
    copia: null,
    vuelta: null,
    ficha,
    anterior: r.compactada ? false : !r.inicio,
    enlace: href ? { href, texto: "Ver en «Retención en detalle»" } : null,
  };
}

/** 14o: una restauración (una orden «restaurar» del historial común, v1.49) como suceso. */
export function sucesoRestauracion(h: EntradaHistorial, repo: string | null): Suceso | null {
  const t = ms(h.hora);
  if (!Number.isFinite(t)) return null;
  const r = resultadoOrden(h);
  const estado = RESULTADO_ORDEN[r] ?? { texto: r || "Sin resultado", tono: "neutral" as const };
  const tono: Tono = estado.tono;
  const mensaje = h.mensaje ? mensajeOrden("restaurar", h.mensaje) : null;
  return {
    clave: `o|${h.id}`,
    hora: h.hora,
    t,
    tipo: "restauracion",
    tono,
    clase: claseDe({ tipo: "restauracion", tono }),
    titulo: "Restauración",
    chip: estado.texto,
    detalle: tono === "bad" ? mensaje : null,
    meta: h.por ? `por ${h.por}` : null,
    repo,
    copia: null,
    vuelta: null,
    ficha: [
      { k: "Qué", v: h.descripcion ?? null },
      { k: "Quién", v: h.por ?? null },
      { k: "Desde", v: h.consola ? `la consola «${h.consola}»` : null },
      { k: "Resultado", v: mensaje ?? estado.texto },
      // Lo que tardó una restauración aún no lo anota el equipo (la ficha de cada restauración, 14l).
      { k: "Cuánto tardó", v: null },
    ],
    anterior: false,
  };
}

const ms = (h: string) => Date.parse(h);
const minutos = (s: number | null | undefined) => (s == null ? null : duracion(s));
const tonoGancho = (g: ResultadoGancho): Tono => (g.estado === "ok" ? "ok" : g.estado === "aviso" ? "warn" : "bad");
const textoGancho = (g: ResultadoGancho) => (g.estado === "ok" ? "Correcto" : g.estado === "aviso" ? "Con avisos" : "Falló");
/** La misma vuelta (misma copia, menos de un minuto de diferencia). */
const misma = (a: number, b: number) => Math.abs(a - b) <= 60_000;

type Cabeza = { n: number; hash: string };
/** v1.50 (9b): qué no cuadró en la actividad de una consola (`auditoria_rehecha`). */
export const textoRehecha = (antes: Cabeza, ahora: Cabeza) =>
  ahora.n < antes.n
    ? `Antes llegaba a la entrada n.º ${numero(antes.n)} y ahora solo a la ${numero(ahora.n)}. Si nadie restauró una copia de esa consola, compruébala con un ancla de un correo anterior.`
    : `La entrada n.º ${numero(antes.n)} tenía la huella ${antes.hash.slice(0, 12)}… y ahora tiene ${ahora.hash.slice(0, 12)}…. Compruébala con un ancla de un correo anterior.`;

/**
 * Los sucesos (de lo más reciente a lo más antiguo) y lo que se dice de cada
 * versión (`notas`, por id). Lo que traen a la vez el informe y el historial
 * del equipo sale una sola vez; una vuelta correcta que dejó versión no es un
 * suceso aparte (es su versión), pero sus avisos y sus pasos previos se dicen
 * en ella.
 */
export function sucesosDe(e: EntradaSucesos): { sucesos: Suceso[]; notas: Map<string, NotaVersion> } {
  const out: (Omit<Suceso, "clase"> & { fallidas?: number })[] = [];
  const notas = new Map<string, NotaVersion>();
  const deCopia = (id: string | null | undefined) => e.copias.find((k) => k.id === id)?.nombre;
  const pasa = (copia: string | null | undefined) => !e.soloCopia || copia === e.soloCopia;
  const historial = e.historial ?? [];
  /** Las entradas `historial` ya contadas (salen en el repositorio de origen y en el nuevo). */
  const yaHistorial = new Set<string>();
  // 14o: las restauraciones (órdenes «restaurar»), la última noticia de cada una.
  const porOrden = new Map<string, EntradaHistorial>();
  for (const h of e.ordenes ?? []) {
    if (h.tipo !== "orden" || h.orden !== "restaurar") continue;
    const k = h.orden_id || h.id;
    const ya = porOrden.get(k);
    if (!ya || ms(h.hora) > ms(ya.hora)) porOrden.set(k, h);
  }
  const restauraciones = [...porOrden.values()];
  const restauracionesVistas = new Set<string>();

  for (const { repo, inf } of e.fuentes) {
    const deRepo = e.conRepo ? ` · «${repo.nombre}»` : "";
    const nombreCopia = (id: string | null | undefined) => {
      const n = deCopia(id);
      return n ? `Copia «${n}»` : `Copia${deRepo ? ` de «${repo.nombre}»` : ""}`;
    };
    const versiones = versionesDe(inf);
    // Los pasos «Antes de copiar» de cada vuelta (del historial y del último informe), por copia y hora.
    const ganchos: { copia: string; t: number; hora: string; gs: ResultadoGancho[]; usado: boolean }[] = [];
    for (const h of historial)
      if (h.tipo === "copia" && h.repo === repo.id && h.ganchos?.length && pasa(h.copia)) ganchos.push({ copia: h.copia ?? "", t: ms(h.hora), hora: h.hora, gs: h.ganchos, usado: false });
    for (const k of e.ultimas ?? []) {
      if (!k.cuando || !k.ganchos?.length || !pasa(k.id) || !e.copias.some((x) => x.id === k.id && x.repo === repo.id)) continue;
      const t = ms(k.cuando);
      if (!ganchos.some((g) => g.copia === k.id && misma(g.t, t))) ganchos.push({ copia: k.id, t, hora: k.cuando, gs: k.ganchos, usado: false });
    }
    const ganchosDe = (copia: string | null | undefined, t: number) => {
      const g = ganchos.find((x) => !x.usado && x.copia === (copia ?? "") && misma(x.t, t));
      if (!g) return [];
      g.usado = true;
      return g.gs;
    };
    const malos = (gs: ResultadoGancho[]) => gs.filter((g) => g.estado !== "ok");
    // 15j: sin repetir «volcado de SQL Server falló» por cada base.
    const textoGanchos = (gs: ResultadoGancho[]) => (gs.length ? `antes de copiar: ${resumenResultados(gs)}` : null);

    // Lo ya contado (para no repetir lo que traen el informe y el historial).
    const vistas = new Set<string>();
    const vuelta = (x: EjecucionInforme | EntradaHistorial, desdeInforme: boolean) => {
      const resultado = x.resultado!;
      const copia = x.copia ?? null;
      const t = ms(x.hora);
      vistas.add(`c|${copia ?? ""}|${Math.round(t / 60_000)}`);
      const gs = ganchosDe(copia, t);
      const v = desdeInforme ? versionDeVuelta(versiones, x as EjecucionInforme) : null;
      const mensaje = "mensaje_corto" in x ? x.mensaje_corto : ((x as EntradaHistorial).mensaje ?? null);
      if (v) {
        // Dejó versión: se dice en ella (sus avisos y sus pasos previos).
        notas.set(v.id, { tono: resultado === "aviso" ? "warn" : null, texto: resultado === "aviso" ? (mensaje ?? null) : null, ganchos: gs });
        return;
      }
      const anadido = (x as { anadido?: number | null }).anadido ?? null;
      const tipo: TipoSuceso = resultado === "fallo" ? "fallo" : resultado === "sin_cambios" ? "sin_cambios" : "copia";
      out.push({
        clave: `${desdeInforme ? "e" : "h"}|${repo.id}|${copia ?? ""}|${x.hora}`,
        hora: x.hora,
        t,
        tipo,
        tono: TONO_RESULTADO[resultado],
        titulo: nombreCopia(copia),
        chip: TEXTO_RESULTADO[resultado],
        detalle: resultado === "ok" || resultado === "sin_cambios" ? (malos(gs).map((g) => g.mensaje).find(Boolean) ?? null) : (mensaje ?? null),
        meta:
          [
            minutos(x.duracion_s),
            anadido != null && resultado !== "sin_cambios" && resultado !== "fallo" ? `+${bytes(anadido)}` : null,
            x.reintento ? "reintento" : null,
            textoGanchos(gs),
          ]
            .filter(Boolean)
            .join(" · ") || null,
        repo: repo.id,
        copia,
        vuelta: desdeInforme ? x.hora : null,
        // 14o: sin su vuelta en el informe (más antigua), su detalle con lo que guarda el historial.
        ficha: desdeInforme
          ? undefined
          : [
              { k: "Cuánto tardó", v: minutos(x.duracion_s) },
              ...(resultado === "fallo"
                ? []
                : [
                    { k: "Lo nuevo", v: anadido != null ? bytes(anadido) : null },
                    {
                      k: "Archivos",
                      v: x.archivos_nuevos != null || x.archivos_cambiados != null ? `${numero(x.archivos_nuevos ?? 0)} nuevos · ${numero(x.archivos_cambiados ?? 0)} cambiados` : null,
                    },
                  ]),
              ...agruparResultados(gs).map((g) => ({ k: NOMBRE_GANCHO[g.tipo] ?? g.tipo, v: `${textoGancho(g)}${g.mensaje ? `: ${g.mensaje}` : ""}` })),
            ],
      });
    };

    for (const x of inf?.ejecuciones ?? []) if (pasa(x.copia)) vuelta(x, true);

    const TAREA: Partial<Record<EntradaHistorial["tipo"], [TipoSuceso, string]>> = {
      verificacion: ["verificacion", "Verificación"],
      prueba_restauracion: ["prueba", "Prueba de restauración"],
      externa: ["externa", "Copia derivada"],
      espejo: ["espejo", "Espejo"],
    };
    for (const h of historial) {
      // v1.47: se trajo el historial (o un paso de «Mover a otro sitio…»): en los dos repositorios, una vez.
      if (h.tipo === "historial" && h.resultado && (h.repo === repo.id || h.origen === repo.id)) {
        if (!yaHistorial.has(h.id)) {
          yaHistorial.add(h.id);
          out.push(sucesoHistorial(h, repo.id, deRepo));
        }
        continue;
      }
      if (h.repo !== repo.id) continue;
      if (h.tipo === "copia" && h.resultado) {
        if (!pasa(h.copia) || vistas.has(`c|${h.copia ?? ""}|${Math.round(ms(h.hora) / 60_000)}`)) continue;
        vuelta(h, false);
      } else if (h.tipo === "resumen_dia") {
        if (!pasa(h.copia)) continue;
        // Más de un año atrás: el equipo lo guarda resumido, una entrada por copia y día.
        const [ok, mal, igual] = [h.ok ?? 0, h.fallidas ?? 0, h.sin_cambios ?? 0];
        const partes = [ok ? `${numero(ok)} ${ok === 1 ? "correcta" : "correctas"}` : null, mal ? `${numero(mal)} ${mal === 1 ? "fallida" : "fallidas"}` : null, igual ? `${numero(igual)} sin cambios` : null];
        out.push({
          clave: `h|${h.id}`,
          hora: h.hora,
          t: ms(h.hora),
          tipo: "resumen",
          tono: mal && !ok && !igual ? "bad" : mal ? "warn" : "ok",
          fallidas: mal,
          titulo: `${nombreCopia(h.copia)}: el día entero`,
          chip: mal ? "Con fallos" : "Correctas",
          detalle: mal && h.ultimo_error ? `Último error: ${h.ultimo_error}` : null,
          meta: [partes.filter(Boolean).join(" · "), h.duracion_s ? `${duracion(h.duracion_s)} en total` : null, h.anadido ? `+${bytes(h.anadido)}` : null].filter(Boolean).join(" · ") || null,
          repo: repo.id,
          copia: h.copia ?? null,
          vuelta: null,
        });
      } else if (TAREA[h.tipo] && h.resultado && h.resultado !== "sin_cambios") {
        const [tipo, titulo] = TAREA[h.tipo]!;
        vistas.add(`t|${tipo}|${Math.round(ms(h.hora) / 60_000)}`);
        const d = numeroValido(h.duracion_s);
        out.push({
          clave: `h|${h.id}`,
          hora: h.hora,
          t: ms(h.hora),
          tipo,
          tono: TONO_TAREA[h.resultado],
          titulo: `${titulo}${deRepo}`,
          chip: TEXTO_TAREA[h.resultado],
          detalle: h.mensaje ?? null,
          meta: d != null ? duracion(d) : null,
          repo: repo.id,
          copia: null,
          vuelta: null,
          ...fichaTarea(tipo, h),
        });
      }
    }
    // 14o: las vueltas de la retención que hizo el propio equipo (las del almacén, en «Retención en detalle»).
    for (const r of e.retenciones ?? []) {
      if (r.tipo !== "retencion" || r.repo !== repo.id || (r.origen && r.origen !== "equipo")) continue;
      const s = sucesoRetencion(r, repo.id, deRepo, e.hrefRetencion?.(repo.id));
      if (s) out.push(s);
    }
    // 14o: las restauraciones de este repositorio (la orden dice su nombre: «Restaurar «…» …»).
    for (const h of restauraciones)
      if (!restauracionesVistas.has(h.id) && h.descripcion?.includes(`«${repo.nombre}»`)) {
        restauracionesVistas.add(h.id);
        const s = sucesoRestauracion(h, repo.id);
        if (s) out.push({ ...s, titulo: `${s.titulo}${deRepo}` });
      }
    // La última de cada tarea, del informe, si el historial no la trae (servidores o agentes anteriores).
    const tareas: [TipoSuceso, string, TareaInforme | null][] = [
      ["verificacion", "Verificación", verificacion(repo, inf)],
      ["prueba", "Prueba de restauración", pruebaRestauracion(repo, inf)],
      ["externa", "Copia derivada", inf?.externa ?? null],
    ];
    for (const [tipo, titulo, x] of tareas) {
      if (!x?.ultima || vistas.has(`t|${tipo}|${Math.round(ms(x.ultima) / 60_000)}`)) continue;
      out.push({
        clave: `i|${repo.id}|${tipo}`,
        hora: x.ultima,
        t: ms(x.ultima),
        tipo,
        tono: TONO_TAREA[x.resultado],
        titulo: `${titulo}${deRepo}`,
        chip: TEXTO_TAREA[x.resultado],
        detalle: x.mensaje_corto ?? null,
        meta: null,
        repo: repo.id,
        copia: null,
        vuelta: null,
        // Solo del informe (el historial no la trae): lo demás no se sabe.
        ...fichaTarea(tipo, {}),
      });
    }
    // Pasos previos que no casan con ninguna vuelta (la vuelta ya no está en el informe).
    for (const g of ganchos)
      if (!g.usado)
        for (const [i, x] of agruparResultados(g.gs).entries())
          out.push({
            clave: `g|${repo.id}|${g.copia}|${g.hora}|${i}`,
            hora: g.hora,
            t: g.t,
            tipo: "gancho",
            tono: tonoGancho(x),
            titulo: `${NOMBRE_GANCHO[x.tipo] ?? x.tipo} · «${deCopia(g.copia) ?? g.copia}»`,
            chip: textoGancho(x),
            detalle: x.mensaje,
            meta: "antes de copiar",
            repo: repo.id,
            copia: g.copia,
            vuelta: null,
          });
  }
  // En un equipo entero, también las restauraciones que no dicen de qué repositorio son.
  if (e.conEquipo)
    for (const h of restauraciones)
      if (!restauracionesVistas.has(h.id)) {
        const s = sucesoRestauracion(h, null);
        if (s) out.push(s);
      }
  if (e.conEquipo)
    for (const h of historial) {
      if (h.repo) continue;
      if (h.tipo === "espejo" && h.resultado && h.resultado !== "sin_cambios")
        out.push({
          clave: `h|${h.id}`,
          hora: h.hora,
          t: ms(h.hora),
          tipo: "espejo",
          tono: TONO_TAREA[h.resultado],
          titulo: "Espejo de lo que guarda",
          chip: TEXTO_TAREA[h.resultado],
          detalle: h.mensaje ?? null,
          meta: null,
          repo: null,
          copia: null,
          vuelta: null,
          ...fichaTarea("espejo", h),
        });
      // v1.50 (9b): el equipo vio que una de sus consolas rehízo su actividad.
      else if (h.tipo === "auditoria_rehecha" && h.antes && h.ahora)
        out.push({ clave: `h|${h.id}`, hora: h.hora, t: ms(h.hora), tipo: "aviso", tono: "bad", titulo: `${h.consola ? `La consola «${h.consola}»` : "Una de sus consolas"} rehízo su actividad`, chip: "Actividad rehecha", detalle: textoRehecha(h.antes, h.ahora), meta: null, repo: null, copia: null, vuelta: null });
      else if (h.tipo === "aviso" && h.mensaje)
        out.push({ clave: `h|${h.id}`, hora: h.hora, t: ms(h.hora), tipo: "aviso", tono: "warn", titulo: "Aviso del equipo", chip: "Aviso", detalle: h.mensaje, meta: null, repo: null, copia: null, vuelta: null });
    }
  const sucesos = out
    .filter((x) => Number.isFinite(x.t))
    .sort((a, b) => b.t - a.t)
    .map(({ fallidas, ...x }): Suceso => ({ ...x, clase: claseDe({ ...x, fallidas }) }));
  return { sucesos, notas };
}

/**
 * La clase de un suceso: un aviso del equipo nunca es un fallo (es
 * informativo o algo que mirar); un día resumido con alguna copia fallida sí
 * lo es; lo demás, por su tono (en rojo, fallo; en ámbar, aviso).
 */
export function claseDe(s: Pick<Suceso, "tipo" | "tono"> & { fallidas?: number }): ClaseSuceso {
  if (s.tipo === "aviso") return "aviso";
  if (s.tipo === "resumen" && s.fallidas) return "fallo";
  return s.tono === "bad" ? "fallo" : s.tono === "warn" ? "aviso" : "ok";
}

/**
 * v1.47: una entrada `historial` (se trajo el historial de otro repositorio) vista
 * desde el repositorio `desde` (el que lo recibió o, si se movió, el de origen).
 * Un movimiento termina con su paso `ultimo`: «Movido a otro sitio».
 */
export function sucesoHistorial(h: EntradaHistorial, desde: string, deRepo = ""): Suceso {
  const resultado = h.resultado === "sin_cambios" ? "ok" : (h.resultado ?? "ok");
  const ok = resultado !== "fallo";
  const esOrigen = !!h.origen && h.origen === desde && h.repo !== desde;
  let titulo: string;
  if (h.mover) {
    const fin = h.paso === "ultimo";
    const a = h.nombre ? ` a «${h.nombre}»` : "";
    const de = h.nombre_origen ? ` desde «${h.nombre_origen}»` : "";
    titulo = !ok ? "Mover a otro sitio: falló" : fin ? (esOrigen ? `Movido a otro sitio${a}` : `Movido aquí${de}`) : `Mover a otro sitio: historial traído${esOrigen ? a : de}`;
  } else {
    titulo = esOrigen ? `Historial copiado${h.nombre ? ` a «${h.nombre}»` : ""}` : `Historial traído${h.nombre_origen ? ` de «${h.nombre_origen}»` : ""}`;
  }
  return {
    clave: `h|${h.id}`,
    hora: h.hora,
    t: ms(h.hora),
    tipo: "historial",
    tono: TONO_TAREA[resultado],
    clase: claseDe({ tipo: "historial", tono: TONO_TAREA[resultado] }),
    titulo: `${titulo}${deRepo}`,
    chip: TEXTO_TAREA[resultado],
    detalle: h.mensaje ?? null,
    meta: h.consola ? `desde la consola «${h.consola}»` : null,
    repo: desde,
    copia: null,
    vuelta: null,
  };
}

/** ¿Entra en el filtro de arriba? (las versiones, aparte: solo en «Todo» y «Versiones»). */
export function pasaFiltro(s: Pick<Suceso, "tipo" | "clase">, f: FiltroHistorial): boolean {
  if (f === "todo") return true;
  if (f === "versiones") return false;
  if (f === "fallos") return s.clase === "fallo";
  if (f === "avisos") return s.clase === "aviso";
  if (f === "comprobaciones") return s.tipo === "verificacion" || s.tipo === "prueba";
  return s.tipo === "externa" || s.tipo === "espejo";
}

/** Lo que cuenta como «hubo fallos» en el calendario (una marca en la casilla): solo los fallos, nunca un aviso. */
export const esFallo = (s: Pick<Suceso, "clase">) => s.clase === "fallo";

/** Las versiones con su repositorio (un equipo: ids de varios repositorios a la vez). */
export function versionesDeFuentes(fuentes: FuenteHistorial[], soloCopia?: string | null) {
  return fuentes.flatMap(({ repo, inf }) =>
    versionesDe(inf)
      .filter((v) => !soloCopia || v.copia === soloCopia)
      .map((v) => ({ v, repo: repo.id })),
  );
}

/** 14d: una versión tal como la da el equipo en la sesión `explorar` (`versiones`, api-servidor.md §7). */
export interface VersionSesion {
  id: string;
  cuando: string;
  archivos?: number | null;
  bytes?: number | null;
  etiquetas?: string[] | null;
}

/**
 * 14d: las versiones del informe (los últimos 60 días, con todos sus datos) y
 * las que se pidieron al equipo (todas, con menos datos), sin repetir. Las del
 * informe mandan; de las demás no se sabe la copia que las hizo, salvo que el
 * repositorio tenga una sola (`copiaUnica`). Lo que no se sabe va a `null`:
 * nunca se inventa. La más reciente primero.
 */
export function unirVersiones(informe: VersionInforme[], traidas: VersionSesion[] | null | undefined, copiaUnica: string | null = null): VersionInforme[] {
  if (!traidas?.length) return informe;
  const ya = new Set(informe.map((v) => v.id.slice(0, 8)));
  const mas: VersionInforme[] = [];
  for (const t of traidas) {
    const id = String(t.id ?? "").slice(0, 8);
    if (!/^[0-9a-f]{8}$/.test(id) || ya.has(id) || Number.isNaN(Date.parse(t.cuando))) continue;
    ya.add(id);
    mas.push({
      id,
      hora: t.cuando,
      copia: copiaUnica,
      total_bytes: typeof t.bytes === "number" ? t.bytes : null,
      anadido: null,
      anadido_empaquetado: null,
      archivos_nuevos: null,
      archivos_cambiados: null,
      archivos_sin_cambios: null,
      duracion_s: null,
      etiquetas: Array.isArray(t.etiquetas) ? t.etiquetas.filter((x) => typeof x === "string") : [],
    });
  }
  return [...informe, ...mas].sort((a, b) => Date.parse(b.hora) - Date.parse(a.hora));
}
