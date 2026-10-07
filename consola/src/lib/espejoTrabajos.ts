// Trabajos de espejo (plan 0.7.26, bloque 4; docs/espejo.md «Trabajos de espejo»).
//
// Un trabajo es un espejo con todo lo suyo: quién lo hace (el almacén o el propio
// equipo), qué repositorios, adónde, cuándo, su retención, su freno, la
// verificación y la velocidad. Aquí, sin Svelte (para las pruebas,
// scripts/vectores-espejo-trabajos.ts): lo que se manda, lo que se comprueba (lo
// mismo que el agente, con vectores compartidos), qué reduce la protección (la
// espera) y los textos cortos de la interfaz.
import { horarioEnFrase, lista } from "./formato";
import { repoEnAlmacen } from "./cadenas";
import { destinoDe } from "./repo";
import { PRINCIPAL } from "./destinos";
import { claveDestinoTrabajo, FRENO_DEFECTO } from "./espejoReduce";
import type { AdondeEspejo, CuandoEspejo, Equipo, FrenoEspejo, Horario, RepositorioResumen, TrabajoEspejo, TrabajoEspejoResumen } from "./tipos";

export type { AdondeEspejo, CuandoEspejo, FrenoEspejo, QueEspejo, RetencionEspejo, TrabajoEspejo, TrabajoEspejoResumen } from "./tipos";
export { claveDestinoTrabajo, FRENO_DEFECTO, reduceTrabajos } from "./espejoReduce";

/** El almacén entiende `espejo: { trabajos }`. */
export const ADMITE_TRABAJOS = "espejo_trabajos";
/** El equipo hace espejos de los repositorios de sus discos (`espejo_equipo`). */
export const ADMITE_EQUIPO = "espejo_equipo";
export const admiteTrabajos = (e: Pick<Equipo, "resumen"> | null | undefined) => !!e?.resumen?.admite?.includes(ADMITE_TRABAJOS);
export const admiteEspejoEquipo = (e: Pick<Equipo, "resumen"> | null | undefined) => !!e?.resumen?.admite?.includes(ADMITE_EQUIPO);

export const RETRASO_DIAS_DEFECTO = 30;
export const MAX_TRABAJOS = 50;
export const RETRASO_MAX_MIN = 1440;
export const RETRASO_DIAS_MAX = 3650;

/** Los trabajos del almacén (los del resumen; uno anterior, sin ellos, no tiene). */
export const trabajosDelAlmacen = (e: Pick<Equipo, "resumen"> | null | undefined): TrabajoEspejoResumen[] => e?.resumen?.guarda_copias?.espejo?.trabajos ?? [];
/** Los espejos que hace el propio equipo. */
export const trabajosDelEquipo = (e: Pick<Equipo, "resumen"> | null | undefined): TrabajoEspejoResumen[] => e?.resumen?.espejo_equipo?.trabajos ?? [];

/** Un id nuevo para un trabajo («t» y 10 cifras hexadecimales). */
export function idNuevo(): string {
  const b = new Uint8Array(5);
  globalThis.crypto.getRandomValues(b);
  return `t${[...b].map((x) => x.toString(16).padStart(2, "0")).join("")}`;
}

const tieneHorario = (h: Horario | null | undefined): h is Horario => !!h && (!!h.horas?.length || !!h.reglas?.length);
export const horarioDiario = (hora: string): Horario => ({ dias: [1, 2, 3, 4, 5, 6, 7], horas: [hora] });

/** Un trabajo nuevo con lo de por defecto (después de cada copia, nunca borra, freno al 10 %). */
export function trabajoNuevo(quien: "almacen" | "equipo", orden: number): TrabajoEspejo {
  return {
    id: idNuevo(),
    nombre: "",
    activo: true,
    quien,
    que: { tipo: "todos" },
    adonde: { tipo: "carpeta", carpeta: "" },
    cuando: { horario: horarioDiario("02:00"), tras_copia: true },
    retencion: { modo: "nunca" },
    freno: { ...FRENO_DEFECTO },
    orden,
  };
}

/** Lo que se manda de un trabajo: sus opciones, nunca sus resultados. */
export function paraOrden(t: TrabajoEspejo): TrabajoEspejo {
  const cuando: CuandoEspejo = {};
  if (tieneHorario(t.cuando.horario)) cuando.horario = t.cuando.horario;
  if (t.cuando.tras_copia) cuando.tras_copia = true;
  if (t.cuando.cadena) cuando.cadena = t.cuando.cadena;
  else if (t.cuando.despues) cuando.despues = t.cuando.despues;
  if (t.cuando.retraso_min) cuando.retraso_min = Math.max(0, Math.round(t.cuando.retraso_min));
  const o: TrabajoEspejo = {
    id: t.id,
    nombre: t.nombre.trim(),
    activo: t.activo,
    quien: t.quien,
    que: t.que.tipo === "todos" ? { tipo: "todos" } : t.que.tipo === "equipos" ? { tipo: "equipos", equipos: [...t.que.equipos] } : { tipo: "repos", repos: [...t.que.repos] },
    adonde: t.adonde.tipo === "nube" ? { tipo: "nube", nube: (t.adonde.nube ?? "").trim(), carpeta: t.adonde.carpeta.trim().replace(/^\/+|\/+$/g, "") } : { tipo: t.adonde.tipo, carpeta: t.adonde.carpeta.trim() },
    cuando,
    retencion: t.retencion.modo === "retraso" ? { modo: "retraso", dias: Math.round(t.retencion.dias) } : { modo: t.retencion.modo },
    freno: { pct: Math.round(t.freno.pct), min_archivos: Math.round(t.freno.min_archivos), min_faltan: Math.round(t.freno.min_faltan), accion: t.freno.accion },
    orden: t.orden,
  };
  if (t.zona && t.zona !== PRINCIPAL) o.zona = t.zona;
  if (typeof t.verificar_pct === "number") o.verificar_pct = Math.round(t.verificar_pct);
  if (t.limite_kib && t.limite_kib > 0) o.limite_kib = Math.round(t.limite_kib);
  if (t.bloqueo) o.bloqueo = true;
  if (t.bloqueo_dias && t.bloqueo_dias > 0) o.bloqueo_dias = Math.round(t.bloqueo_dias);
  if (t.que.tipo === "repos" && t.vistos?.length) o.vistos = [...t.vistos];
  return o;
}

/** La orden al almacén con estos trabajos (todos; sin ninguno, quita el espejo). */
export const cuerpoAlmacen = (ts: TrabajoEspejo[]) => (ts.length ? { espejo: { trabajos: ts.map(paraOrden) } } : { espejo: null });
/** La orden al equipo con sus espejos (sin ninguno, los quita). */
export const cuerpoEquipo = (ts: TrabajoEspejo[]) => ({ espejo_equipo: ts.length ? { trabajos: ts.map(paraOrden) } : null });

/** Con el orden de la lista (0, 1, 2…). */
export const renumerar = (ts: TrabajoEspejo[]): TrabajoEspejo[] => ts.map((t, i) => ({ ...t, orden: i }));
/** Sube (-1) o baja (+1) un trabajo en la lista. */
export function mover(ts: TrabajoEspejo[], id: string, delta: -1 | 1): TrabajoEspejo[] {
  const l = [...ts].sort((a, b) => a.orden - b.orden);
  const i = l.findIndex((t) => t.id === id);
  const j = i + delta;
  if (i < 0 || j < 0 || j >= l.length) return renumerar(l);
  [l[i], l[j]] = [l[j], l[i]];
  return renumerar(l);
}
/** Cambia uno (o lo añade al final si no está). */
export function conTrabajo(ts: TrabajoEspejo[], t: TrabajoEspejo): TrabajoEspejo[] {
  const l = ts.some((x) => x.id === t.id) ? ts.map((x) => (x.id === t.id ? t : x)) : [...ts, { ...t, orden: ts.length }];
  return renumerar([...l].sort((a, b) => a.orden - b.orden));
}
/** Sin uno (y sin lo que iba en cadena o después de él: empiezan con su horario o después de cada copia). */
export function sinTrabajo(ts: TrabajoEspejo[], id: string): TrabajoEspejo[] {
  return renumerar(
    ts
      .filter((t) => t.id !== id)
      .map((t) => {
        if (t.cuando.cadena !== id && t.cuando.despues !== id) return t;
        const cuando: CuandoEspejo = { ...t.cuando, cadena: null, despues: null };
        if (!tieneHorario(cuando.horario) && !cuando.tras_copia) cuando.tras_copia = true;
        return { ...t, cuando };
      }),
  );
}

// --- Lo que se comprueba (lo mismo que el agente; vectores compartidos) ---

const repoValido = (r: string) => {
  const p = r.split("/");
  return p.length >= 1 && p.length <= 2 && p.every((x) => x.length >= 1 && x.length <= 100 && !x.startsWith(".") && /^[A-Za-z0-9._-]+$/.test(x));
};
const idRepoValido = (r: string) => /^[A-Za-z0-9_-]{1,64}$/.test(r);
const idValido = (r: string) => /^[A-Za-z0-9_-]{1,40}$/.test(r);
const zonaValida = (z: string) => /^z[0-9a-f]{6}$/.test(z);
export const carpetaRemotaValida = (c: string) => {
  const x = c.trim().replace(/^\/+|\/+$/g, "");
  return !!x && x.length <= 200 && !/[\u0000-\u001f:]/.test(x) && !x.split("/").some((p) => p === ".." || p === "." || !p);
};

/** El nombre que se pone a un trabajo sin nombre (como el agente). */
export function nombrePorDefecto(a: AdondeEspejo): string {
  const n = a.tipo === "nube" && a.nube ? `Espejo a «${a.nube}»` : a.tipo === "zona" ? (a.carpeta === PRINCIPAL ? "Espejo a la zona principal" : `Espejo a la zona ${a.carpeta.trim()}`) : `Espejo a ${a.carpeta.trim()}`;
  // Como mucho 80 letras, como el agente (una carpeta larga: el final).
  const l = [...n];
  return l.length <= 80 ? n : `…${l.slice(-79).join("")}`;
}

/** El error de un trabajo (o null), sin mirar el equipo. `quien`: quién lo va a hacer. */
export function errorTrabajo(t: TrabajoEspejo, quien: "almacen" | "equipo"): string | null {
  if (t.quien !== quien) return quien === "equipo" ? "Ese espejo lo tiene que hacer el almacén." : "Ese espejo lo tiene que hacer el propio equipo.";
  if (!idValido(t.id)) return "Id de espejo no válido.";
  const nombre = t.nombre.trim() || nombrePorDefecto(t.adonde);
  if ([...nombre].length > 80 || /[\u0000-\u001f]/.test(nombre)) return "El nombre: hasta 80 letras.";
  const equipo = quien === "equipo";
  const listaOk = (l: string[], ok: (x: string) => boolean, que: string) => {
    if (!l.length) return `Elige al menos un ${que} (o todos).`;
    if (l.length > 500) return "Demasiados repositorios en un espejo.";
    return l.every((x) => ok(x.trim())) ? null : `${que[0].toUpperCase()}${que.slice(1)} no válido.`;
  };
  if (t.que.tipo === "equipos") {
    if (equipo) return "Un espejo del propio equipo elige repositorios, no equipos.";
    const e = listaOk(t.que.equipos, (x) => repoValido(x) && !x.includes("/"), "equipo");
    if (e) return e;
  } else if (t.que.tipo === "repos") {
    const e = listaOk(t.que.repos, equipo ? idRepoValido : repoValido, "repositorio");
    if (e) return e;
  }
  const zona = t.zona && t.zona !== PRINCIPAL ? t.zona : null;
  if (zona && equipo) return "Un espejo del propio equipo no copia de una zona.";
  if (zona && !zonaValida(zona)) return "Zona de origen no válida.";
  const carpeta = t.adonde.carpeta.trim();
  switch (t.adonde.tipo) {
    case "carpeta":
      if (!carpeta) return "Falta la carpeta.";
      break;
    case "zona":
      if (equipo) return "Un espejo del propio equipo va a una carpeta o a una nube.";
      if (carpeta !== PRINCIPAL && !zonaValida(carpeta)) return "Zona no válida.";
      if (carpeta === (zona ?? PRINCIPAL)) return "No puede copiar una zona en sí misma: elige otra.";
      break;
    case "nube":
      if (!(t.adonde.nube ?? "").trim()) return "Elige la nube.";
      if (!carpetaRemotaValida(carpeta)) return "Carpeta de la nube no válida (por ejemplo, Resguardo/Sur).";
      break;
    default:
      return "Adónde no válido.";
  }
  const c = t.cuando;
  if (c.horario && !tieneHorario(c.horario)) return "El horario está vacío.";
  if (c.cadena && c.despues) return "En cadena o después de otro, no las dos cosas.";
  const otro = c.cadena || c.despues;
  if (otro && (!idValido(otro) || otro === t.id)) return "No puede ir después de sí mismo.";
  if ((c.retraso_min ?? 0) < 0 || (c.retraso_min ?? 0) > RETRASO_MAX_MIN) return "El retraso va de 0 a 1440 minutos (un día).";
  if (!tieneHorario(c.horario) && !c.tras_copia && !c.cadena && !c.despues) return "Di cuándo: con horario, después de cada copia o después de otro espejo.";
  if (t.retencion.modo === "retraso" && (!Number.isInteger(t.retencion.dias) || t.retencion.dias < 1 || t.retencion.dias > RETRASO_DIAS_MAX)) return `El retraso del borrado va de 1 a ${RETRASO_DIAS_MAX} días.`;
  if (!["nunca", "retraso", "igual"].includes(t.retencion.modo)) return "Retención no válida.";
  if (t.bloqueo && t.retencion.modo !== "nunca") return "Con bloqueo de objetos sin plazo no se borra: elige «Nunca borra».";
  const n = t.bloqueo_dias;
  if (n != null && n !== 0) {
    if (!Number.isInteger(n) || n < 1 || n > 36500) return "Los días del bloqueo no son válidos.";
    if (t.retencion.modo === "igual") return `Con bloqueo de objetos de ${n} días no se puede «Igual que el origen».`;
    if (t.retencion.modo === "retraso" && t.retencion.dias <= n) return `Con bloqueo de objetos de ${n} días, el retraso tiene que ser de más de ${n} días.`;
  }
  const f = t.freno;
  if (!Number.isInteger(f.pct) || f.pct < 1 || f.pct > 50) return "El freno va del 1 al 50 % (no se puede apagar).";
  if (!Number.isInteger(f.min_archivos) || f.min_archivos < 0 || f.min_archivos > 10_000_000 || !Number.isInteger(f.min_faltan) || f.min_faltan < 0 || f.min_faltan > 10_000_000) return "Mínimos del freno no válidos.";
  if (f.accion !== "avisar" && f.accion !== "confirmar") return "Acción del freno no válida.";
  if (t.verificar_pct != null && (t.verificar_pct < 0 || t.verificar_pct > 100)) return "La verificación va de 0 a 100 %.";
  return null;
}

/** El error de todos juntos (o null): ids, «después de» a uno que existe, sin círculos; al mismo destino, misma retención. */
export function errorTrabajos(ts: TrabajoEspejo[], quien: "almacen" | "equipo"): string | null {
  if (ts.length > MAX_TRABAJOS) return `Como mucho ${MAX_TRABAJOS} espejos.`;
  for (const t of ts) {
    const e = errorTrabajo(t, quien);
    if (e) return `«${t.nombre.trim() || nombrePorDefecto(t.adonde)}»: ${e}`;
  }
  if (new Set(ts.map((t) => t.id)).size !== ts.length) return "Hay dos espejos con el mismo id.";
  const antes = (x: TrabajoEspejo) => x.cuando.cadena || x.cuando.despues || null;
  for (const t of ts) {
    const nombre = t.nombre.trim() || nombrePorDefecto(t.adonde);
    const o = antes(t);
    if (o && !ts.some((x) => x.id === o)) return `«${nombre}» va después de un espejo que ya no está.`;
    let actual = t.id;
    for (let i = 0; i <= ts.length; i++) {
      const sig = antes(ts.find((x) => x.id === actual) ?? t);
      if (!sig) break;
      if (sig === t.id) return `Los espejos de «${nombre}» se cierran en un círculo: alguno tiene que empezar con su horario.`;
      actual = sig;
    }
    const mismo = ts.find((x) => x.id !== t.id && claveDestinoTrabajo(x) === claveDestinoTrabajo(t) && (JSON.stringify(x.retencion) !== JSON.stringify(t.retencion) || (x.bloqueo_dias ?? null) !== (t.bloqueo_dias ?? null)));
    if (mismo) return `«${nombre}» y «${mismo.nombre.trim() || nombrePorDefecto(mismo.adonde)}» van al mismo destino: tienen que tener la misma retención.`;
  }
  return null;
}

// --- Textos cortos ---

/** «Todos los repositorios», «Los de 2 equipos: Caja, Servidor», «Solo Contabilidad (Caja)». */
export function textoQue(t: Pick<TrabajoEspejo, "que" | "quien">, nombreRepo: (r: string) => string = (r) => r, nombreEquipo: (u: string) => string = (u) => u): string {
  if (t.que.tipo === "todos") return t.quien === "equipo" ? "Todos sus repositorios" : "Todos los repositorios";
  if (t.que.tipo === "equipos") return t.que.equipos.length === 1 ? `Los de ${nombreEquipo(t.que.equipos[0])}` : `Los de ${t.que.equipos.length} equipos: ${lista(t.que.equipos.map(nombreEquipo))}`;
  return t.que.repos.length === 1 ? `Solo ${nombreRepo(t.que.repos[0])}` : `${t.que.repos.length} repositorios: ${lista(t.que.repos.map(nombreRepo))}`;
}

/** «Cada día a las 02:00 · después de cada copia nueva», «En cadena tras «Disco E» (10 min después)». */
export function textoCuando(t: Pick<TrabajoEspejo, "cuando">, nombreDe: (id: string) => string = (id) => id): string {
  const partes: string[] = [];
  const c = t.cuando;
  const retraso = c.retraso_min ? ` (${c.retraso_min} min después)` : "";
  if (c.cadena) partes.push(`En cadena tras «${nombreDe(c.cadena)}»${retraso}`);
  if (c.despues) partes.push(`Después de «${nombreDe(c.despues)}»${retraso}`);
  if (c.tras_copia) partes.push(`Después de cada copia nueva${c.cadena || c.despues ? "" : retraso}`);
  if (tieneHorario(c.horario)) partes.push(horarioEnFrase(c.horario));
  if (!partes.length) return "Sin cuándo";
  const [primero, ...resto] = partes;
  return [primero, ...resto.map((p) => p[0].toLowerCase() + p.slice(1))].join(" · ");
}

/** «Nunca borra», «Borra a los 30 días», «Igual que el origen». */
export function textoRetencion(t: Pick<TrabajoEspejo, "retencion" | "bloqueo" | "bloqueo_dias">): string {
  if (t.bloqueo) return "Nunca borra (bloqueo de objetos)";
  if (t.retencion.modo === "nunca") return "Nunca borra";
  if (t.retencion.modo === "igual") return "Igual que el origen";
  return `Borra lo quitado a los ${t.retencion.dias} días`;
}

/** Aviso de «Igual que el origen». */
export const AVISO_IGUAL = "Si algo borra en el original, aquí también.";

/** «Frena si falta más del 10 % · avisa» (corto). */
export function textoFreno(f: FrenoEspejo): string {
  return `Frena si falta de golpe más del ${f.pct} % (con más de ${f.min_archivos} archivos y ${f.min_faltan} que falten) o un repositorio entero · ${f.accion === "avisar" ? "conserva y avisa" : "pide confirmación"}`;
}

/** ¿Va bien? «ok», «error», «pausado», «nuevo» (aún no se ha hecho). */
export function estadoTrabajo(t: TrabajoEspejoResumen): "ok" | "error" | "pausado" | "nuevo" {
  if (!t.activo) return "pausado";
  if (!t.resultado) return "nuevo";
  return t.resultado.startsWith("ERROR") ? "error" : "ok";
}

// --- Qué se puede elegir ---

/** Un repositorio que se puede elegir: su nombre para el espejo y cómo se enseña. */
export interface OpcionRepo {
  valor: string;
  nombre: string;
  /** De qué equipo es (su nombre), si se sabe. */
  equipo?: string;
}

/**
 * Los repositorios que puede copiar quien hace el espejo: en el almacén, los de la
 * zona `zona` (`<usuario>/<repo>`, con su nombre y su equipo si están en esta consola);
 * en el propio equipo, los de sus discos (por id).
 */
export function opcionesRepos(hace: Equipo, quien: "almacen" | "equipo", equipos: Equipo[], zona: string = PRINCIPAL): OpcionRepo[] {
  if (quien === "equipo") {
    const locales = new Set((hace.resumen?.destinos ?? []).filter((d) => d.tipo === "local").map((d) => d.id));
    return (hace.resumen?.repositorios ?? []).filter((r) => locales.has(r.destino)).map((r) => ({ valor: r.id, nombre: r.nombre || r.id }));
  }
  const g = hace.resumen?.guarda_copias;
  const lista = zona === PRINCIPAL ? g?.repositorios : g?.zonas?.find((z) => z.id === zona)?.repositorios;
  const nombres = [...new Set((lista ?? []).flatMap((u) => u.repos.map((r) => (r === "." || r === "" ? u.usuario : `${u.usuario}/${r}`))))].filter((n) => n.split("/").length <= 2).sort();
  return nombres.map((n) => {
    for (const e of equipos)
      for (const r of e.resumen?.repositorios ?? []) {
        const en = repoEnAlmacen(e, r, equipos);
        if (en && en.almacen.id === hace.id && en.nombre === n && en.zona === zona) return { valor: n, nombre: r.nombre || r.id, equipo: e.nombre };
      }
    return { valor: n, nombre: n };
  });
}

/** Los equipos que guardan en el almacén (su usuario y, si está aquí, su nombre). */
export function equiposDelAlmacen(almacen: Equipo, equipos: Equipo[], zona: string = PRINCIPAL): { usuario: string; nombre: string }[] {
  const usuarios = [...new Set(opcionesRepos(almacen, "almacen", equipos, zona).map((o) => o.valor.split("/")[0]))];
  return usuarios.map((u) => {
    const e = equipos.find((x) => (x.resumen?.repositorios ?? []).some((r) => repoEnAlmacen(x, r, equipos)?.nombre.split("/")[0] === u && repoEnAlmacen(x, r, equipos)?.almacen.id === almacen.id));
    return { usuario: u, nombre: e?.nombre ?? u };
  });
}

// --- Los espejos de una copia (la lista de copias los enseña agrupados por almacén) ---

/** Un trabajo que copia ese repositorio, con quién lo hace. */
export interface EspejoDeCopia {
  trabajo: TrabajoEspejoResumen;
  /** El equipo que lo hace (el almacén o el propio equipo). */
  hace: Equipo;
}
/** Los espejos de un repositorio, agrupados por quién los hace. */
export interface GrupoEspejos {
  /** Id del almacén (o del propio equipo). */
  id: string;
  nombre: string;
  /** El que los hace, si está en esta consola. */
  equipo: Equipo | null;
  /** «almacen» (hechos por el almacén donde está el repositorio) o «equipo» (por el propio equipo). */
  quien: "almacen" | "equipo";
  /** El almacén no está en esta consola: sus espejos no se ven aquí («Conectar también…»). */
  fuera: boolean;
  /** Las otras consolas del equipo que copia (por su nombre), para decir dónde mirar. */
  consolas: string[];
  /** Cómo se llama el repositorio para ese almacén (`<usuario>/<repo>`) o en el equipo (su id). */
  nombreRepo: string;
  /** De qué zona copia (en el almacén). */
  zona: string;
  espejos: EspejoDeCopia[];
}

/** ¿Este trabajo del almacén copia el repositorio `nombre` (de la zona `zona`)? */
export function trabajoIncluye(t: Pick<TrabajoEspejo, "que" | "zona" | "quien">, nombre: string, zona: string = PRINCIPAL): boolean {
  if (t.quien === "almacen" && (t.zona || PRINCIPAL) !== zona) return false;
  if (t.que.tipo === "todos") return true;
  if (t.que.tipo === "equipos") return t.que.equipos.some((u) => nombre === u || nombre.startsWith(`${u}/`));
  return t.que.repos.includes(nombre);
}

/**
 * Los espejos de un repositorio de `equipo`, agrupados por quién los hace: el almacén
 * donde está (si está en esta consola; si no, un grupo «fuera» sin espejos) y el propio
 * equipo (los de sus discos). Plan 0.7.26, bloque 4 (lo usa la lista de copias, bloque 3).
 */
export function espejosDeCopia(equipo: Equipo, repo: RepositorioResumen, equipos: Equipo[]): GrupoEspejos[] {
  const grupos: GrupoEspejos[] = [];
  const en = repoEnAlmacen(equipo, repo, equipos);
  const consolas = (equipo.resumen?.consolas ?? []).filter((c) => !c.esta).map((c) => c.nombre?.trim() || "otra consola");
  if (en) {
    const almacen = equipos.find((e) => e.id === en.almacen.id) ?? en.almacen;
    const espejos = trabajosDelAlmacen(almacen)
      .filter((t) => trabajoIncluye(t, en.nombre, en.zona))
      .sort((a, b) => a.orden - b.orden)
      .map((trabajo) => ({ trabajo, hace: almacen }));
    grupos.push({ id: almacen.id, nombre: almacen.nombre, equipo: almacen, quien: "almacen", fuera: false, consolas: [], nombreRepo: en.nombre, zona: en.zona, espejos });
  } else {
    // Lo dio un almacén del cliente («Copiar en …») que no está en esta consola.
    const d = destinoDe(equipo.resumen?.destinos, repo);
    if (d?.tipo === "rest" && d.equipo_almacen && !equipos.some((e) => e.id === d.equipo_almacen)) {
      grupos.push({ id: d.equipo_almacen, nombre: d.nombre || "Almacén", equipo: null, quien: "almacen", fuera: true, consolas, nombreRepo: "", zona: PRINCIPAL, espejos: [] });
    }
  }
  const propios = trabajosDelEquipo(equipo)
    .filter((t) => trabajoIncluye(t, repo.id))
    .sort((a, b) => a.orden - b.orden)
    .map((trabajo) => ({ trabajo, hace: equipo }));
  if (propios.length) grupos.push({ id: equipo.id, nombre: equipo.nombre, equipo, quien: "equipo", fuera: false, consolas: [], nombreRepo: repo.id, zona: PRINCIPAL, espejos: propios });
  return grupos;
}

/** El enlace a los espejos de un equipo (almacén o propio), con lo que se quiere hacer ya elegido. */
export function hrefEspejos(cliente: string, equipo: string, nuevo?: { repo?: string; zona?: string; destino?: string; quien?: "almacen" | "equipo" }, editar?: string): string {
  const q = new URLSearchParams();
  if (nuevo) {
    q.set("nuevo", "1");
    if (nuevo.repo) q.set("repo", nuevo.repo);
    if (nuevo.zona && nuevo.zona !== PRINCIPAL) q.set("zona", nuevo.zona);
    if (nuevo.destino) q.set("destino", nuevo.destino);
    if (nuevo.quien) q.set("quien", nuevo.quien);
  }
  if (editar) q.set("editar", editar);
  const s = q.toString();
  return `/c/${cliente}/equipos/${equipo}/espejos${s ? `?${s}` : ""}`;
}
