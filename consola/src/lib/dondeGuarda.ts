// «¿Dónde se guardan las copias?» (v1.41): en palabras y sin dudas, para
// cada repositorio (y por tanto para cada copia): en el almacén de otro
// equipo, en una carpeta del mismo equipo que protege, en un disco USB, en
// la nube o en un servidor de copias de fuera. Y el aviso de seguridad
// «copias en el mismo equipo»: si ese equipo se daña o lo cifra un
// ransomware, se pierden los archivos y sus copias a la vez.
//
// Todo sale del resumen de los equipos (sin rutas: de un destino local, el
// agente ≥ v1.41 dice solo su unidad, si es extraíble y si es de la red).
import type { ComprobacionProteccion, DestinoResumen, Equipo, RepositorioResumen } from "./tipos";
import { destinoDe } from "./repo";
import { esDeAlmacen } from "./retencion";
import { claveZona, zonaDeDestino } from "./destinos";

export type ClaseLugar = "almacen" | "almacen_propio" | "carpeta" | "usb" | "red" | "nube" | "servidor" | "desconocido";

export interface Lugar {
  clase: ClaseLugar;
  /** Lo principal: «Almacén SERVIDOR-ALTAMAR (otro equipo)», «En este mismo equipo (D:)», «Nube (Backblaze B2)»… */
  texto: string;
  /** Lo que va después del «·»: la carpeta del almacén, el bucket, el servidor o el nombre del destino. */
  detalle: string | null;
  /** Se queda en el mismo equipo que hace la copia (aunque sea en un disco USB). */
  mismoEquipo: boolean;
  /** El almacén (si es uno del cliente). */
  almacen?: Equipo;
  /** Un destino local sin saber si es un disco USB: el agente aún no lo dice (anterior a v1.41) o no pudo saberlo. */
  discoDesconocido?: "agente" | "no_se_sabe";
  /**
   * v1.4x: la clave del destino en el catálogo de esta consola (`zona:<almacén>:<zona>` o el id
   * del destino), para enseñar el nombre que se le puso aquí (SeGuardaEn). Los nombres del
   * catálogo son de cada consola (docs/consolas-multiples.md §6).
   */
  clave?: string;
  /** v1.4x: un almacén que no está en esta consola (se gestiona desde otra). */
  deOtraConsola?: boolean;
}

const NUBE: Record<string, string> = { s3: "S3", b2: "Backblaze B2" };

/** «host:puerto» de una dirección («https://nas:8000/caja-1/» → «nas:8000»), sin usuario. */
export function servidorDe(donde: string | null | undefined): string | null {
  if (!donde) return null;
  const sin = donde.trim().replace(/^[a-z0-9+]+:(?=[a-z]+:\/\/)/i, "");
  try {
    const u = new URL(sin.includes("://") ? sin : `x://${sin}`);
    return u.host || null;
  } catch {
    return sin.replace(/^[^@]*@/, "").split("/")[0] || null;
  }
}

/** El almacén del cliente que guarda este destino (o undefined). */
export function almacenDeDestino(d: DestinoResumen | undefined, equipos: Equipo[]): Equipo | undefined {
  if (!d || d.tipo !== "rest") return undefined;
  return equipos.find((a) => a.resumen?.guarda_copias?.activo && a.modo !== "trasladado" && esDeAlmacen(d, a)) ?? (d.equipo_almacen ? equipos.find((a) => a.id === d.equipo_almacen) : undefined);
}

/**
 * Dónde guarda un destino de `equipo` (el que hace la copia), en palabras.
 * `equipos`: todos los del cliente (para reconocer los almacenes).
 */
export function lugarDe(d: DestinoResumen | undefined, equipo: Pick<Equipo, "id" | "nombre">, equipos: Equipo[]): Lugar {
  if (!d) return { clase: "desconocido", texto: "Un destino que este equipo aún no ha descrito", detalle: null, mismoEquipo: false };
  switch (d.tipo) {
    case "local": {
      const u = d.unidad ? ` (${d.unidad})` : "";
      if (d.red) return { clase: "red", texto: "Carpeta de otra máquina de la red", detalle: d.nombre, mismoEquipo: false, clave: d.id };
      if (d.extraible) return { clase: "usb", texto: `Disco extraíble${u} de este equipo`, detalle: d.nombre, mismoEquipo: true, clave: d.id };
      return { clase: "carpeta", texto: `En este mismo equipo${u}`, detalle: d.nombre, mismoEquipo: true, discoDesconocido: !("extraible" in d) ? "agente" : d.extraible == null ? "no_se_sabe" : undefined, clave: d.id };
    }
    case "rest": {
      const a = almacenDeDestino(d, equipos);
      // Tarea 7b: en otra zona del almacén (otro disco), con su carpeta.
      const z = a ? zonaDeDestino(d, equipos) : null;
      const zona = z && !z.principal && z.almacen.id === a?.id ? z : null;
      const carpeta = zona ? zona.carpeta : (a?.resumen?.guarda_copias?.carpeta ?? null);
      const enZona = zona?.nombre ? ` · ${zona.nombre}` : "";
      const clave = z ? claveZona(z.almacen.id, z.id) : d.id;
      if (a && a.id === equipo.id) return { clase: "almacen_propio", texto: `Su propio almacén${enZona} (este mismo equipo)`, detalle: carpeta, mismoEquipo: true, almacen: a, clave };
      if (a) return { clase: "almacen", texto: `Almacén ${a.nombre}${enZona} (otro equipo)`, detalle: carpeta, mismoEquipo: false, almacen: a, clave };
      // v1.4x: lo dio un almacén del cliente («Copiar en …») que no está en esta consola: el
      // nombre del destino (el mismo en todas las consolas: lo guarda el equipo), no «externo».
      if (d.equipo_almacen) return { clase: "almacen", texto: `Almacén ${d.nombre} (otro equipo)`, detalle: "se gestiona desde otra consola", mismoEquipo: false, clave, deOtraConsola: true };
      return { clase: "servidor", texto: "Servidor de copias externo", detalle: [d.nombre, servidorDe(d.donde)].filter(Boolean).join(" · ") || null, mismoEquipo: false, clave: d.id };
    }
    case "s3":
    case "b2":
      return { clase: "nube", texto: `Nube (${NUBE[d.tipo]})`, detalle: d.donde ? `bucket ${d.donde}` : d.nombre, mismoEquipo: false, clave: d.id };
    case "sftp":
      return { clase: "servidor", texto: "Servidor externo (SFTP)", detalle: servidorDe(d.donde) ?? d.nombre, mismoEquipo: false, clave: d.id };
    default:
      return { clase: "desconocido", texto: `Destino «${d.nombre}»`, detalle: d.donde ?? null, mismoEquipo: false, clave: d.id };
  }
}

/** Dónde guarda un repositorio de `equipo`. */
export function lugarRepo(repo: RepositorioResumen | null | undefined, equipo: Equipo, equipos: Equipo[]): Lugar {
  return lugarDe(destinoDe(equipo.resumen?.destinos, repo), equipo, equipos);
}

/** «Almacén SERVIDOR-ALTAMAR (otro equipo) · D:\Copias»: la línea entera (sin el «Se guarda en:»). */
export const lineaLugar = (l: Lugar) => (l.detalle ? `${l.texto} · ${l.detalle}` : l.texto);

export interface RiesgoMismoEquipo {
  lugar: Lugar;
  /** Corto, para chips y listas. */
  titulo: string;
  /** La frase entera del aviso. */
  texto: string;
  /** Una nota de más (el disco no se sabe si es USB). */
  nota: string | null;
}

/**
 * ¿Se quedan las copias de este repositorio en el mismo equipo que protegen,
 * sin nada fuera? (Una carpeta del propio equipo o su propio almacén, sin
 * copia externa ni espejo; un disco USB o una carpeta de la red no cuentan.)
 * Si no se sabe qué disco es, también avisa.
 */
export function riesgoMismoEquipo(repo: RepositorioResumen, equipo: Equipo, equipos: Equipo[]): RiesgoMismoEquipo | null {
  if (repo.solo_lectura) return null;
  // Ninguna copia guarda ya en él (p. ej. tras «Mover a otro sitio…»): no hay nada que avisar.
  if (equipo.resumen?.copias && !equipo.resumen.copias.some((k) => k.repo === repo.id && k.activa !== false)) return null;
  const lugar = lugarRepo(repo, equipo, equipos);
  if (lugar.clase !== "carpeta" && lugar.clase !== "almacen_propio") return null;
  if (repo.externa) return null;
  // Su propio almacén con espejo (a otro disco o a la nube): ya sale de aquí cada noche.
  if (lugar.clase === "almacen_propio" && equipo.resumen?.guarda_copias?.espejo?.destinos?.length) return null;
  const n = equipo.nombre;
  return {
    lugar,
    titulo: "Copias en el mismo equipo",
    texto: `Las copias de ${n} se guardan en el propio ${n}: si ese equipo se daña o lo cifra un ransomware, se pierden las dos. Guárdalas en un almacén de otro equipo o añade una copia externa.`,
    nota:
      lugar.discoDesconocido === "agente"
        ? `El agente de ${n} aún no dice si es un disco USB (actualízalo). Si lo es y lo guardas aparte, no pasa nada.`
        : lugar.discoDesconocido
          ? "No se pudo saber si es un disco USB. Si lo es y lo guardas aparte, no pasa nada."
          : null,
  };
}

/** Los repositorios de todos los equipos con copias en el mismo equipo (para «Necesita atención»). */
export function riesgosDelCliente(equipos: Equipo[], todos: Equipo[] = equipos): { equipo: Equipo; repo: RepositorioResumen; riesgo: RiesgoMismoEquipo }[] {
  return equipos
    .filter((e) => e.modo !== "trasladado" && e.confirmado)
    .flatMap((e) =>
      (e.resumen?.repositorios ?? [])
        .flatMap((r) => {
          const riesgo = riesgoMismoEquipo(r, e, todos);
          return riesgo ? [{ equipo: e, repo: r, riesgo }] : [];
        }),
    );
}

/**
 * La comprobación «Fuera de este equipo» para la salud de la protección,
 * cuando el agente no la manda (anterior a v1.41, o su propio almacén, que el
 * agente ve como un servidor más).
 */
export function comprobacionLugar(repo: RepositorioResumen, equipo: Equipo, equipos: Equipo[]): ComprobacionProteccion | null {
  const r = riesgoMismoEquipo(repo, equipo, equipos);
  if (r) return { id: "lugar", estado: "aviso", etiqueta: "Fuera de este equipo", detalle: `${r.lugar.texto}: si se daña o lo cifra un ransomware, se pierden los archivos y las copias. Guárdalas en un almacén de otro equipo o añade una copia externa.` };
  return null;
}
