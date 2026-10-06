// «Historial y versiones» (docs/diseno.md §4): lo que no es pantalla. Junta
// en una sola línea de tiempo lo que antes iba en dos listas (la bitácora de
// las versiones y la «Historia» de las vueltas): las versiones guardadas van
// aparte (las pinta la bitácora, con su copia y su retención) y aquí se sacan
// los demás sucesos, ya sin repetir: copias que fallaron o no guardaron nada
// («sin cambios»), comprobaciones, pruebas de restauración, subidas a la nube,
// el espejo, los pasos «Antes de copiar» que fueron mal y lo que el equipo
// resume por día de hace más de un año. Sirve igual para un repositorio, para
// una copia y para un equipo entero (varios repositorios a la vez).
import type { CopiaResumen, EjecucionInforme, EntradaHistorial, Informe, RepoInforme, RepositorioResumen, ResultadoGancho, TareaInforme } from "./tipos";
import { NOMBRE_GANCHO } from "./ganchos";
import { duracion, pruebaRestauracion, TEXTO_RESULTADO, TEXTO_TAREA, TONO_RESULTADO, TONO_TAREA, verificacion, versionDeVuelta, versionesDe } from "./repo";
import { bytes, numero } from "./formato";
import type { Tono } from "./salud";

/** Qué clase de suceso es (cada una con su icono en la pantalla). */
export type TipoSuceso = "copia" | "sin_cambios" | "fallo" | "gancho" | "resumen" | "verificacion" | "prueba" | "externa" | "espejo" | "aviso" | "historial";

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
  /** «Copia «Documentos»», «Verificación», «Subida a la nube»… */
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
    const textoGanchos = (gs: ResultadoGancho[]) => (gs.length ? `antes de copiar: ${gs.map((g) => `${(NOMBRE_GANCHO[g.tipo] ?? g.tipo).replace(/^./, (x) => x.toLowerCase())} ${textoGancho(g).toLowerCase()}`).join(", ")}` : null);

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
      });
    };

    for (const x of inf?.ejecuciones ?? []) if (pasa(x.copia)) vuelta(x, true);

    const TAREA: Partial<Record<EntradaHistorial["tipo"], [TipoSuceso, string]>> = {
      verificacion: ["verificacion", "Verificación"],
      prueba_restauracion: ["prueba", "Prueba de restauración"],
      externa: ["externa", "Subida a la nube"],
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
        out.push({ clave: `h|${h.id}`, hora: h.hora, t: ms(h.hora), tipo, tono: TONO_TAREA[h.resultado], titulo: `${titulo}${deRepo}`, chip: TEXTO_TAREA[h.resultado], detalle: h.mensaje ?? null, meta: null, repo: repo.id, copia: null, vuelta: null });
      }
    }
    // La última de cada tarea, del informe, si el historial no la trae (servidores o agentes anteriores).
    const tareas: [TipoSuceso, string, TareaInforme | null][] = [
      ["verificacion", "Verificación", verificacion(repo, inf)],
      ["prueba", "Prueba de restauración", pruebaRestauracion(repo, inf)],
      ["externa", "Subida a la nube", inf?.externa ?? null],
    ];
    for (const [tipo, titulo, x] of tareas) {
      if (!x?.ultima || vistas.has(`t|${tipo}|${Math.round(ms(x.ultima) / 60_000)}`)) continue;
      out.push({ clave: `i|${repo.id}|${tipo}`, hora: x.ultima, t: ms(x.ultima), tipo, tono: TONO_TAREA[x.resultado], titulo: `${titulo}${deRepo}`, chip: TEXTO_TAREA[x.resultado], detalle: x.mensaje_corto ?? null, meta: null, repo: repo.id, copia: null, vuelta: null });
    }
    // Pasos previos que no casan con ninguna vuelta (la vuelta ya no está en el informe).
    for (const g of ganchos)
      if (!g.usado)
        for (const [i, x] of g.gs.entries())
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
  if (e.conEquipo)
    for (const h of historial) {
      if (h.repo) continue;
      if (h.tipo === "espejo" && h.resultado && h.resultado !== "sin_cambios")
        out.push({ clave: `h|${h.id}`, hora: h.hora, t: ms(h.hora), tipo: "espejo", tono: TONO_TAREA[h.resultado], titulo: "Espejo de lo que guarda", chip: TEXTO_TAREA[h.resultado], detalle: h.mensaje ?? null, meta: null, repo: null, copia: null, vuelta: null });
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
