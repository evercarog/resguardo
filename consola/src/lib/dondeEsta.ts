// Plan 0.8.1, C1 (plan-mejoras 15n): «Dónde está» cada cosa, a la vista.
//
// Todos los sitios donde existen los datos de un repositorio, con su **ruta
// real**: su carpeta en el almacén (zona y carpeta: `/srv/copias/pc-01-3f2a/
// contabilidad`), en un disco o carpeta local, en un servidor o en la nube, y
// cada espejo y copia derivada con su destino y su ruta allí
// (`Dropbox Oficina:Resguardo/pc-01-3f2a/contabilidad`), con lo que ocupa, la
// última vez y si está al día. Sale de lo que ya mandan los resúmenes y de los
// pasos de la regla 3-2-1-1-0 (`reglaDeCopia`) y del camino (`caminoDeCopia`):
// dice lo mismo que la tira y el camino. Además, para el editor del espejo,
// la ruta de **cada** repositorio elegido y si ya está en el destino (o en otra
// ruta, que lo subiría todo de nuevo). Sin runas (scripts/vectores-donde-esta.ts).
import type { AdondeEspejo, DestinoResumen, Equipo, Informe, RepositorioResumen, TrabajoEspejo } from "./tipos";
import type { Eslabon } from "./camino";
import type { PasoVista, ReglaCopia } from "./regla321";
import type { TipoCorto } from "./tipoDestino";
import type { Tono } from "./salud";
import { bytesRepo, destinoDe, informeDe, ultimaVersion } from "./repo";
import { almacenDetras, enAlmacenDe } from "./mapa";
import { zonasDe } from "./destinos";
import { trabajoIncluye } from "./espejoTrabajos";
import { PRINCIPAL } from "./destinos";

/** Un sitio donde están los datos del repositorio. */
export interface Sitio {
  id: string;
  clase: "repositorio" | "espejo" | "derivada";
  /** El destino («Almacén · Disco D», «Backblaze Oficina»). */
  nombre: string;
  /** Quién lo hace o lo guarda, si no es el propio equipo («ALMACEN-01»). */
  quien: string | null;
  tipo: TipoCorto | null;
  /** La ruta real (en el equipo que lo guarda, en el servidor o en la nube), o null si no se sabe. */
  ruta: string | null;
  /** Por qué no hay ruta («su carpeta propia en el destino»). */
  nota: string | null;
  /** Lo que ocupa allí (si se sabe). */
  bytes: number | null;
  /** La última vez que se puso al día (o se intentó). */
  cuando: string | null;
  tono: Tono;
  estado: string;
  href: string | null;
  /** Dónde está físicamente (con infraestructura anotada). */
  fisico: string | null;
}

const esWindows = (r: string) => /^[A-Za-z]:/.test(r) || r.includes("\\");

/** Une una carpeta y una ruta relativa (`a/b`) con las barras de la carpeta. */
export function unirRuta(base: string, rel: string): string {
  const b = base.trim().replace(/[\\/]+$/, "");
  const r = rel.trim().replace(/^[\\/]+/, "");
  if (!b) return r;
  if (!r) return b;
  return esWindows(b) ? `${b}\\${r.replace(/\//g, "\\")}` : `${b}/${r}`;
}

/** La carpeta del repositorio dentro de su destino (`r.ruta`, o su id). */
const rutaPropia = (r: RepositorioResumen) => (r.ruta === "." ? "" : r.ruta || r.id);

/**
 * La ruta real de un repositorio en su destino: en un almacén, la carpeta de su zona y
 * `<usuario>/<repo>`; en un servidor o un bucket, su dirección y su carpeta; en una
 * nube conectada, `<nube>:<carpeta>/<repo>`; en un disco o carpeta local, la carpeta
 * que dice el agente (o la unidad, con un agente anterior).
 */
export function rutaDelRepositorio(e: Equipo, r: RepositorioResumen, equipos: Equipo[]): string | null {
  const d = destinoDe(e.resumen?.destinos, r);
  if (!d) return null;
  const alm = almacenDetras(e, d, equipos);
  if (alm) {
    const en = enAlmacenDe(r, d, alm);
    if (!en) return null;
    const z = zonasDe(alm).find((x) => x.id === en.zona) ?? zonasDe(alm).find((x) => x.id === PRINCIPAL);
    return z?.carpeta ? unirRuta(z.carpeta, en.nombre) : null;
  }
  return rutaEnDestino(d, rutaPropia(r));
}

/** Una carpeta (relativa) dentro de un destino de un equipo. */
export function rutaEnDestino(d: DestinoResumen, rel: string): string | null {
  switch (d.tipo) {
    case "local":
      if (d.carpeta) return unirRuta(d.carpeta, rel);
      return d.unidad ? `${d.unidad}\\…\\${rel.replace(/\//g, "\\")}` : null;
    case "nube":
      return d.nube ? `${d.nube}:${unirRuta(d.donde ?? "", rel)}` : null;
    case "b2":
    case "s3":
    case "rest":
    case "sftp":
      return d.donde ? unirRuta(d.donde.replace(/^(rest|s3|b2|sftp):/i, ""), rel) : null;
    default:
      return null;
  }
}

/** La ruta de un repositorio en el destino de un espejo (`<carpeta>/<repo>`; una zona, la carpeta de esa zona). */
export function rutaEnEspejo(adonde: Pick<AdondeEspejo, "tipo" | "carpeta" | "nube">, repo: string, almacen?: Equipo | null): string | null {
  if (adonde.tipo === "zona") {
    const z = almacen ? zonasDe(almacen).find((x) => x.id === adonde.carpeta) : undefined;
    return z?.carpeta ? unirRuta(z.carpeta, repo) : null;
  }
  const base = adonde.carpeta?.trim() ?? "";
  if (adonde.tipo === "nube") return adonde.nube ? `${adonde.nube}:${unirRuta(base, repo)}` : null;
  return base ? unirRuta(base, repo) : null;
}

/** La ruta de un paso «espejo» de la regla (lo que sabe de su destino). */
function rutaDePaso(p: PasoVista, equipos: Equipo[]): string | null {
  const x = p.espejo;
  if (!x?.ruta) return null;
  // A otra zona del almacén: la regla ya trae la carpeta de esa zona (no su id).
  if (x.tipo === "zona") return x.carpeta ? unirRuta(x.carpeta, x.ruta) : null;
  const almacen = x.almacen ? equipos.find((e) => e.id === x.almacen) : null;
  return rutaEnEspejo({ tipo: x.tipo as AdondeEspejo["tipo"], carpeta: x.carpeta, nube: x.nube ?? undefined }, x.ruta, almacen);
}

/**
 * Los sitios donde están los datos del repositorio de `rc`, en el orden del camino:
 * su destino, los espejos y las copias derivadas (con lo que dice el camino de cada uno).
 */
export function sitiosDe(rc: ReglaCopia, camino: Eslabon[], equipos: Equipo[], informe: Informe | null | undefined): Sitio[] {
  const inf = informeDe(informe, rc.repo.id);
  const out: Sitio[] = [];
  for (const p of rc.pasos) {
    const x = camino.find((c) => c.id === p.id);
    const base = {
      id: p.id,
      nombre: p.nombre,
      quien: x?.quien ?? null,
      tipo: x?.tipo ?? null,
      tono: x?.tono ?? "neutral",
      estado: x?.estado ?? "Sin datos",
      href: x?.href ?? null,
      fisico: p.lineaFisica ?? null,
    } satisfies Partial<Sitio>;
    if (p.tipo === "copia") {
      out.push({ ...base, clase: "repositorio", ruta: rutaDelRepositorio(rc.equipo, rc.repo, equipos), nota: null, bytes: bytesRepo(rc.repo, inf) ?? null, cuando: ultimaVersion(rc.repo, inf) ?? x?.cuando ?? null });
    } else if (p.tipo === "espejo") {
      const ruta = rutaDePaso(p, equipos);
      out.push({ ...base, clase: "espejo", ruta, nota: ruta ? null : "La carpeta de su destino no se sabe", bytes: x?.ocupa ?? null, cuando: x?.cuando ?? null });
    } else {
      out.push({ ...base, clase: "derivada", ruta: p.donde ?? null, nota: "Un repositorio propio en ese destino", bytes: null, cuando: x?.cuando ?? null });
    }
  }
  return out;
}

// ---------- El editor del espejo: la ruta de cada repositorio elegido ----------

export interface RutaRepoEspejo {
  repo: string;
  ruta: string | null;
  /** `nuevo`: se subirá entero; `ya_esta`: otro espejo ya lo deja ahí (solo subirá lo nuevo); `otra_ruta`: está en el mismo destino en otra carpeta. */
  estado: "nuevo" | "ya_esta" | "otra_ruta";
  /** La otra ruta y el espejo que lo deja allí. */
  otra?: string | null;
  espejo?: string | null;
}

/** ¿Van dos espejos al mismo destino (la misma nube, la misma zona o el mismo disco)? */
function mismoDestino(a: AdondeEspejo, b: AdondeEspejo): boolean {
  if (a.tipo !== b.tipo) return false;
  if (a.tipo === "nube") return !!a.nube && a.nube === b.nube;
  if (a.tipo === "zona") return a.carpeta === b.carpeta;
  const unidad = (x: string) => (/^[A-Za-z]:/.test(x) ? x.slice(0, 2).toUpperCase() : x.split("/").filter(Boolean)[0] ?? "");
  return unidad(a.carpeta) === unidad(b.carpeta);
}

/**
 * Para cada repositorio elegido en un espejo, dónde quedará y si ya está en ese destino:
 * por otro espejo con la misma ruta («ya está allí: solo subirá lo nuevo») o con otra
 * («ya está en … : con esta carpeta se subiría todo de nuevo»). Lo que ya está en el propio
 * espejo (al cambiarlo) cuenta como «ya está» si su ruta no cambia.
 */
export function rutasDelEspejo(t: TrabajoEspejo, otros: readonly (TrabajoEspejo & { ocupa?: { repos?: Record<string, number> | null } | null })[], repos: readonly string[], almacen?: Equipo | null, antes?: TrabajoEspejo | null): RutaRepoEspejo[] {
  return repos.map((repo) => {
    const ruta = rutaEnEspejo(t.adonde, repo, almacen);
    if (antes && antes.id === t.id && trabajoIncluye(antes, repo, antes.zona || PRINCIPAL) && rutaEnEspejo(antes.adonde, repo, almacen) === ruta) return { repo, ruta, estado: "ya_esta" as const, espejo: antes.nombre };
    let otra: RutaRepoEspejo | null = null;
    for (const o of otros) {
      if (o.id === t.id || !mismoDestino(o.adonde, t.adonde) || !trabajoIncluye(o, repo, o.zona || PRINCIPAL)) continue;
      const r = rutaEnEspejo(o.adonde, repo, almacen);
      if (r && r === ruta) return { repo, ruta, estado: "ya_esta" as const, espejo: o.nombre };
      if (r && !otra) otra = { repo, ruta, estado: "otra_ruta", otra: r, espejo: o.nombre };
    }
    return otra ?? { repo, ruta, estado: "nuevo" as const };
  });
}

/** El destino de una derivada en un equipo (para su ruta en «Dónde está» de un destino). */
export const destinoDeId = (e: Equipo, id: string | null | undefined): DestinoResumen | undefined => (id ? e.resumen?.destinos?.find((d) => d.id === id) : undefined);
