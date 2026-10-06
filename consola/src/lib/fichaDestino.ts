// La página de un destino (`/c/[c]/destinos/[d]`, docs/copias-en-cadena.md «La
// página de un destino»): lo que no es pantalla. Qué repositorios guarda, qué
// lo usa (copias externas y derivadas, el espejo del almacén que llega o sale),
// lo último que pasó y dónde se puede usar en una copia (con `usosPosibles` de
// lib/cadenas.ts, el mismo que «Añadir paso» del editor de copias). Todo sale de los
// resúmenes de los equipos y del catálogo (sin rutas de los equipos ni secretos).
import type { DestinoCatalogo, DestinoResumen, Equipo, RepositorioResumen } from "./tipos";
import { claveNube, claveZona, PRINCIPAL, zonaDeDestino, zonasDe, type DestinoVista } from "./destinos";
import { destinoDe } from "./repo";
import { destinosParaPasos, usosPosibles } from "./cadenas";

/** La dirección de la página de un destino (la clave del catálogo, codificada). */
export const hrefDestino = (cliente: string, clave: string) => `/c/${cliente}/destinos/${encodeURIComponent(clave)}`;

/** La clave (la de su página y la del catálogo) del destino de un equipo: su zona si es de un almacén; si no, su id. */
export function claveDeDestino(d: DestinoResumen, equipos: Equipo[]): string {
  const z = zonaDeDestino(d, equipos);
  return z ? claveZona(z.almacen.id, z.id) : d.id;
}

/** El destino de una clave (null si ya no está: una zona quitada, una nube desconectada…). */
export function vistaPorClave(clave: string, equipos: Equipo[], catalogo: DestinoCatalogo[]): DestinoVista | null {
  // Los del cliente y las nubes conectadas en los equipos (las de «Añadir paso»).
  return destinosParaPasos(equipos, catalogo).find((v) => v.clave === clave) ?? null;
}

export interface RepoEnDestino {
  equipo: Equipo;
  repo: RepositorioResumen;
}

/** Los repositorios que se guardan en un destino (en una zona: los de todos los equipos que copian en ella). */
export function reposEnDestino(v: DestinoVista, equipos: Equipo[]): RepoEnDestino[] {
  return equipos.flatMap((e) =>
    (e.resumen?.repositorios ?? []).filter((r) => {
      const d = destinoDe(e.resumen?.destinos, r);
      return !!d && v.ids.includes(d.id);
    }).map((repo) => ({ equipo: e, repo })),
  );
}

export interface UsoDestino {
  /** `externa` y `derivada`: copias de un repositorio que llegan aquí; `espejo_entra`: el espejo de un almacén que llega aquí; `espejo_sale`: lo que esta zona refleja en otro sitio. */
  tipo: "externa" | "derivada" | "espejo_entra" | "espejo_sale";
  texto: string;
  equipo: Equipo;
  href: string;
  ultima?: string | null;
  resultado?: string | null;
}

type DestinoEspejo = NonNullable<NonNullable<NonNullable<NonNullable<Equipo["resumen"]>["guarda_copias"]>["espejo"]>["destinos"]>[number];

/** El nombre de lo que hay al otro lado de un destino del espejo: la nube, la zona o la carpeta. */
function nombreEspejo(a: Equipo, d: DestinoEspejo): string {
  if (d.tipo === "nube") return d.nube ?? "una nube";
  if (d.tipo === "zona") {
    const z = zonasDe(a).find((x) => x.id === d.carpeta);
    return z?.nombre ?? "otra zona";
  }
  return d.carpeta ?? "otra carpeta";
}

/** Lo que usa un destino además de sus repositorios: copias externas y derivadas y el espejo de los almacenes. */
export function usosDeDestino(v: DestinoVista, equipos: Equipo[], cliente: string): UsoDestino[] {
  const out: UsoDestino[] = [];
  for (const e of equipos) {
    const destinos = e.resumen?.destinos ?? [];
    for (const r of e.resumen?.repositorios ?? []) {
      const href = `/c/${cliente}/equipos/${e.id}/repositorios/${encodeURIComponent(r.id)}`;
      const idExterna = r.externa ? (r.externa.destino_id ?? destinos.find((d) => d.nombre === r.externa!.destino)?.id ?? null) : null;
      if (idExterna && v.ids.includes(idExterna)) out.push({ tipo: "externa", texto: `Copia externa de «${r.nombre}» (${e.nombre})`, equipo: e, href });
      for (const x of r.derivadas ?? []) if (x.destino_id && v.ids.includes(x.destino_id)) out.push({ tipo: "derivada", texto: `Copia derivada de «${r.nombre}» (${e.nombre})`, equipo: e, href });
    }
  }
  // El espejo de los almacenes: lo que llega aquí (a esta nube o a esta zona) y lo que sale de esta zona.
  for (const a of equipos) {
    const esp = a.resumen?.guarda_copias?.espejo;
    if (!a.resumen?.guarda_copias?.activo || !esp) continue;
    const zonas = zonasDe(a);
    const nombreZona = (id: string | null | undefined) => zonas.find((z) => z.id === (id ?? PRINCIPAL))?.nombre ?? "la zona principal";
    for (const d of esp.destinos ?? []) {
      const href = `/c/${cliente}/equipos/${a.id}`;
      const entra = (v.nube && v.nube.equipo.id === a.id && d.tipo === "nube" && d.nube === v.nube.nombre) || (v.zona && v.zona.almacen.id === a.id && d.tipo === "zona" && d.carpeta === v.zona.id);
      if (entra) out.push({ tipo: "espejo_entra", texto: `Espejo de ${nombreZona(d.zona)} del almacén ${a.nombre}`, equipo: a, href, ultima: d.ultima, resultado: d.resultado });
      if (v.zona && v.zona.almacen.id === a.id && (d.zona ?? PRINCIPAL) === v.zona.id)
        out.push({ tipo: "espejo_sale", texto: `Se refleja en ${nombreEspejo(a, d)}`, equipo: a, href: d.tipo === "nube" && d.nube ? hrefDestino(cliente, claveNube(a.id, d.nube)) : d.tipo === "zona" && d.carpeta ? hrefDestino(cliente, claveZona(a.id, d.carpeta)) : href, ultima: d.ultima, resultado: d.resultado });
    }
  }
  return out;
}

export interface SucesoDestino {
  cuando: string;
  tono: "ok" | "warn" | "bad";
  texto: string;
  detalle?: string | null;
  href: string;
}

const tonoDe = (r: string | null | undefined): SucesoDestino["tono"] => (r === "fallo" || r === "error" ? "bad" : r === "aviso" ? "warn" : "ok");

/** Lo último que pasó en un destino: la última vez de cada copia que guarda aquí y del espejo que llega o sale, lo más reciente primero. */
export function actividadDestino(v: DestinoVista, equipos: Equipo[], cliente: string, max = 12): SucesoDestino[] {
  const out: SucesoDestino[] = [];
  for (const { equipo: e, repo: r } of reposEnDestino(v, equipos))
    for (const k of e.resumen?.copias ?? [])
      if (k.repo === r.id && k.ultima?.cuando)
        out.push({
          cuando: k.ultima.cuando,
          tono: tonoDe(k.ultima.estado),
          texto: `Copia «${k.nombre}» de ${e.nombre}`,
          detalle: k.ultima.estado === "ok" ? null : (k.ultima.mensaje ?? null),
          href: `/c/${cliente}/equipos/${e.id}/copias/${encodeURIComponent(k.id)}`,
        });
  for (const u of usosDeDestino(v, equipos, cliente))
    if ((u.tipo === "espejo_entra" || u.tipo === "espejo_sale") && u.ultima) out.push({ cuando: u.ultima, tono: tonoDe(u.resultado), texto: u.texto, detalle: u.resultado && tonoDe(u.resultado) !== "ok" ? u.resultado : null, href: u.href });
  return out.sort((a, b) => Date.parse(b.cuando) - Date.parse(a.cuando)).slice(0, max);
}

export interface UsoPosible {
  texto: string;
  /** Una línea: qué hace y quién lo hace (o qué falta antes). */
  detalle: string;
  href: string;
}

/**
 * «Usar en una copia» de la página de un destino: con qué copias se puede usar
 * y un enlace que abre el paso con este destino ya elegido. Lo que se puede o
 * no hacer lo decide `usosPosibles` de `lib/cadenas.ts` (el mismo que «Añadir
 * paso» del editor de copias); aquí solo se recorren los equipos y sus
 * repositorios y se arma el enlace:
 * - **espejo** de un repositorio (lo hace su almacén, sin contraseñas):
 *   `?paso_espejo=<repo>&destino=<clave>` en la ficha del equipo dueño;
 * - **repositorio nuevo a partir de** él (el equipo dueño):
 *   `?derivada=<repo>&destino=<clave>`; también si antes hay que conectar la
 *   nube en ese equipo (lo dice, y el diálogo lo ofrece);
 * - **copia nueva** de carpetas: `?nueva=1&destino=<clave>` en el editor de
 *   copias (elige un repositorio del equipo en este destino, si tiene);
 * - un destino suelto del catálogo: «Nuevo repositorio» (`?nuevo=1`).
 */
export function usarEnCopia(v: DestinoVista, equipos: Equipo[], cliente: string): UsoPosible[] {
  if (v.clase === "suelto") return [{ texto: "Nuevo repositorio aquí", detalle: "Elige el equipo; pide las credenciales del destino una vez.", href: `/c/${cliente}/repositorios?nuevo=1` }];
  const q = `destino=${encodeURIComponent(v.clave)}`;
  const activos = equipos.filter((e) => e.confirmado && e.modo !== "trasladado");
  const out: UsoPosible[] = [];
  // Copias nuevas de carpetas: a una zona, desde cualquier equipo (no el propio almacén); a otro destino, desde quien lo tiene.
  for (const e of activos) {
    if (e.rol === "almacenamiento" || !usosPosibles(v, e, null, equipos).copia.ok) continue;
    const puede = v.zona ? v.zona.almacen.id !== e.id : v.ids.some((id) => e.resumen?.destinos?.some((d) => d.id === id));
    if (puede) out.push({ texto: `Copia nueva de ${e.nombre}`, detalle: `Sus carpetas a ${v.nombre}${v.zona ? ", en solo añadir" : ""}.`, href: `/c/${cliente}/equipos/${e.id}/copias?nueva=1&${q}` });
  }
  // Pasos a partir de un repositorio que ya existe.
  for (const e of activos)
    for (const r of e.resumen?.repositorios ?? []) {
      if (r.solo_lectura) continue;
      const u = usosPosibles(v, e, r, equipos);
      if (u.espejo.ok) {
        const alm = v.zona?.almacen.nombre ?? v.nube?.equipo.nombre ?? "su almacén";
        out.push({ texto: `Espejo de «${r.nombre}» (${e.nombre})`, detalle: `Lo hace el almacén ${alm}, sin contraseñas: los mismos archivos en «${v.nombre}».`, href: `/c/${cliente}/equipos/${e.id}?paso_espejo=${encodeURIComponent(r.id)}&${q}` });
      }
      const d = u.derivada;
      if (d.ok || d.accion?.tipo === "conectar_nube")
        out.push({
          texto: `Repositorio nuevo a partir de «${r.nombre}» (${e.nombre})`,
          detalle: d.ok ? `Lo hace ${e.nombre}, con su propia contraseña y retención${d.motivo ? ` (${d.motivo.toLowerCase()})` : ""}.` : `Lo hace ${e.nombre}: antes hay que conectar «${v.nube?.nombre ?? v.nombre}» también en ese equipo (te lo ofrece el diálogo).`,
          href: `/c/${cliente}/equipos/${e.id}?derivada=${encodeURIComponent(r.id)}&${q}`,
        });
    }
  return out;
}
