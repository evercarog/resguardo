// La regla 3-2-1-1-0 como guía (tarea 8, docs/regla-3-2-1.md).
//
// Dos partes:
//
// 1. `regla321`: la regla, igual que `regla_321` de crates/agente/src/protection.rs
//    (las dos pasan crates/protocolo/vectors/regla-321.json en
//    scripts/vectores-regla.ts). Si cambia una, cambia la otra.
// 2. `reglaDeCopia`: la entrada de cada copia, con lo que ve la consola: el
//    destino de su repositorio (una zona de un almacén, la nube…), el espejo
//    del almacén que lo incluye, su copia externa, y lo que la persona dijo de
//    cada destino en el catálogo (`atributos`). Cuando lleguen las cadenas
//    (tarea 7, parte B: espejos por repositorio, copias derivadas), son más
//    pasos en la misma lista.
//
// Guía, nunca obligación: nada de esto bloquea un botón. Y nunca cuenta el
// sistema operativo ni el sistema de archivos (8e): solo lo que dice la persona.
// Sin dependencias de Svelte (lo prueban los vectores).
import type { AtributosDestino, CopiaResumen, DestinoCatalogo, DestinoResumen, Equipo, Informe, InmutableDestino, LugarDestino, RepositorioResumen } from "./tipos";
import { claveNube, claveZona, nombreDestino, nombreZonaPorDefecto, unidadDe, zonaDeDestino, zonasDe, type DestinoVista, type ZonaVista } from "./destinos";
import { espejoDelRepo, nombreEnAlmacen } from "./espejo";
import { destinoDe, informeDe } from "./repo";
import { resultadoConError } from "./salud";
import { proximaProgramada, ultimaProgramada } from "./copia";
import { servidorDe } from "./dondeGuarda";
import { lista, plural, relativo } from "./formato";

// ---------- 1. La regla (como protection.rs) ----------

export type IdParte = "copias" | "soportes" | "fuera" | "inmutable" | "errores";

export interface OrigenRegla {
  equipo: string;
  soporte: string;
}

export interface PasoRegla {
  id: string;
  nombre?: string;
  /** `copia`, `espejo`, `externa`, `derivada` o `paso` (parte B): solo informa. */
  tipo?: string;
  lugar?: LugarDestino;
  inmutable?: InmutableDestino;
  soporte: string;
  /** El equipo que lo guarda (sin él: la nube o un servidor de fuera). */
  equipo?: string | null;
  /** RFC 3339: la última vez que se puso al día bien. */
  ultima_ok?: string | null;
  /** Cada cuánto le toca (sin él, cada día). */
  cada_horas?: number | null;
  /** Su comprobación encontró datos dañados. */
  verificacion_mal?: boolean;
}

export interface PruebaRegla {
  configurada: boolean;
  ultima_ok?: string | null;
  fallo?: boolean;
}

export interface EntradaRegla {
  origen: OrigenRegla;
  pasos: PasoRegla[];
  verificacion: PruebaRegla;
  prueba_restauracion: PruebaRegla;
}

export interface ParteRegla {
  id: IdParte;
  meta: number;
  valor: number;
  valor_config: number;
  cumple: boolean;
  cumple_config: boolean;
  /** Qué hacer (código; vacío si cumple). */
  accion: string;
  detalle: string;
}

export interface Regla321 {
  cumple: boolean;
  cumple_config: boolean;
  /** La configuración cumple pero hoy algo no está al día o falló. */
  dejo_de_cumplir: boolean;
  partes: ParteRegla[];
  atrasados: string[];
  /** `mismo_equipo`, `inmutable_local`. */
  avisos: string[];
}

/** Días como mucho desde la última verificación o prueba correcta. */
export const DIAS_PRUEBA_REGLA = 45;
const HORA = 3600_000;

/** Horas que puede pasar un paso sin ponerse al día: su horario y la mitad, más 12 h. */
export function margenHoras(cadaHoras: number | null | undefined): number {
  const c = cadaHoras != null && Number.isFinite(cadaHoras) && cadaHoras > 0 ? cadaHoras : 24;
  return c * 1.5 + 12;
}

/** ¿Está al día este paso? */
export function pasoAlDia(p: PasoRegla, ahora: number): boolean {
  const t = p.ultima_ok ? Date.parse(p.ultima_ok) : NaN;
  if (!Number.isFinite(t)) return false;
  return Math.trunc((ahora - t) / 1000) / 3600 <= margenHoras(p.cada_horas);
}

function pruebaBien(p: PruebaRegla, ahora: number): boolean {
  const t = p.ultima_ok ? Date.parse(p.ultima_ok) : NaN;
  return p.configurada && !p.fallo && Number.isFinite(t) && ahora - t <= DIAS_PRUEBA_REGLA * 24 * HORA;
}

const fueraDeLaOficina = (l: LugarDestino | undefined) => l === "otra_sede" || l === "nube";
const esInmutable = (i: InmutableDestino | undefined) => !!i && i !== "no";

function parte(id: IdParte, meta: number, valor: number, valorConfig: number, accionFalta: string, alDia: string, detalle: string): ParteRegla {
  const [cumple, cumpleConfig] = id === "errores" ? [valor === 0, valorConfig === 0] : [valor >= meta, valorConfig >= meta];
  return { id, meta, valor, valor_config: valorConfig, cumple, cumple_config: cumpleConfig, accion: cumple ? "" : cumpleConfig ? alDia : accionFalta, detalle };
}

/** La regla 3-2-1-1-0 de una copia (la misma que `protection::regla_321`). */
export function regla321(e: EntradaRegla, ahora: number): Regla321 {
  const alDia = e.pasos.filter((p) => pasoAlDia(p, ahora));
  const todos = e.pasos;
  const atrasados = e.pasos.filter((p) => !pasoAlDia(p, ahora)).map((p) => p.id);
  const soportes = (l: PasoRegla[]) => new Set([e.origen.soporte, ...l.map((p) => p.soporte)]).size;
  const fuera = (l: PasoRegla[]) => l.filter((p) => fueraDeLaOficina(p.lugar)).length;
  const inmutables = (l: PasoRegla[]) => l.filter((p) => esInmutable(p.inmutable)).length;
  const n = (x: number) => plural(x, "destino", "destinos");

  const [c, cc] = [1 + alDia.length, 1 + todos.length];
  const [s, sc] = [soportes(alDia), soportes(todos)];
  const [f, fc] = [fuera(alDia), fuera(todos)];
  const [i, ic] = [inmutables(alDia), inmutables(todos)];

  const problemas: string[] = [];
  let estructurales = 0;
  for (const [p, programar, revisar] of [
    [e.verificacion, "programar_verificacion", "revisar_verificacion"],
    [e.prueba_restauracion, "programar_prueba", "revisar_prueba"],
  ] as const) {
    if (!p.configurada) {
      problemas.push(programar);
      estructurales++;
    } else if (!pruebaBien(p, ahora)) problemas.push(revisar);
  }
  if (e.pasos.some((p) => p.verificacion_mal)) problemas.push("revisar_destino");
  // Lo que falta programar va antes que lo que hay que revisar (orden estable).
  const ordenados = [...problemas.filter((p) => p.startsWith("programar")), ...problemas.filter((p) => !p.startsWith("programar"))];
  const primero = ordenados[0] ?? "";

  const partes = [
    parte("copias", 3, c, cc, "anadir_destino", "poner_al_dia", `${c} de 3: los originales y ${n(c - 1)} al día.`),
    parte("soportes", 2, s, sc, "otro_soporte", "poner_al_dia", `${s} de 2 soportes distintos (equipo y disco).`),
    parte("fuera", 1, f, fc, "anadir_fuera", "poner_al_dia", `${n(f)} fuera de la oficina.`),
    parte("inmutable", 1, i, ic, "anadir_inmutable", "poner_al_dia", `${n(i)} inmutable o fuera del alcance de los equipos.`),
    parte(
      "errores",
      0,
      problemas.length,
      estructurales,
      primero,
      primero,
      problemas.length ? `${problemas.length} por resolver al verificar o probar la restauración.` : "Verificación y prueba de restauración recientes y sin errores.",
    ),
  ];
  const cumple = partes.every((p) => p.cumple);
  const cumpleConfig = partes.every((p) => p.cumple_config);

  const avisos: string[] = [];
  const porEquipo = new Map<string, Set<string>>([[e.origen.equipo, new Set([e.origen.soporte])]]);
  for (const p of todos) if (p.equipo) porEquipo.set(p.equipo, new Set([...(porEquipo.get(p.equipo) ?? []), p.soporte]));
  if ([...porEquipo.values()].some((x) => x.size >= 2)) avisos.push("mismo_equipo");
  if (ic > 0 && !todos.some((p) => esInmutable(p.inmutable) && fueraDeLaOficina(p.lugar))) avisos.push("inmutable_local");
  return { cumple, cumple_config: cumpleConfig, dejo_de_cumplir: cumpleConfig && !cumple, partes, atrasados, avisos };
}

// ---------- 2. La entrada de cada copia, con lo que ve la consola ----------

/** Un paso con lo que hace falta para enseñarlo. */
export interface PasoVista extends PasoRegla {
  nombre: string;
  tipo: "copia" | "espejo" | "externa" | "derivada" | "paso";
  lugar: LugarDestino;
  inmutable: InmutableDestino;
  /** La clave del destino en el catálogo (para marcarlo), si la tiene. */
  clave: string | null;
  /** Lo deducido del tipo (antes de lo que dice la persona). */
  porDefecto: Required<Pick<AtributosDestino, "lugar" | "inmutable">>;
  /** Lo que dice la persona en el catálogo. */
  marcado: AtributosDestino | null;
  /** El equipo que lo hace o lo guarda (para el enlace «Ponlo al día»). */
  equipoNombre: string | null;
  equipoId: string | null;
  /** Tarea 8e: el sistema de archivos de su carpeta (solo un dato). */
  sistemaArchivos: string | null;
  /** Para guardarlo en el catálogo: su tipo y, si es de red, su dirección. */
  tipoCatalogo: DestinoCatalogo["tipo"];
  donde: string | null;
}

/** Lo que hace falta para marcar un destino (AtributosDestino.svelte). */
export interface MarcarDestino {
  clave: string;
  nombre: string;
  tipo: DestinoCatalogo["tipo"];
  donde: string | null;
  porDefecto: Required<Pick<AtributosDestino, "lugar" | "inmutable">>;
  /** Su entrada del catálogo (nombre propio y lo marcado), si la tiene. */
  catalogo: DestinoCatalogo | null;
  sistemaArchivos: string | null;
  /** El equipo que lo guarda, para enseñar su entorno (8e). */
  equipo: Equipo | null;
}

/** El tipo del catálogo de un destino de un equipo. */
const tipoCatalogoDe = (d: DestinoResumen | undefined, zona: boolean): DestinoCatalogo["tipo"] =>
  zona ? "zona" : d && (["rest", "s3", "b2", "sftp", "nube"] as const).includes(d.tipo as "rest") ? (d.tipo as DestinoCatalogo["tipo"]) : "local";

export interface ReglaCopia {
  equipo: Equipo;
  copia: CopiaResumen;
  repo: RepositorioResumen;
  entrada: EntradaRegla;
  pasos: PasoVista[];
  regla: Regla321;
}

/** Un resumen corto (FNV-1a de 32 bits) para la clave de una carpeta del espejo: la ruta no va al servidor. */
export function resumenCorto(texto: string): string {
  let h = 0x811c9dc5;
  for (const ch of texto) {
    h ^= ch.codePointAt(0)!;
    h = Math.imul(h, 0x01000193) >>> 0;
  }
  return h.toString(16).padStart(8, "0");
}

/** La clave en el catálogo de una carpeta del espejo de un almacén. */
export const claveEspejoCarpeta = (almacen: string, carpeta: string) => `espejo:${almacen}:${resumenCorto(carpeta.trim().replace(/[\\/]+$/, "").toLowerCase())}`;

/** Las horas entre dos vueltas de un horario (24 si no se sabe). */
export function horasEntre(horario: CopiaResumen["horario"] | null | undefined, ahora: number): number {
  const a = horario ? ultimaProgramada(horario, ahora) : null;
  const b = horario ? proximaProgramada(horario, ahora) : null;
  if (a == null || b == null || b <= a) return 24;
  return Math.max(0.25, (b - a) / HORA);
}

/** Lo que se deduce de un destino de un equipo (8a, «Valores por defecto» de docs/regla-3-2-1.md). */
function deducirDestino(
  d: DestinoResumen | undefined,
  e: Equipo,
  equipos: Equipo[],
  inmutableComprobado: boolean,
): { lugar: LugarDestino; inmutable: InmutableDestino; soporte: string; equipo: string | null; zona: ZonaVista | null; fs: string | null } {
  const origen = `equipo:${e.id}:origen`;
  if (!d) return { lugar: "este_equipo", inmutable: "no", soporte: origen, equipo: e.id, zona: null, fs: null };
  const z = zonaDeDestino(d, equipos);
  if (z) {
    const a = z.almacen;
    const fs = z.principal ? (a.resumen?.guarda_copias?.sistema_archivos ?? null) : (a.resumen?.guarda_copias?.zonas?.find((x) => x.id === z.id)?.sistema_archivos ?? null);
    return { lugar: a.id === e.id ? "este_equipo" : "oficina", inmutable: "solo_anadir", soporte: `equipo:${a.id}:${unidadDe(z.carpeta) ?? z.id}`, equipo: a.id, zona: z, fs };
  }
  switch (d.tipo) {
    case "rest":
      return { lugar: "otra_sede", inmutable: inmutableComprobado || d.inmutable ? "solo_anadir" : "no", soporte: `servidor:${servidorDe(d.donde) ?? d.id}`, equipo: null, zona: null, fs: null };
    case "s3":
    case "b2":
      return { lugar: "nube", inmutable: d.inmutable ? "object_lock" : "no", soporte: `nube:${d.tipo}:${d.donde ?? d.id}`, equipo: null, zona: null, fs: null };
    case "sftp":
      return { lugar: "otra_sede", inmutable: "no", soporte: `servidor:${servidorDe(d.donde) ?? d.id}`, equipo: null, zona: null, fs: null };
    // Tarea 4a: una nube conectada en el propio equipo (Dropbox, Drive… por rclone): fuera y no inmutable.
    case "nube":
      return { lugar: "nube", inmutable: "no", soporte: `nube:${d.nube ?? d.id}`, equipo: null, zona: null, fs: null };
    case "local":
      if (d.red) return { lugar: "oficina", inmutable: "no", soporte: `red:${d.id}`, equipo: null, zona: null, fs: null };
      // Una carpeta del propio equipo: el mismo soporte que los originales (no se sabe en qué disco están).
      if (d.extraible) return { lugar: "este_equipo", inmutable: "no", soporte: `equipo:${e.id}:usb:${d.unidad ?? d.id}`, equipo: e.id, zona: null, fs: d.sistema_archivos ?? null };
      return { lugar: "este_equipo", inmutable: "no", soporte: origen, equipo: e.id, zona: null, fs: d.sistema_archivos ?? null };
    default:
      return { lugar: "este_equipo", inmutable: "no", soporte: `destino:${d.id}`, equipo: e.id, zona: null, fs: null };
  }
}

/** Lo deducido con lo que dice la persona encima. */
function conMarcado(p: Omit<PasoVista, "marcado" | "porDefecto">, catalogo: DestinoCatalogo[]): PasoVista {
  const marcado = (p.clave && catalogo.find((c) => c.id === p.clave)?.atributos) || null;
  return {
    ...p,
    porDefecto: { lugar: p.lugar, inmutable: p.inmutable },
    marcado,
    lugar: marcado?.lugar ?? p.lugar,
    inmutable: marcado?.inmutable ?? p.inmutable,
    soporte: marcado?.soporte ? `marcado:${marcado.soporte.toLowerCase()}` : p.soporte,
  };
}

/** La última vez que la copia salió bien (del resumen o del informe). */
function ultimaBien(k: CopiaResumen, informe: Informe | null | undefined): string | null {
  const desdeInforme = informeDe(informe, k.repo)
    ?.ejecuciones.filter((x) => x.copia === k.id && x.resultado !== "fallo")
    .map((x) => x.hora)
    .sort()
    .at(-1);
  const delResumen = k.ultima && k.ultima.estado !== "fallo" ? k.ultima.cuando : null;
  return [desdeInforme, delResumen].filter((x): x is string => !!x).sort().at(-1) ?? null;
}

/**
 * La regla de una copia (activa, en un repositorio que no es de solo lectura),
 * o null. `informe`: el último del equipo; `catalogo`: el del cliente.
 */
export function reglaDeCopia(e: Equipo, k: CopiaResumen, equipos: Equipo[], informe: Informe | null | undefined, catalogo: DestinoCatalogo[], ahora: number): ReglaCopia | null {
  if (k.activa === false) return null;
  const r = e.resumen?.repositorios?.find((x) => x.id === k.repo);
  if (!r || r.solo_lectura) return null;
  const d = destinoDe(e.resumen?.destinos, r);
  const cadaCopia = horasEntre(k.horario, ahora);
  const pasos: PasoVista[] = [];

  // 1. El destino del repositorio.
  const x = deducirDestino(d, e, equipos, !!r.solo_anadir);
  const claveDestino = x.zona ? claveZona(x.zona.almacen.id, x.zona.id) : d ? d.id : null;
  pasos.push(
    conMarcado(
      {
        id: "destino",
        nombre: nombreDestino(d, equipos, catalogo) ?? r.destino,
        tipo: "copia",
        lugar: x.lugar,
        inmutable: x.inmutable,
        soporte: x.soporte,
        equipo: x.equipo,
        ultima_ok: ultimaBien(k, informe),
        cada_horas: cadaCopia,
        clave: claveDestino,
        equipoNombre: e.nombre,
        equipoId: e.id,
        sistemaArchivos: x.fs,
        tipoCatalogo: tipoCatalogoDe(d, !!x.zona),
        donde: !x.zona && d && d.tipo !== "local" ? (d.donde ?? null) : null,
      },
      catalogo,
    ),
  );

  // 2. El espejo del almacén que lo incluye. Tarea 7d.2: cada destino copia desde su
  //    zona (sin ella, la principal) y puede ir a otra zona del almacén.
  if (x.zona) {
    const a = x.zona.almacen;
    const g = a.resumen?.guarda_copias;
    const zonaId = x.zona.id;
    const esp = espejoDelRepo(g?.espejo ?? null, nombreEnAlmacen(d?.donde, r));
    (esp?.destinos ?? []).forEach((dd, i) => {
      if ((dd.zona ?? "principal") !== zonaId) return;
      const nube = dd.tipo === "nube";
      // A otra zona del mismo almacén: su disco, con el nombre de la zona.
      const zonaDestino = dd.tipo === "zona" ? zonasDe(a).find((z) => z.id === dd.carpeta) : undefined;
      if (zonaDestino) {
        pasos.push(
          conMarcado(
            {
              id: `espejo-${i + 1}`,
              nombre: catalogo.find((c) => c.id === claveZona(a.id, zonaDestino.id))?.nombre.trim() || nombreZonaPorDefecto(zonaDestino),
              tipo: "espejo",
              lugar: x.lugar,
              inmutable: "no",
              soporte: `equipo:${a.id}:${unidadDe(zonaDestino.carpeta) ?? `zona-${zonaDestino.id}`}`,
              equipo: a.id,
              ultima_ok: resultadoConError(dd.resultado) ? null : (dd.ultima ?? null),
              cada_horas: dd.tras_copia ? cadaCopia : dd.horario ? horasEntre(dd.horario, ahora) : 24,
              verificacion_mal: (dd.verificacion?.mal ?? 0) > 0,
              clave: claveZona(a.id, zonaDestino.id),
              equipoNombre: a.nombre,
              equipoId: a.id,
              sistemaArchivos: null,
              tipoCatalogo: "zona",
              donde: null,
            },
            catalogo,
          ),
        );
        return;
      }
      const tipoNube = nube ? (g?.nubes?.find((n) => n.nombre === dd.nube)?.tipo ?? "") : "";
      const lugar: LugarDestino = nube ? (tipoNube === "smb" ? "oficina" : tipoNube === "sftp" ? "otra_sede" : "nube") : x.lugar;
      const carpeta = dd.carpeta ?? "";
      const clave = nube ? claveNube(a.id, dd.nube ?? "") : carpeta ? claveEspejoCarpeta(a.id, carpeta) : null;
      pasos.push(
        conMarcado(
          {
            id: `espejo-${i + 1}`,
            nombre: (clave && catalogo.find((c) => c.id === clave)?.nombre.trim()) || (nube ? (dd.nube ?? "Nube") : `Espejo en ${unidadDe(carpeta) ? `el disco ${unidadDe(carpeta)}` : "otra carpeta"} de ${a.nombre}`),
            tipo: "espejo",
            lugar,
            inmutable: dd.bloqueo ? "object_lock" : "no",
            soporte: nube ? `nube:${clave}` : `equipo:${a.id}:${unidadDe(carpeta) ?? `espejo-${resumenCorto(carpeta)}`}`,
            equipo: nube ? null : a.id,
            ultima_ok: resultadoConError(dd.resultado) ? null : (dd.ultima ?? null),
            cada_horas: dd.tras_copia ? cadaCopia : dd.horario ? horasEntre(dd.horario, ahora) : 24,
            verificacion_mal: (dd.verificacion?.mal ?? 0) > 0,
            clave,
            equipoNombre: a.nombre,
            equipoId: a.id,
            sistemaArchivos: nube ? null : (dd.sistema_archivos ?? null),
            tipoCatalogo: nube ? "nube" : "local",
            donde: null,
          },
          catalogo,
        ),
      );
    });
  }

  // 3. La copia externa del repositorio (cada día).
  if (r.externa) {
    const de = e.resumen?.destinos?.find((y) => (r.externa?.destino_id ? y.id === r.externa.destino_id : y.nombre === r.externa?.destino));
    const xe = deducirDestino(de, e, equipos, !!r.externa.solo_anadir);
    const inmutable: InmutableDestino = r.externa.bloqueo_dias ? "object_lock" : r.externa.solo_anadir ? "solo_anadir" : xe.inmutable;
    const ext = informeDe(informe, r.id)?.externa ?? null;
    pasos.push(
      conMarcado(
        {
          id: "externa",
          nombre: nombreDestino(de, equipos, catalogo) ?? r.externa.destino,
          tipo: "externa",
          lugar: de ? xe.lugar : "otra_sede",
          inmutable,
          soporte: de ? xe.soporte : `destino:${r.externa.destino}`,
          equipo: xe.equipo,
          ultima_ok: ext && ext.resultado !== "fallo" ? ext.ultima : null,
          cada_horas: 24,
          clave: xe.zona ? claveZona(xe.zona.almacen.id, xe.zona.id) : (de?.id ?? null),
          equipoNombre: e.nombre,
          equipoId: e.id,
          sistemaArchivos: xe.fs,
          tipoCatalogo: tipoCatalogoDe(de, !!xe.zona),
          donde: !xe.zona && de && de.tipo !== "local" ? (de.donde ?? null) : null,
        },
        catalogo,
      ),
    );
  }
  // 4. Tarea 4b: las demás copias derivadas del repositorio (como la externa, cada
  //    una con su destino, su bloqueo y su última vuelta del informe).
  for (const dv of r.derivadas ?? []) {
    if (dv.activa === false) continue;
    const de = e.resumen?.destinos?.find((y) => y.id === dv.destino_id);
    const xd = deducirDestino(de, e, equipos, !!dv.solo_anadir);
    const inmutable: InmutableDestino = dv.bloqueo_dias ? "object_lock" : dv.solo_anadir ? "solo_anadir" : xd.inmutable;
    const ult = informeDe(informe, r.id)?.derivadas?.find((y) => y.id === dv.id) ?? null;
    pasos.push(
      conMarcado(
        {
          id: `derivada-${dv.id}`,
          nombre: nombreDestino(de, equipos, catalogo) ?? dv.destino ?? "Copia derivada",
          tipo: "derivada",
          lugar: de ? xd.lugar : "otra_sede",
          inmutable,
          soporte: de ? xd.soporte : `destino:${dv.destino_id ?? dv.id}`,
          equipo: xd.equipo,
          ultima_ok: ult && ult.resultado !== "fallo" ? (ult.ultima ?? null) : null,
          cada_horas: dv.cuando?.tras_copia ? cadaCopia : dv.cuando?.horario ? horasEntre(dv.cuando.horario, ahora) : 24,
          verificacion_mal: ult?.verificacion?.resultado === "fallo",
          clave: xd.zona ? claveZona(xd.zona.almacen.id, xd.zona.id) : (de?.id ?? null),
          equipoNombre: e.nombre,
          equipoId: e.id,
          sistemaArchivos: xd.fs,
          tipoCatalogo: tipoCatalogoDe(de, !!xd.zona),
          donde: !xd.zona && de && de.tipo !== "local" && de.tipo !== "nube" ? (de.donde ?? null) : null,
        },
        catalogo,
      ),
    );
  }

  // Verificación y prueba de restauración del repositorio.
  const inf = informeDe(informe, r.id);
  const v = inf?.verificacion ?? null;
  const verificacion: PruebaRegla = {
    configurada: !!r.verificacion_auto || !!v || !!r.verificado,
    ultima_ok: v?.ultima ?? r.verificado ?? null,
    fallo: v?.resultado === "fallo",
  };
  const pr = inf?.prueba_restauracion ?? null;
  const itemPrueba = inf?.proteccion?.items.find((it) => it.id === "restauracion");
  const prueba: PruebaRegla = {
    // Programada (en el agente o desde la consola) o hecha alguna vez a mano.
    configurada: !!r.prueba_auto || !!pr || !!r.prueba_restauracion || (!!itemPrueba && !/^Sin prueba/i.test(itemPrueba.detalle)),
    ultima_ok: pr?.ultima ?? r.prueba_restauracion ?? null,
    fallo: pr?.resultado === "fallo",
  };
  const entrada: EntradaRegla = { origen: { equipo: e.id, soporte: `equipo:${e.id}:origen` }, pasos, verificacion, prueba_restauracion: prueba };
  return { equipo: e, copia: k, repo: r, entrada, pasos, regla: regla321(entrada, ahora) };
}

/** Las de todas las copias del cliente (sin los equipos trasladados). */
export function reglasDelCliente(equipos: Equipo[], informes: Record<string, Informe | null | undefined>, catalogo: DestinoCatalogo[], ahora: number, todos: Equipo[] = equipos): ReglaCopia[] {
  return equipos
    .filter((e) => e.modo !== "trasladado")
    .flatMap((e) => (e.resumen?.copias ?? []).map((k) => reglaDeCopia(e, k, todos, informes[e.id], catalogo, ahora)))
    .filter((x): x is ReglaCopia => !!x);
}

/**
 * La regla de una copia mientras se edita («Cambiar las copias»): con su
 * repositorio, horario y lo que se va a enviar (verificación y prueba de
 * restauración programadas). Lo que importa aquí es la configuración
 * (`cumple_config`): una copia nueva aún no ha llegado a ningún sitio.
 */
export function reglaEnEdicion(
  e: Equipo,
  k: Pick<CopiaResumen, "id" | "nombre" | "repo" | "horario" | "activa">,
  equipos: Equipo[],
  informe: Informe | null | undefined,
  catalogo: DestinoCatalogo[],
  ahora: number,
  programado: { verificacion?: boolean; prueba?: boolean } = {},
): ReglaCopia | null {
  const previa = e.resumen?.copias?.find((x) => x.id === k.id);
  const rc = reglaDeCopia(e, { ...previa, ...k, activa: k.activa !== false }, equipos, informe, catalogo, ahora);
  if (!rc) return null;
  const entrada: EntradaRegla = {
    ...rc.entrada,
    verificacion: { ...rc.entrada.verificacion, configurada: !!programado.verificacion || rc.entrada.verificacion.configurada },
    prueba_restauracion: { ...rc.entrada.prueba_restauracion, configurada: !!programado.prueba || rc.entrada.prueba_restauracion.configurada },
  };
  return { ...rc, entrada, regla: regla321(entrada, ahora) };
}

/** La frase de la configuración: «Con esta configuración cumple…» o «Le faltaría: …». */
export function fraseConfig(r: Regla321): string {
  if (r.cumple_config) return r.cumple ? "Cumple la regla 3-2-1-1-0." : "Con esta configuración cumple la regla 3-2-1-1-0 (cuando todo esté al día).";
  return `Para la regla 3-2-1-1-0 le falta: ${lista(r.partes.filter((p) => !p.cumple_config).map((p) => PARTES[p.id].titulo))}.`;
}

/** Para marcar el destino de un paso (desde la tira de una copia). */
export function marcarDesdePaso(p: PasoVista, equipos: Equipo[], catalogo: DestinoCatalogo[]): MarcarDestino | null {
  if (!p.clave) return null;
  const c = catalogo.find((x) => x.id === p.clave) ?? null;
  return { clave: p.clave, nombre: p.nombre, tipo: p.tipoCatalogo, donde: p.donde, porDefecto: p.porDefecto, catalogo: c, sistemaArchivos: p.sistemaArchivos, equipo: equipos.find((e) => e.id === p.equipo) ?? null };
}

/** Lo deducido de un destino de la lista de «Repositorios y destinos» (sin una copia concreta). */
export function marcarDesdeVista(v: DestinoVista, equipos: Equipo[]): MarcarDestino {
  const base = { clave: v.clave, nombre: v.nombre, donde: v.donde, catalogo: v.catalogo ?? null };
  if (v.zona) {
    const a = v.zona.almacen;
    const fs = v.zona.principal ? (a.resumen?.guarda_copias?.sistema_archivos ?? null) : (a.resumen?.guarda_copias?.zonas?.find((z) => z.id === v.zona!.id)?.sistema_archivos ?? null);
    return { ...base, tipo: "zona", donde: null, porDefecto: { lugar: "oficina", inmutable: "solo_anadir" }, sistemaArchivos: fs, equipo: a };
  }
  if (v.nube) {
    const t = v.nube.tipo;
    const lugar: LugarDestino = t === "smb" ? "oficina" : t === "sftp" ? "otra_sede" : "nube";
    return { ...base, tipo: "nube", donde: null, porDefecto: { lugar, inmutable: "no" }, sistemaArchivos: null, equipo: v.nube.equipo };
  }
  const d: DestinoResumen = v.destino ?? { id: v.clave, nombre: v.nombre, tipo: (v.tipo as DestinoResumen["tipo"]) ?? "otro", donde: v.donde ?? undefined };
  const e = equipos.find((x) => v.equipos.includes(x.nombre)) ?? equipos[0];
  const x = e ? deducirDestino(d, e, equipos, !!d.inmutable) : { lugar: "otra_sede" as LugarDestino, inmutable: "no" as InmutableDestino, fs: null };
  return { ...base, tipo: tipoCatalogoDe(d, false), porDefecto: { lugar: x.lugar, inmutable: x.inmutable }, sistemaArchivos: x.fs, equipo: d.tipo === "local" ? (e ?? null) : null };
}

export interface CuentaRegla {
  total: number;
  cumplen: number;
  /** Las que cumplían (su configuración cumple) y hoy no. */
  dejaron: number;
  /** Cuántas copias fallan cada parte. */
  faltan: Record<IdParte, number>;
}

export function cuentaRegla(rs: ReglaCopia[]): CuentaRegla {
  const faltan: Record<IdParte, number> = { copias: 0, soportes: 0, fuera: 0, inmutable: 0, errores: 0 };
  for (const r of rs) for (const p of r.regla.partes) if (!p.cumple) faltan[p.id]++;
  return { total: rs.length, cumplen: rs.filter((r) => r.regla.cumple).length, dejaron: rs.filter((r) => r.regla.dejo_de_cumplir).length, faltan };
}

// ---------- Lo que se ve ----------

/** La cifra y el nombre de cada parte. */
export const PARTES: Record<IdParte, { cifra: string; titulo: string; corto: string }> = {
  copias: { cifra: "3", titulo: "3 copias", corto: "copias" },
  soportes: { cifra: "2", titulo: "2 soportes", corto: "soportes" },
  fuera: { cifra: "1", titulo: "1 fuera de la oficina", corto: "fuera" },
  inmutable: { cifra: "1", titulo: "1 inmutable", corto: "inmutable" },
  errores: { cifra: "0", titulo: "0 errores", corto: "errores" },
};

export const TEXTO_LUGAR: Record<LugarDestino, string> = {
  este_equipo: "En este mismo equipo",
  oficina: "En otro equipo de la oficina",
  otra_sede: "En otra sede",
  nube: "En la nube",
};

export const TEXTO_INMUTABLE: Record<InmutableDestino, string> = {
  solo_anadir: "Solo añadir (desde el equipo no se puede borrar)",
  object_lock: "Bloqueo de objetos (Object Lock)",
  instantaneas: "Con instantáneas inmutables fuera de su alcance",
  desconectado: "Desconectado (un disco que se rota)",
  no: "No es inmutable",
};

/** El valor de una parte en corto: «2 de 3», «0 errores», «1 de 1». */
export function cifraParte(p: ParteRegla): string {
  if (p.id === "errores") return p.valor === 0 ? "sin errores" : plural(p.valor, "por resolver", "por resolver");
  return p.valor > p.meta ? `${p.valor} (pide ${p.meta})` : `${p.valor} de ${p.meta}`;
}

/**
 * El estado de una parte en una o dos palabras (la tira, debajo de su nombre):
 * «Cumple», «Atrasado» (la configuración cumple, pero algo no está al día) o
 * «Falta 1» / «Faltan 2» (en el «0», lo que hay por resolver). Con su tono y su icono: el
 * estado nunca va solo en el color.
 */
export function estadoParte(p: ParteRegla): { tono: "ok" | "warn" | "neutral"; icono: "cumple" | "atrasado" | "falta"; texto: string } {
  if (p.cumple) return { tono: "ok", icono: "cumple", texto: "Cumple" };
  if (p.cumple_config) return { tono: "warn", icono: "atrasado", texto: "Atrasado" };
  // En el «0», lo que hay por resolver (verificación, prueba, datos dañados); en las demás, lo que falta.
  const n = p.id === "errores" ? Math.max(1, p.valor) : Math.max(1, p.meta - p.valor);
  return { tono: "neutral", icono: "falta", texto: n === 1 ? "Falta 1" : `Faltan ${n}` };
}

/** El globo de una parte: qué pide, cómo está y, si falta algo, qué hacer. */
export function globoParte(p: ParteRegla, rc: ReglaCopia, cliente: string, ahora: number): string {
  const que = queHacer(p, rc, cliente, ahora);
  return [`${PARTES[p.id].titulo}: ${cifraParte(p)}.`, fraseParte(p, rc, ahora), que ? `Qué hacer: ${que.texto}` : ""].filter(Boolean).join(" ");
}

/** Lo que se ve debajo de la tira: una línea corta, «Falta: 1 fuera de la oficina · 1 inmutable», o null si cumple. */
export function lineaFalta(r: Regla321): string | null {
  if (r.cumple) return null;
  const falta = r.partes.filter((p) => !p.cumple_config).map((p) => PARTES[p.id].titulo);
  const atrasado = r.partes.filter((p) => !p.cumple && p.cumple_config).map((p) => PARTES[p.id].titulo);
  return [falta.length ? `Falta: ${falta.join(" · ")}` : "", atrasado.length ? `No está al día: ${atrasado.join(" · ")}` : ""].filter(Boolean).join(". ") + ".";
}

/** Lo que dice la parte, en una frase, con los nombres de los destinos. */
export function fraseParte(p: ParteRegla, rc: ReglaCopia, ahora: number): string {
  const al = rc.pasos.filter((x) => pasoAlDia(x, ahora));
  const nombres = (l: PasoVista[]) => lista(l.map((x) => `«${x.nombre}»`));
  switch (p.id) {
    case "copias":
      return al.length ? `Los originales y ${nombres(al)}.` : "Solo los originales.";
    case "soportes":
      return `${p.valor === 1 ? "Un solo soporte" : `${p.valor} soportes`}: equipo y disco distintos.`;
    case "fuera": {
      const f = al.filter((x) => fueraDeLaOficina(x.lugar));
      return f.length ? `${nombres(f)}.` : "Ninguno al día fuera de la oficina.";
    }
    case "inmutable": {
      const i = al.filter((x) => esInmutable(x.inmutable));
      return i.length ? `${nombres(i)}.` : "Ninguno al día que no se pueda borrar desde los equipos.";
    }
    case "errores":
      return p.valor ? p.detalle : "Verificación y prueba de restauración recientes y correctas.";
  }
}

export interface QueHacer {
  texto: string;
  enlace?: { texto: string; href: string };
}

/** Qué hacer para cumplir una parte (con un enlace a donde se hace hoy). */
export function queHacer(p: ParteRegla, rc: ReglaCopia, cliente: string, ahora: number): QueHacer | null {
  if (p.cumple) return null;
  const e = rc.equipo;
  const repo = `/c/${cliente}/equipos/${e.id}/repositorios/${encodeURIComponent(rc.repo.id)}`;
  const externa = { texto: "Copia externa", href: `/c/${cliente}/equipos/${e.id}?externa=${encodeURIComponent(rc.repo.id)}` };
  // Los atrasados que importan para esta parte (fuera: los de fuera; inmutable: los inmutables), primero.
  const importa = (x: PasoVista) => (p.id === "fuera" ? fueraDeLaOficina(x.lugar) : p.id === "inmutable" ? esInmutable(x.inmutable) : true);
  const todosAtrasados = rc.pasos.filter((x) => rc.regla.atrasados.includes(x.id));
  const atrasados = [...todosAtrasados.filter(importa), ...todosAtrasados.filter((x) => !importa(x))];
  switch (p.accion) {
    case "poner_al_dia": {
      const x = atrasados[0];
      if (!x) return { texto: "Ponla al día." };
      const desde = x.ultima_ok ? `no se pone al día desde ${relativo(x.ultima_ok, ahora)}` : "no está al día (la última vez falló o aún no se ha hecho)";
      const href = x.tipo === "espejo" && x.equipoId ? `/c/${cliente}/equipos/${x.equipoId}` : x.tipo === "copia" ? `/c/${cliente}/equipos/${e.id}/copias/${encodeURIComponent(rc.copia.id)}` : repo;
      return { texto: `«${x.nombre}» ${desde}${atrasados.length > 1 ? ` (y ${plural(atrasados.length - 1, "paso más", "pasos más")})` : ""}: revisa por qué.`, enlace: { texto: `Revisar «${x.nombre}»`, href } };
    }
    case "anadir_destino":
      return { texto: "Añade otro destino: una copia externa (la nube u otro disco) o un espejo del almacén.", enlace: externa };
    case "otro_soporte":
      return { texto: "Guarda una copia en otro equipo u otro disco: un almacén de la oficina, un disco USB o la nube.", enlace: { texto: "Repositorios y destinos", href: `/c/${cliente}/repositorios` } };
    case "anadir_fuera":
      return { texto: "Añade un destino fuera de la oficina: Backblaze B2 (con bloqueo de objetos) o Dropbox, como copia externa o espejo del almacén.", enlace: externa };
    case "anadir_inmutable":
      return {
        texto: "Añade un destino que no se pueda borrar desde los equipos: un almacén (solo añadir), B2 o S3 con bloqueo de objetos, o marca un destino con instantáneas del anfitrión o desconectado.",
        enlace: { texto: "Destinos", href: `/c/${cliente}/repositorios#destinos` },
      };
    case "programar_verificacion":
      return { texto: "Programa la verificación automática del repositorio (por ejemplo, cada semana, un 10 %).", enlace: { texto: "Verificación", href: `/c/${cliente}/equipos/${e.id}/copias?verificacion=${encodeURIComponent(rc.repo.id)}` } };
    case "programar_prueba":
      return {
        texto: e.resumen?.admite?.includes("prueba_auto")
          ? "Programa una prueba de restauración cada mes."
          : `Prueba la restauración cada mes («Probar la restauración» en ${e.nombre}). Para programarla sola, actualiza su agente.`,
        enlace: e.resumen?.admite?.includes("prueba_auto") ? { texto: "Prueba de restauración", href: `/c/${cliente}/equipos/${e.id}/copias?prueba=${encodeURIComponent(rc.repo.id)}` } : { texto: "Ver el equipo", href: `/c/${cliente}/equipos/${e.id}` },
      };
    case "revisar_verificacion":
      return { texto: rc.entrada.verificacion.fallo ? "La última verificación encontró errores o no se pudo hacer: revísala." : "No hay una verificación correcta reciente: verifica ahora.", enlace: { texto: "Ver el repositorio", href: repo } };
    case "revisar_prueba":
      return {
        texto: rc.entrada.prueba_restauracion.fallo ? "La última prueba de restauración falló: revísala." : `No hay una prueba de restauración correcta en los últimos ${DIAS_PRUEBA_REGLA} días: prueba ahora.`,
        enlace: { texto: "Ver el repositorio", href: repo },
      };
    case "revisar_destino": {
      const x = rc.pasos.find((y) => y.verificacion_mal);
      return { texto: `${x ? `«${x.nombre}»` : "Un destino"} encontró archivos dañados al comprobarse: revísalo.`, enlace: x?.equipoId ? { texto: `Revisar «${x.nombre}»`, href: `/c/${cliente}/equipos/${x.equipoId}` } : undefined };
    }
    default:
      return null;
  }
}

/** Los avisos, en frase. */
export const TEXTO_AVISO: Record<string, string> = {
  mismo_equipo: "Dos soportes están en el mismo equipo: un fallo del equipo, un robo o un incendio se los lleva a la vez.",
  inmutable_local: "Lo que no se puede borrar está todo en la oficina: no protege de un incendio o un robo.",
};

/** «Cumple la regla 3-2-1-1-0», «Le falta: 1 fuera de la oficina y 0 errores», «Dejó de cumplir: …». */
export function fraseRegla(r: Regla321): string {
  if (r.cumple) return "Cumple la regla 3-2-1-1-0.";
  const faltan = r.partes.filter((p) => !p.cumple_config).map((p) => PARTES[p.id].titulo);
  const atrasadas = r.partes.filter((p) => !p.cumple && p.cumple_config).map((p) => PARTES[p.id].titulo);
  if (r.dejo_de_cumplir) return `Dejó de cumplir: ${lista(atrasadas)} (algo no está al día).`;
  return `Le falta: ${lista(faltan)}.${atrasadas.length ? ` Y no está al día: ${lista(atrasadas)}.` : ""}`;
}

/** Tarea 8e: el entorno de un equipo en palabras («máquina virtual KVM», «contenedor LXC»), o null. */
export function textoEntorno(e: Pick<Equipo, "resumen">): string | null {
  const en = e.resumen?.entorno;
  if (!en) return null;
  const V: Record<string, string> = { kvm: "KVM (Proxmox, QEMU)", vmware: "VMware", hyperv: "Hyper-V", virtualbox: "VirtualBox", xen: "Xen", otra: "otra" };
  const C: Record<string, string> = { lxc: "LXC", docker: "Docker", podman: "Podman", wsl: "WSL", otro: "otro" };
  const partes = [en.contenedor ? `un contenedor ${C[en.contenedor] ?? en.contenedor}` : null, en.virtual ? `una máquina virtual ${V[en.virtual] ?? en.virtual}` : null].filter(Boolean);
  return partes.length ? `En ${partes.join(" dentro de ")}` : null;
}
