// Copias en cadena (tarea 7, parte B; docs/copias-en-cadena.md): la línea de
// cada copia («Documentos (RECEPCION) → Almacén · Disco D → después → espejo
// Disco E → copia derivada a B2»), las cadenas «después de la anterior» y la
// recomendación de tener un destino fuera del alcance de la retención.
//
// Lo que lee la tarea 8 (3-2-1-1-0): `lineaCadena` devuelve los pasos con lo
// que se sabe de cada uno (`clase`, `destinoId`, `tipoDestino`, `inmutable`,
// `fueraRetencion`, `despues`). Sin dependencias de Svelte: lo prueban los
// vectores (scripts/vectores-cadenas.ts).
import type { CopiaConfig, CuandoDerivada, DerivadaResumen, DestinoCatalogo, Equipo, FiltroVersiones, RepositorioResumen } from "./tipos";
import { destinoDe } from "./repo";
import { almacenDe } from "./retencion";
import { almacenesDe, claveNube, destinosDelCliente, nombreZonaPorDefecto, PRINCIPAL, TEXTO_TIPO, zonaDeDestino, zonasDe, type DestinoVista } from "./destinos";
import { etiquetaCarpeta, nombreTipoNube, TIPOS_NUBE } from "./espejo";
import { horarioEnFrase } from "./formato";

/** Lo nuevo de la parte B que anuncia el agente en `resumen.admite`. */
export const ADMITE = {
  /** `config.copias[].tras` e `informe.cadenas[]`. */
  cadenas: "cadenas",
  /** `cambiar_derivada` / `quitar_derivada`. */
  derivadas: "derivadas",
  /** `filtro` con carpetas y fechas (en `copiar_historial` y en las derivadas). */
  filtros: "filtros",
  /** `conectar_nube` fuera de un almacén y destinos «nube» en las derivadas. */
  nubeEquipo: "nube_equipo",
  /** Destinos del espejo con `zona` y de tipo «zona». */
  espejoZonas: "espejo_zonas",
  /** Tarea 4a completa: `crear_repositorio` en una nube conectada en el equipo (copiar las carpetas directamente). */
  repoEnNube: "repo_en_nube",
} as const;
export const admite = (e: Pick<Equipo, "resumen"> | null | undefined, que: string) => !!e?.resumen?.admite?.includes(que);

type CopiaCadena = Pick<CopiaConfig, "id" | "nombre"> & { tras?: string | null };

/** Lo que falla en las cadenas de una configuración (como lo comprueba el agente), o null. */
export function errorCadenas(copias: CopiaCadena[]): string | null {
  for (const k of copias) {
    if (!k.tras) continue;
    if (k.tras === k.id) return `«${k.nombre}» no puede ir después de sí misma.`;
    if (!copias.some((x) => x.id === k.tras)) return `«${k.nombre}» va después de una copia que ya no está: elige otra o ponle horario.`;
    let actual: string | null | undefined = k.tras;
    for (let i = 0; i <= copias.length && actual; i++) {
      const sig: string | null | undefined = copias.find((x) => x.id === actual)?.tras;
      if (sig === k.id) return `La cadena de «${k.nombre}» se cierra en un círculo: alguna copia tiene que empezar con su horario.`;
      actual = sig;
    }
  }
  return null;
}

/** Las copias que pueden ir antes de `k` (todas menos ella y las que ya van detrás de ella). */
export function posiblesAnteriores<T extends CopiaCadena>(copias: T[], k: CopiaCadena): T[] {
  const detras = new Set<string>([k.id]);
  for (let cambio = true; cambio; ) {
    cambio = false;
    for (const x of copias)
      if (x.tras && detras.has(x.tras) && !detras.has(x.id)) {
        detras.add(x.id);
        cambio = true;
      }
  }
  return copias.filter((x) => !detras.has(x.id));
}

/** Mueve la copia `i` una posición arriba (-1) o abajo (+1). Devuelve una lista nueva. */
export function mover<T>(lista: T[], i: number, paso: -1 | 1): T[] {
  const j = i + paso;
  if (i < 0 || i >= lista.length || j < 0 || j >= lista.length) return lista;
  const l = [...lista];
  [l[i], l[j]] = [l[j], l[i]];
  return l;
}

/** Lleva el elemento de la posición `de` a la posición `a` (arrastrar). Devuelve una lista nueva. */
export function moverA<T>(lista: T[], de: number, a: number): T[] {
  if (de === a || de < 0 || a < 0 || de >= lista.length || a >= lista.length) return lista;
  const l = [...lista];
  const [x] = l.splice(de, 1);
  l.splice(a, 0, x);
  return l;
}

/** ¿Va `copias[i]` «después de la anterior» (la que tiene justo encima)? */
export const despuesDeLaAnterior = (copias: CopiaCadena[], i: number) => i > 0 && !!copias[i].tras && copias[i].tras === copias[i - 1]?.id;

/**
 * Después de ordenar (editor de copias, docs/editor-de-copias.md): «después
 * de la anterior» sigue a la de encima.
 * - La que iba después de la que tenía justo encima pasa a ir después de la
 *   que tiene encima ahora.
 * - La primera no puede ir después de otra: pierde `tras` (la página le pone
 *   horario y lo dice). Van en `aHorario`.
 * - Una que iba después de otra que no era la de encima (configuraciones
 *   anteriores) se queda igual.
 */
export function reenlazar<T extends CopiaCadena>(antes: T[], despues: T[]): { copias: T[]; aHorario: string[] } {
  const encimaAntes = new Map(antes.map((k, i) => [k.id, i > 0 ? antes[i - 1].id : null]));
  const aHorario: string[] = [];
  const copias = despues.map((k, i) => {
    if (!k.tras) return k;
    if (i === 0) {
      aHorario.push(k.id);
      return { ...k, tras: null };
    }
    const encima = despues[i - 1].id;
    if (k.tras === encimaAntes.get(k.id) && k.tras !== encima) return { ...k, tras: encima };
    return k;
  });
  return { copias, aHorario };
}

/** «Después de cada copia», «Cada día a las 21:00» o el horario en frase. */
export function cuandoEnFrase(c: CuandoDerivada | null | undefined): string {
  if (!c) return "";
  if (c.tras_copia) return c.min_minutos ? `Después de cada copia (como mucho cada ${c.min_minutos} min)` : "Después de cada copia";
  if (c.horario) return horarioEnFrase(c.horario);
  return c.hora ? `Cada día a las ${c.hora}` : "";
}

/** «de PC-ANA, con la etiqueta diaria, de los últimos 90 días» (vacío: todas). */
export function filtroEnFrase(f: FiltroVersiones | null | undefined): string {
  if (!f) return "";
  const p: string[] = [];
  if (f.equipos?.length) p.push(`de ${f.equipos.join(", ")}`);
  if (f.etiquetas?.length) p.push(`con la etiqueta ${f.etiquetas.join(" o ")}`);
  const n = Array.isArray(f.carpetas) ? f.carpetas.length : (f.carpetas ?? 0);
  if (n) p.push(n === 1 ? "de 1 carpeta" : `de ${n} carpetas`);
  if (f.desde) p.push(`desde el ${f.desde}`);
  if (f.ultimos_dias) p.push(`de los últimos ${f.ultimos_dias} días`);
  return p.join(", ");
}

/** El filtro para la orden: sin listas vacías ni campos sin valor (null: todas). */
export function filtroParaOrden(f: { equipos?: string; etiquetas?: string; carpetas?: string; desde?: string; ultimos_dias?: number | string | null }): FiltroVersiones | null {
  const lista = (t?: string) =>
    (t ?? "")
      .split(/[\n,]/)
      .map((x) => x.trim())
      .filter(Boolean);
  const o: FiltroVersiones = {};
  const eq = lista(f.equipos);
  const et = lista(f.etiquetas);
  const ca = (f.carpetas ?? "")
    .split(/\r?\n/)
    .map((x) => x.trim())
    .filter(Boolean);
  if (eq.length) o.equipos = eq;
  if (et.length) o.etiquetas = et;
  if (ca.length) o.carpetas = ca;
  if (f.desde && /^\d{4}-\d{2}-\d{2}$/.test(f.desde)) o.desde = f.desde;
  const n = Number(f.ultimos_dias);
  if (f.ultimos_dias !== "" && f.ultimos_dias != null && Number.isInteger(n) && n > 0) o.ultimos_dias = n;
  return Object.keys(o).length ? o : null;
}

/** Lo que falla en un filtro escrito a mano (o null). */
export function errorFiltro(f: { desde?: string; ultimos_dias?: number | string | null }): string | null {
  if (f.desde && !/^\d{4}-\d{2}-\d{2}$/.test(f.desde)) return "La fecha «desde» no es válida.";
  if (f.ultimos_dias !== "" && f.ultimos_dias != null) {
    const n = Number(f.ultimos_dias);
    if (!Number.isInteger(n) || n < 1 || n > 3650) return "«Los últimos días» van de 1 a 3650.";
  }
  return null;
}

/** Un paso de la cadena de una copia. */
export interface PasoCadena {
  /** «origen» (las carpetas del equipo), «copia» (un repositorio al que copia una copia), «espejo» o «derivada». */
  clase: "origen" | "copia" | "espejo" | "derivada";
  /** Dónde: «Almacén ALMACEN-01 · Disco D», «Dropbox Oficina»… */
  texto: string;
  /** Qué es: «espejo, sin retención», «copia derivada, otra contraseña»… */
  detalle: string;
  /** Empieza «después de la anterior» (o de cada copia). */
  despues: boolean;
  /** Sangría (0: la copia; 1: lo que cuelga de ella…). */
  nivel: number;
  /** Para la tarea 8: el id del destino (o de la zona: `zona:<almacén>:<zona>`), si se sabe. */
  destinoId?: string;
  /** Para la tarea 8: «zona», «rest», «b2», «s3», «local», «nube», «carpeta» (espejo a una carpeta del almacén)… */
  tipoDestino?: string;
  /** Inmutable (bloqueo de objetos o solo añadir); null: no se sabe. */
  inmutable?: boolean | null;
  /** Aviso: no es inmutable (Dropbox, Drive, SMB, SFTP, WebDAV). */
  noInmutable?: string;
  /** Queda fuera del alcance de la retención del original (un espejo sin retención o una copia con la suya). */
  fueraRetencion: boolean;
}

const NO_INMUTABLE = (n: string) => `${n} no es inmutable: un ransomware con acceso a la cuenta podría borrar lo de allí.`;

/** El tipo de nube de un nombre conectado en un equipo («dropbox»…), si se sabe. */
function tipoNube(e: Equipo | undefined, nombre: string | null | undefined): string | undefined {
  const lista = [...(e?.resumen?.nubes ?? []), ...(e?.resumen?.guarda_copias?.nubes ?? [])];
  return lista.find((n) => n.nombre === nombre)?.tipo;
}

/** Los pasos de una copia derivada (o de la copia externa de siempre) de un repositorio. */
function pasosDerivadas(e: Equipo, r: RepositorioResumen, nivel: number): PasoCadena[] {
  const pasos: PasoCadena[] = [];
  const destinos = e.resumen?.destinos ?? [];
  if (r.externa) {
    const d = destinos.find((x) => x.id === r.externa!.destino_id) ?? destinos.find((x) => x.nombre === r.externa!.destino);
    const inmutable = r.externa.bloqueo_dias ? true : r.externa.solo_anadir ? true : null;
    pasos.push({
      clase: "derivada",
      texto: r.externa.destino,
      detalle: `copia externa · cada día a las ${r.externa.hora}`,
      despues: false,
      nivel,
      destinoId: d?.id ?? r.externa.destino_id ?? undefined,
      tipoDestino: d?.tipo,
      inmutable,
      fueraRetencion: true,
    });
  }
  for (const x of r.derivadas ?? []) {
    const d = destinos.find((y) => y.id === x.destino_id);
    const tipo = d?.tipo === "nube" ? tipoNube(e, d.nube) : undefined;
    const filtro = filtroEnFrase(x.filtro);
    const inmutable = x.bloqueo_dias ? true : x.solo_anadir ? true : tipo ? (TIPOS_NUBE[tipo]?.inmutable ?? null) || null : null;
    pasos.push({
      clase: "derivada",
      texto: x.destino ?? "otro destino",
      detalle: ["copia derivada", cuandoEnFrase(x.cuando).toLowerCase(), filtro ? `solo las versiones ${filtro}` : ""].filter(Boolean).join(" · "),
      despues: !!x.cuando?.tras_copia,
      nivel,
      destinoId: x.destino_id ?? undefined,
      tipoDestino: d?.tipo,
      inmutable,
      noInmutable: tipo && TIPOS_NUBE[tipo] && !TIPOS_NUBE[tipo].inmutable ? NO_INMUTABLE(TIPOS_NUBE[tipo].nombre) : undefined,
      fueraRetencion: true,
    });
  }
  return pasos;
}

/** Los pasos «espejo» del almacén donde está un repositorio (los de su zona que lo copian). */
function pasosEspejo(e: Equipo, r: RepositorioResumen, equipos: Equipo[], nivel: number): PasoCadena[] {
  const d = destinoDe(e.resumen?.destinos, r);
  const en = almacenDe(r, d, equipos);
  if (!en) return [];
  const zona = zonaDeDestino(d, equipos)?.id ?? PRINCIPAL;
  const nombre = en.carpeta === "." || !en.carpeta ? en.usuario : `${en.usuario}/${en.carpeta}`;
  const zonas = zonasDe(en.almacen);
  const espejo = en.almacen.resumen?.guarda_copias?.espejo;
  return (espejo?.destinos ?? [])
    .filter((x) => (x.zona ?? PRINCIPAL) === zona && (!Array.isArray(x.repos) || x.repos.includes(nombre) || x.repos.includes(en.usuario)))
    .map<PasoCadena>((x) => {
      const tipo = x.tipo === "nube" ? tipoNube(en.almacen, x.nube) : undefined;
      const zonaDestino = x.tipo === "zona" ? zonas.find((z) => z.id === x.carpeta) : undefined;
      const texto = x.tipo === "nube" ? (x.nube ?? "Nube") : zonaDestino ? nombreZonaPorDefecto(zonaDestino) : `Almacén ${en.almacen.nombre} · ${etiquetaCarpeta(x.carpeta)}`;
      const retencion = !!x.retencion_dias && !x.bloqueo;
      return {
        clase: "espejo",
        texto,
        detalle: ["espejo", retencion ? `con retención (${x.retencion_dias} días)` : "sin retención", Array.isArray(x.repos) ? "" : "todo el almacén"].filter(Boolean).join(" · "),
        despues: !!x.tras_copia,
        nivel,
        destinoId: zonaDestino ? `zona:${en.almacen.id}:${zonaDestino.id}` : undefined,
        tipoDestino: x.tipo === "nube" ? "nube" : x.tipo,
        inmutable: x.bloqueo ? true : tipo ? (TIPOS_NUBE[tipo]?.inmutable ?? null) || null : null,
        noInmutable: tipo && TIPOS_NUBE[tipo] && !TIPOS_NUBE[tipo].inmutable ? NO_INMUTABLE(TIPOS_NUBE[tipo].nombre) : undefined,
        fueraRetencion: !retencion,
      };
    });
}

/**
 * La cadena de la copia `copiaId` de `e`: sus carpetas, el repositorio al que
 * copia, lo que cuelga de él (copia externa, derivadas y espejos de su almacén)
 * y las copias que van «después de» ella (con lo suyo), en orden.
 */
export function lineaCadena(e: Equipo, copiaId: string, equipos: Equipo[]): PasoCadena[] {
  const copias = e.resumen?.copias ?? [];
  const k0 = copias.find((k) => k.id === copiaId);
  if (!k0) return [];
  const pasos: PasoCadena[] = [{ clase: "origen", texto: `${k0.nombre} (${e.nombre})`, detalle: "las carpetas del equipo", despues: false, nivel: 0, fueraRetencion: false }];
  const vistos = new Set<string>();
  const reposVistos = new Set<string>();
  const visitar = (k: (typeof copias)[number], despues: boolean, nivel: number) => {
    if (vistos.has(k.id) || vistos.size > 20) return;
    vistos.add(k.id);
    const r = e.resumen?.repositorios?.find((x) => x.id === k.repo);
    const d = r ? destinoDe(e.resumen?.destinos, r) : undefined;
    const z = zonaDeDestino(d, equipos);
    pasos.push({
      clase: "copia",
      texto: z ? nombreZonaPorDefecto(z) : (d?.nombre ?? r?.destino ?? "su destino"),
      detalle: `${nivel ? `«${k.nombre}», ` : ""}repositorio «${r?.nombre ?? k.repo}»`,
      despues,
      nivel,
      destinoId: z ? `zona:${z.almacen.id}:${z.id}` : d?.id,
      tipoDestino: z ? "zona" : d?.tipo,
      inmutable: r?.solo_anadir ? true : null,
      fueraRetencion: nivel > 0,
    });
    if (r && !reposVistos.has(r.id)) {
      reposVistos.add(r.id);
      pasos.push(...pasosEspejo(e, r, equipos, nivel + 1), ...pasosDerivadas(e, r, nivel + 1));
    }
    for (const sig of copias.filter((x) => x.tras === k.id)) visitar(sig, true, nivel + 1);
  };
  visitar(k0, false, 0);
  return pasos;
}

/**
 * Lo que cuelga de un repositorio, para la tarjeta de una copia (editor de
 * copias): su destino («copia») y, debajo (nivel 1), los espejos de su almacén,
 * la copia externa y las derivadas. Vale también para una copia aún sin enviar
 * (el repositorio ya existe en el resumen del equipo).
 */
export function pasosDelRepo(e: Equipo, repoId: string, equipos: Equipo[]): PasoCadena[] {
  const r = e.resumen?.repositorios?.find((x) => x.id === repoId);
  if (!r) return [];
  const d = destinoDe(e.resumen?.destinos, r);
  const z = zonaDeDestino(d, equipos);
  return [
    {
      clase: "copia",
      texto: z ? nombreZonaPorDefecto(z) : (d?.nombre ?? r.destino ?? "su destino"),
      detalle: `repositorio «${r.nombre}»`,
      despues: false,
      nivel: 0,
      destinoId: z ? `zona:${z.almacen.id}:${z.id}` : d?.id,
      tipoDestino: z ? "zona" : d?.tipo,
      inmutable: r.solo_anadir ? true : null,
      fueraRetencion: false,
    },
    ...pasosEspejo(e, r, equipos, 1),
    ...pasosDerivadas(e, r, 1),
  ];
}

/** La cadena en una línea: «Documentos (RECEPCION) → Almacén · Disco D → después → espejo …». */
export function lineaEnTexto(pasos: PasoCadena[]): string {
  return pasos.map((p, i) => (i === 0 ? p.texto : `${p.despues ? "después → " : ""}${p.texto}${p.clase === "espejo" ? " (espejo)" : p.clase === "derivada" ? " (copia derivada)" : ""}`)).join(" → ");
}

/**
 * Seguridad de la tarea 7: ¿hace falta recomendar un destino fuera del alcance
 * de la retención? Cuando la copia tiene más de un destino y ninguno queda
 * fuera (todos los espejos siguen a la retención del original). Nunca obliga.
 */
export function recomendarFueraRetencion(pasos: PasoCadena[]): boolean {
  const destinos = pasos.filter((p) => p.clase !== "origen" && p.clase !== "copia");
  return destinos.length > 0 && !destinos.some((p) => p.fueraRetencion || p.inmutable);
}

export const TEXTO_FUERA_RETENCION =
  "Todos los destinos siguen a la retención del original. Recomendado: al menos uno fuera de su alcance (un espejo sin retención, una copia derivada con su propia retención o una nube con bloqueo de objetos): si un equipo comprometido llenara el repositorio de versiones basura, no desplazaría las buenas en todos.";

/** Los almacenes del cliente con espejo por zonas (para ofrecer pasos «espejo»). */
export const almacenesConZonas = (equipos: Equipo[]) => almacenesDe(equipos).filter((a) => admite(a, ADMITE.espejoZonas));

/** Los destinos de un equipo a los que puede ir una derivada de `r` (todos menos el suyo). */
export function destinosParaDerivada(e: Equipo, r: RepositorioResumen) {
  const propio = destinoDe(e.resumen?.destinos, r)?.id;
  return (e.resumen?.destinos ?? []).filter((d) => d.id !== propio);
}

/** Un id libre para una derivada nueva de un repositorio (`d1`, `d2`…). */
export function idDerivadaNueva(r: Pick<RepositorioResumen, "derivadas">): string {
  const usados = new Set((r.derivadas ?? []).map((d) => d.id));
  for (let i = 1; ; i++) if (!usados.has(`d${i}`)) return `d${i}`;
}

/** El nombre de un repositorio en el espejo de su almacén, con su zona (para un paso «espejo»). */
export function repoEnAlmacen(e: Equipo, r: RepositorioResumen, equipos: Equipo[]): { almacen: Equipo; nombre: string; zona: string } | null {
  const d = destinoDe(e.resumen?.destinos, r);
  const en = almacenDe(r, d, equipos);
  if (!en) return null;
  return { almacen: en.almacen, nombre: en.carpeta === "." || !en.carpeta ? en.usuario : `${en.usuario}/${en.carpeta}`, zona: zonaDeDestino(d, equipos)?.id ?? PRINCIPAL };
}

/** Tarea 7e: un destino del espejo de un solo repositorio es un paso de la cadena de esa copia (o null). */
export function pasoDeEspejo(d: { repos?: string[] | null; zona?: string | null }, almacen: Equipo, equipos: Equipo[]): { equipo: Equipo; repo: RepositorioResumen } | null {
  if (!Array.isArray(d.repos) || d.repos.length !== 1) return null;
  for (const e of equipos)
    for (const r of e.resumen?.repositorios ?? []) {
      const x = repoEnAlmacen(e, r, equipos);
      if (x && x.almacen.id === almacen.id && x.nombre === d.repos[0] && x.zona === (d.zona ?? PRINCIPAL)) return { equipo: e, repo: r };
    }
  return null;
}

/** Las derivadas de un repositorio, para enseñarlas (la externa de siempre primero). */
export function derivadasDe(r: RepositorioResumen): (DerivadaResumen & { externa?: boolean })[] {
  const l: (DerivadaResumen & { externa?: boolean })[] = [];
  if (r.externa) l.push({ id: "externa", externa: true, destino: r.externa.destino, destino_id: r.externa.destino_id, cuando: { hora: r.externa.hora }, bloqueo_dias: r.externa.bloqueo_dias, solo_anadir: r.externa.solo_anadir, con_retencion: r.externa.con_retencion });
  return l.concat(r.derivadas ?? []);
}

// --- ¿Para qué sirve cada destino desde una copia? (docs/editor-de-copias.md) ---
// «Añadir paso» enseña todos los destinos del cliente con lo que se puede hacer
// con cada uno desde esta copia, y por qué no cuando no se puede. La página de
// un destino puede usar lo mismo para «Usar en una copia».

/** Lo que se puede hacer con un destino desde una copia. */
export type UsoPaso = "copia" | "espejo" | "derivada";
export interface Uso {
  ok: boolean;
  /** Por qué no (o una nota corta si sí). */
  motivo?: string;
  /** Lo que lo haría posible: conectar la nube en un equipo, actualizar su agente o usar otro tipo de paso. */
  accion?: { tipo: "conectar_nube"; equipo: Equipo; nube: string; tipoNube: string; texto?: string } | { tipo: "actualizar"; equipo: Equipo } | { tipo: "otro_paso"; uso: UsoPaso };
}

/** Todos los destinos del cliente, más las nubes conectadas en equipos que no son almacenes (para las derivadas). */
export function destinosParaPasos(equipos: Equipo[], catalogo: DestinoCatalogo[] = []): DestinoVista[] {
  const l = destinosDelCliente(equipos, catalogo);
  const almacenes = new Set(almacenesDe(equipos).map((a) => a.id));
  for (const e of equipos) {
    if (almacenes.has(e.id)) continue;
    for (const n of e.resumen?.nubes ?? []) {
      // Si ya la usa un destino del equipo, basta con ese (no dos veces la misma nube).
      if (e.resumen?.destinos?.some((x) => x.tipo === "nube" && x.nube === n.nombre)) continue;
      const clave = claveNube(e.id, n.nombre);
      if (!l.some((x) => x.clave === clave)) l.push({ clave, nombre: n.nombre, nombrePorDefecto: n.nombre, renombrado: false, clase: "nube", tipo: "nube", donde: null, nube: { equipo: e, nombre: n.nombre, tipo: n.tipo }, ids: [], equipos: [e.nombre] });
    }
  }
  return l;
}

/** «Dropbox · conectada en ALMACEN-SUR», «Backblaze B2 · copias-sur», «Zona de un almacén». */
export function detalleDestino(d: DestinoVista): string {
  if (d.nube) return `${nombreTipoNube(d.nube.tipo)} · conectada en ${d.nube.equipo.nombre}`;
  if (d.zona) return d.zona.principal ? `Almacén ${d.zona.almacen.nombre}` : `Otra zona de ${d.zona.almacen.nombre}`;
  return [TEXTO_TIPO[d.tipo] ?? d.tipo, d.donde].filter(Boolean).join(" · ");
}

/** ¿Está la nube `nombre` conectada en `equipo`? (para sus copias o, si es un almacén, para su espejo) */
export const nubeEn = (equipo: Equipo, nombre: string) => [...(equipo.resumen?.nubes ?? []), ...(equipo.resumen?.guarda_copias?.nubes ?? [])].some((n) => n.nombre === nombre);

/**
 * Tarea 4a: ¿puede `equipo` copiar sus carpetas directamente a la nube `n`
 * (un repositorio allí)? Si no, por qué y qué hacer: actualizar su agente,
 * conectarla también en él (Dropbox, desde la consola) o, para B2 y S3, usarlos
 * como un destino de siempre (con sus datos).
 */
export function usoNubeDirecta(n: { equipo: Equipo; nombre: string; tipo: string }, equipo: Equipo): Uso {
  if (!admite(equipo, ADMITE.repoEnNube)) return { ok: false, motivo: `Actualiza el agente de ${equipo.nombre} para copiar directo a una nube`, accion: { tipo: "actualizar", equipo } };
  if (n.equipo.id === equipo.id || nubeEn(equipo, n.nombre)) return { ok: true, motivo: TIPOS_NUBE[n.tipo] && !TIPOS_NUBE[n.tipo].inmutable ? "No es inmutable" : undefined };
  const tipo = nombreTipoNube(n.tipo);
  if (n.tipo === "dropbox") return { ok: false, motivo: `Hace falta también en ${equipo.nombre}`, accion: { tipo: "conectar_nube", equipo, nube: n.nombre, tipoNube: n.tipo, texto: `Conectar ${tipo} también en ${equipo.nombre}` } };
  if (n.tipo === "drive") return { ok: false, motivo: `Conéctala en ${equipo.nombre} desde el propio equipo («resguardo-agente nube conectar drive»)` };
  if (n.tipo === "b2" || n.tipo === "s3") return { ok: false, motivo: `Desde ${equipo.nombre}: «Un destino nuevo…» de tipo ${tipo}, con sus datos` };
  return { ok: false, motivo: `En ${equipo.nombre}, todavía no se conecta desde la consola` };
}

/**
 * Qué se puede hacer con el destino `d` desde el repositorio `repo` de `equipo`:
 * - «copia» (carpetas directas): a una nube, si está conectada en el equipo y su agente lo admite (4a, `usoNubeDirecta`).
 * - «espejo»: lo hace el almacén donde está el repositorio, a otra de sus zonas o a una nube conectada en él.
 * - «derivada»: lo hace el equipo dueño, que necesita el destino (o la nube conectada) en él.
 */
export function usosPosibles(d: DestinoVista, equipo: Equipo, repo: RepositorioResumen | null, equipos: Equipo[]): Record<UsoPaso, Uso> {
  const propio = repo ? destinoDe(equipo.resumen?.destinos, repo) : undefined;
  const zonaPropia = propio ? zonaDeDestino(propio, equipos) : null;
  const esElSuyo = !!repo && ((!!zonaPropia && d.zona?.almacen.id === zonaPropia.almacen.id && d.zona.id === zonaPropia.id) || (!!propio && d.ids.includes(propio.id)));
  const yaEsta: Uso = { ok: false, motivo: "Ya guarda aquí" };

  // Copia nueva de carpetas.
  // Una nube: la conectada en un equipo o la de un destino «nube» de un equipo (el suyo primero).
  let nube = d.nube;
  if (!nube && d.tipo === "nube") {
    const nombre = d.destino?.nube ?? d.nombre;
    const duenio = [equipo, ...equipos].find((e) => e.resumen?.destinos?.some((x) => d.ids.includes(x.id)));
    if (duenio) nube = { equipo: duenio, nombre, tipo: tipoNube(duenio, nombre) ?? "" };
  }
  const copia: Uso = nube ? usoNubeDirecta(nube, equipo) : { ok: true };

  // Espejo: el almacén donde está el repositorio.
  let espejo: Uso;
  const en = repo ? repoEnAlmacen(equipo, repo, equipos) : null;
  if (esElSuyo) espejo = yaEsta;
  else if (!en) espejo = { ok: false, motivo: "Solo si la copia guarda en un almacén" };
  else if (!admite(en.almacen, ADMITE.espejoZonas)) espejo = { ok: false, motivo: `Actualiza el agente de ${en.almacen.nombre}`, accion: { tipo: "actualizar", equipo: en.almacen } };
  else if (d.zona) espejo = d.zona.almacen.id === en.almacen.id ? { ok: true } : { ok: false, motivo: `Es de ${d.zona.almacen.nombre}; el espejo lo hace ${en.almacen.nombre}` };
  else if (d.nube)
    espejo =
      d.nube.equipo.id === en.almacen.id
        ? { ok: true }
        : { ok: false, motivo: `Hace falta también en ${en.almacen.nombre}`, accion: { tipo: "conectar_nube", equipo: en.almacen, nube: d.nube.nombre, tipoNube: d.nube.tipo } };
  else espejo = { ok: false, motivo: `El espejo va a otra zona o a una nube de ${en.almacen.nombre}` };

  // Repositorio nuevo a partir de esta: el equipo dueño.
  let derivada: Uso;
  const tiene = d.ids.some((id) => equipo.resumen?.destinos?.some((x) => x.id === id));
  if (esElSuyo) derivada = yaEsta;
  else if (!admite(equipo, ADMITE.derivadas)) derivada = { ok: false, motivo: `Actualiza el agente de ${equipo.nombre}`, accion: { tipo: "actualizar", equipo } };
  else if (d.nube) {
    const aqui = d.nube.equipo.id === equipo.id || (equipo.resumen?.nubes ?? []).some((n) => n.nombre === d.nube!.nombre);
    derivada = aqui
      ? { ok: true }
      : !admite(equipo, ADMITE.nubeEquipo)
        ? { ok: false, motivo: `Actualiza el agente de ${equipo.nombre} para usar nubes`, accion: { tipo: "actualizar", equipo } }
        : d.nube.tipo === "dropbox"
          ? { ok: false, motivo: `Hace falta también en ${equipo.nombre}`, accion: { tipo: "conectar_nube", equipo, nube: d.nube.nombre, tipoNube: d.nube.tipo } }
          : // B2, S3, SFTP… conectadas por rclone en el almacén: desde el equipo, como un destino nuevo con sus datos.
            { ok: false, motivo: `Desde ${equipo.nombre}: «Un destino nuevo…» con sus datos` };
  } else if (tiene) derivada = { ok: true };
  else if (d.zona) derivada = { ok: false, motivo: `${equipo.nombre} aún no entra en esta zona: mejor un espejo` };
  else if (["b2", "s3", "rest"].includes(d.tipo)) derivada = { ok: true, motivo: "Pide sus credenciales" };
  else derivada = { ok: false, motivo: `No es de ${equipo.nombre}` };

  return { copia, espejo, derivada };
}
