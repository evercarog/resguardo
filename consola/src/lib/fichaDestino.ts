// La página de un destino (`/c/[c]/destinos/[d]`, docs/copias-en-cadena.md «La
// página de un destino»): lo que no es pantalla. Qué repositorios guarda, qué
// lo usa (copias externas y derivadas, el espejo del almacén que llega o sale),
// lo último que pasó y dónde se puede usar en una copia (con `usosPosibles` de
// lib/cadenas.ts, el mismo que «Añadir paso» del editor de copias). Todo sale de los
// resúmenes de los equipos y del catálogo (sin rutas de los equipos ni secretos).
import type { DestinoCatalogo, DestinoResumen, Equipo, RepositorioResumen } from "./tipos";
import { claveNube, claveZona, clavesDe, PRINCIPAL, zonaDeDestino, zonaDetras, zonasDe, type DestinoVista } from "./destinos";
import { contenidoAlmacen, duenoDeCarpeta, esAdoptado } from "./almacenUnico";
import { destinoDe } from "./repo";
import { destinosParaPasos, usosPosibles } from "./cadenas";
import { estadoTrabajo, hrefEspejo, nombrePorDefecto, trabajosDelAlmacen, trabajosDelEquipo } from "./espejoTrabajos";
import type { TrabajoEspejoResumen } from "./tipos";

// --- Plan 0.8.0 (B2, 14r): lo que guarda un destino que recibe espejos ---

/** Un repositorio que llega a un destino por un espejo (con su dueño, si está en esta consola). */
export interface RepoPorEspejo {
  /** `<usuario>/<repo>` en el almacén, o el id del repositorio en un espejo del propio equipo. */
  valor: string;
  /** «Contabilidad», «Contabilidad (copia derivada)» o la carpeta tal cual (15k). */
  nombre: string;
  /** De quién es: el equipo, o «no sé de qué equipo es». */
  de: string;
  equipo?: Equipo;
  /** El repositorio (en una derivada, el de origen: su página). */
  repo?: RepositorioResumen;
  /** 15k: es una copia derivada de `repo` (su tamaño no es el de `repo`). */
  derivada?: boolean;
}
/** Un espejo que llega a un destino, con lo que guarda allí. */
export interface EspejoEnDestino {
  trabajo: TrabajoEspejoResumen;
  nombre: string;
  /** Quién lo hace: el almacén o el propio equipo. */
  hace: Equipo;
  quien: "almacen" | "equipo";
  estado: ReturnType<typeof estadoTrabajo>;
  href: string;
  repos: RepoPorEspejo[];
}

/** Los repositorios que copia un trabajo de espejo (según su selección), con su dueño si está aquí. */
export function reposDeTrabajo(t: TrabajoEspejoResumen, hace: Equipo, equipos: Equipo[]): RepoPorEspejo[] {
  if (t.quien === "equipo") {
    const locales = new Set((hace.resumen?.destinos ?? []).filter((d) => d.tipo === "local").map((d) => d.id));
    const suyos = (hace.resumen?.repositorios ?? []).filter((r) => locales.has(r.destino));
    const elegidos = t.que.tipo === "repos" ? suyos.filter((r) => (t.que as { repos: string[] }).repos.includes(r.id)) : suyos;
    return elegidos.map((r) => ({ valor: r.id, nombre: r.nombre || r.id, de: hace.nombre, equipo: hace, repo: r }));
  }
  const zona = t.zona || PRINCIPAL;
  const g = hace.resumen?.guarda_copias;
  const lista = zona === PRINCIPAL ? g?.repositorios : g?.zonas?.find((z) => z.id === zona)?.repositorios;
  const todos = [...new Set((lista ?? []).flatMap((u) => u.repos.map((r) => (r === "." || r === "" ? u.usuario : `${u.usuario}/${r}`))))].sort();
  const elegidos = t.que.tipo === "repos" ? todos.filter((x) => (t.que as { repos: string[] }).repos.includes(x)) : t.que.tipo === "equipos" ? todos.filter((x) => (t.que as { equipos: string[] }).equipos.includes(x.split("/")[0])) : todos;
  // 15k: de quién es cada carpeta (sus repositorios, también los adoptados, y sus copias derivadas).
  const contenido = contenidoAlmacen(hace, equipos);
  return elegidos.map((valor) => {
    const x = duenoDeCarpeta(hace, valor, equipos, zona, contenido);
    return { valor, nombre: x.nombre, de: x.de, ...(x.equipo ? { equipo: x.equipo } : {}), ...(x.repo && (x.tipo === "repo" || x.tipo === "derivada") ? { repo: x.repo } : {}), ...(x.tipo === "derivada" ? { derivada: true } : {}) };
  });
}

/** Los espejos que llegan a un destino (una nube conectada o una zona), por su nombre y con lo que guardan allí. */
export function espejosEnDestino(v: DestinoVista, equipos: Equipo[], cliente: string): EspejoEnDestino[] {
  const out: EspejoEnDestino[] = [];
  const llega = (t: TrabajoEspejoResumen, hace: Equipo) =>
    (!!v.nube && v.nube.equipo.id === hace.id && t.adonde.tipo === "nube" && t.adonde.nube === v.nube.nombre) || (!!v.zona && v.zona.almacen.id === hace.id && t.adonde.tipo === "zona" && t.adonde.carpeta === v.zona.id);
  for (const hace of equipos)
    for (const [quien, lista] of [
      ["almacen", trabajosDelAlmacen(hace)],
      ["equipo", trabajosDelEquipo(hace)],
    ] as const)
      for (const t of lista)
        if (llega(t, hace))
          out.push({
            trabajo: t,
            nombre: t.nombre.trim() || nombrePorDefecto(t.adonde),
            hace,
            quien,
            estado: estadoTrabajo(t),
            href: hrefEspejo(cliente, hace.id, t.id),
            repos: reposDeTrabajo(t, hace, equipos),
          });
  return out;
}

/** La dirección de la página de un destino (la clave del catálogo, codificada). */
export const hrefDestino = (cliente: string, clave: string) => `/c/${cliente}/destinos/${encodeURIComponent(clave)}`;

/** La clave (la de su página y la del catálogo) del destino de un equipo: su zona si es de un almacén; si no, su id. */
export function claveDeDestino(d: DestinoResumen, equipos: Equipo[], de?: Pick<Equipo, "id" | "resumen">): string {
  // 15dd: con el equipo, un «Servidor de copias» que es un almacén del cliente es su zona.
  const z = de ? zonaDetras(de, d, equipos) : zonaDeDestino(d, equipos);
  return z ? claveZona(z.almacen.id, z.id) : d.id;
}

/** El destino de una clave (null si ya no está: una zona quitada, una nube desconectada…). */
export function vistaPorClave(clave: string, equipos: Equipo[], catalogo: DestinoCatalogo[]): DestinoVista | null {
  // Los del cliente y las nubes conectadas en los equipos (las de «Añadir paso»).
  // Plan 0.8.0 (B2): también por la clave de lo que se unió a él (la entrada del catálogo con el mismo bucket).
  const l = destinosParaPasos(equipos, catalogo);
  // 15dd: la de un «Servidor de copias» que se unió a su almacén lleva a la zona.
  return l.find((v) => v.clave === clave) ?? l.find((v) => clavesDe(v).includes(clave)) ?? l.find((v) => v.adoptados?.includes(clave)) ?? null;
}

export interface RepoEnDestino {
  equipo: Equipo;
  repo: RepositorioResumen;
  /** 15dd: llega a la zona por un «Servidor de copias» genérico (adoptado). */
  adoptado?: boolean;
}

/** Los repositorios que se guardan en un destino (en una zona: los de todos los equipos que copian en ella). */
export function reposEnDestino(v: DestinoVista, equipos: Equipo[]): RepoEnDestino[] {
  return equipos.flatMap((e) =>
    (e.resumen?.repositorios ?? []).flatMap((repo) => {
      const d = destinoDe(e.resumen?.destinos, repo);
      if (!d || !v.ids.includes(d.id)) return [];
      return [{ equipo: e, repo, ...(v.zona && esAdoptado(e, d, equipos) ? { adoptado: true } : {}) }];
    }),
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
      if (idExterna && v.ids.includes(idExterna)) out.push({ tipo: "externa", texto: `Copia derivada de «${r.nombre}» (${e.nombre})`, equipo: e, href });
      for (const x of r.derivadas ?? []) if (x.destino_id && v.ids.includes(x.destino_id)) out.push({ tipo: "derivada", texto: `Copia derivada de «${r.nombre}» (${e.nombre})`, equipo: e, href });
    }
  }
  // El espejo de los almacenes: lo que llega aquí (a esta nube o a esta zona) y lo que sale de esta zona.
  for (const a of equipos) {
    const esp = a.resumen?.guarda_copias?.espejo;
    if (!a.resumen?.guarda_copias?.activo || !esp) continue;
    const zonas = zonasDe(a);
    const nombreZona = (id: string | null | undefined) => zonas.find((z) => z.id === (id ?? PRINCIPAL))?.nombre ?? "la zona principal";
    // Plan 0.8.0 (B2, 14r): con trabajos, cada espejo que llega, por su nombre (también los que están en pausa).
    const trabajos = esp.trabajos ?? [];
    for (const t of trabajos) {
      const entra = (v.nube && v.nube.equipo.id === a.id && t.adonde.tipo === "nube" && t.adonde.nube === v.nube.nombre) || (v.zona && v.zona.almacen.id === a.id && t.adonde.tipo === "zona" && t.adonde.carpeta === v.zona.id);
      if (entra) out.push({ tipo: "espejo_entra", texto: `Espejo «${t.nombre.trim() || nombrePorDefecto(t.adonde)}» (${a.nombre})${t.activo ? "" : " · en pausa"}`, equipo: a, href: hrefEspejo(cliente, a.id, t.id), ultima: t.ultima, resultado: t.resultado });
    }
    for (const d of esp.destinos ?? []) {
      const href = `/c/${cliente}/equipos/${a.id}`;
      const entra = !trabajos.length && ((v.nube && v.nube.equipo.id === a.id && d.tipo === "nube" && d.nube === v.nube.nombre) || (v.zona && v.zona.almacen.id === a.id && d.tipo === "zona" && d.carpeta === v.zona.id));
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

/** El tono de un resultado: «fallo», «error» o un mensaje del agente que empieza por «ERROR» es malo. */
export const tonoDe = (r: string | null | undefined): SucesoDestino["tono"] => (r === "fallo" || r === "error" || !!r?.startsWith("ERROR") ? "bad" : r === "aviso" ? "warn" : "ok");

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
  if (v.clase === "suelto") return [{ texto: "Nuevo repositorio aquí", detalle: "Elige el equipo; pide las credenciales del destino una vez.", href: `/c/${cliente}/destinos?nuevo=1` }];
  const q = `destino=${encodeURIComponent(v.clave)}`;
  const activos = equipos.filter((e) => e.confirmado && e.modo !== "trasladado");
  const out: UsoPosible[] = [];
  // Copias nuevas de carpetas: a una zona, desde cualquier equipo (no el propio almacén); a otro destino, desde quien lo tiene.
  for (const e of activos) {
    if (e.rol === "almacenamiento") continue;
    const uc = usosPosibles(v, e, null, equipos).copia;
    // Tarea 4a: una nube, directamente (un repositorio nuevo allí); si aún no está en ese equipo, el diálogo ofrece conectarla.
    if (v.nube || v.tipo === "nube") {
      if (uc.ok || uc.accion?.tipo === "conectar_nube")
        out.push({
          texto: `Copia nueva de ${e.nombre}`,
          detalle: uc.ok ? `Sus carpetas directamente a ${v.nombre}, en un repositorio nuevo allí.` : `Sus carpetas directamente a ${v.nombre}: antes hay que conectarla también en ${e.nombre} (te lo ofrece el diálogo).`,
          href: `/c/${cliente}/equipos/${e.id}/copias?nueva=1&${q}`,
        });
      continue;
    }
    if (!uc.ok) continue;
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
          texto: `Nuevo repositorio a partir de «${r.nombre}» (${e.nombre})`,
          detalle: d.ok ? `Lo hace ${e.nombre}, con su propia contraseña y retención${d.motivo ? ` (${d.motivo.toLowerCase()})` : ""}.` : `Lo hace ${e.nombre}: antes hay que conectar «${v.nube?.nombre ?? v.nombre}» también en ese equipo (te lo ofrece el diálogo).`,
          href: `/c/${cliente}/equipos/${e.id}?derivada=${encodeURIComponent(r.id)}&${q}`,
        });
    }
  return out;
}
