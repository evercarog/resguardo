// Destinos de primera clase (tarea 7a) y zonas del almacén (tarea 7b):
// docs/copias-en-cadena.md. Todo lo que se ve como «destino» sale de tres
// sitios, que aquí se juntan:
//
// - las zonas de cada almacén del cliente (la principal y las de
//   `guarda_copias.zonas`), del resumen del almacén;
// - los destinos que ya usan los equipos (`resumen.destinos`), agrupados por
//   su id; uno que es una zona de un almacén se enseña dentro de esa zona;
// - el catálogo del cliente en el servidor (`/api/clientes/{c}/destinos`):
//   nombres puestos a cualquiera de los anteriores y destinos creados sin
//   repositorio todavía. En claro y sin secretos: las credenciales viajan
//   selladas para cada equipo cuando crea un repositorio allí.
//
// Sin dependencias de Svelte: lo prueban los vectores (scripts/vectores-destinos.ts).
import type { DestinoCatalogo, DestinoResumen, EspacioVolumen, Equipo } from "./tipos";
import { esDeAlmacen } from "./retencion";

/** El agente entiende las zonas (`guarda_copias { zona }`, `{ anadir, zona }`…). */
export const ADMITE_ZONAS = "zonas_almacen";
export const admiteZonas = (e: Equipo | null | undefined) => !!e?.resumen?.guarda_copias?.activo && !!e.resumen.admite?.includes(ADMITE_ZONAS);

/** El id de la zona principal de un almacén (la de siempre). */
export const PRINCIPAL = "principal";
/** Como mucho, zonas además de la principal (lo mismo que el agente). */
export const ZONAS_MAX = 8;

/** Clave de una zona en el catálogo. */
export const claveZona = (equipo: string, zona: string) => `zona:${equipo}:${zona}`;
/** Clave de una nube conectada en un equipo (para el espejo). */
export const claveNube = (equipo: string, nombre: string) =>
  `nube:${equipo}:${
    nombre
      .toLowerCase()
      .normalize("NFD")
      .replace(/[\u0300-\u036f]/g, "")
      .replace(/[^a-z0-9]+/g, "-")
      .replace(/^-|-$/g, "") || "nube"
  }`;

/** Id de una zona: `z` y 6 cifras hexadecimales. */
export const zonaIdValido = (id: string) => /^z[0-9a-f]{6}$/.test(id);

export interface ZonaVista {
  almacen: Equipo;
  /** `principal` o el id de la zona. */
  id: string;
  principal: boolean;
  /** Lo que dice el agente («Disco E»), o el disco de la carpeta de la principal. */
  nombre: string | null;
  puerto: number | null;
  carpeta: string | null;
  usuarios: number;
  espacio: EspacioVolumen | null;
  /** Si su rest-server responde ahora (solo zonas; `null`: no se sabe). */
  escucha: boolean | null;
  repositorios: { usuario: string; repos: string[] }[];
}

/** «D:» de una carpeta de Windows («D:\Resguardo»), o null. */
export function unidadDe(carpeta: string | null | undefined): string | null {
  const m = /^([a-z]):/i.exec((carpeta ?? "").trim());
  return m ? `${m[1].toUpperCase()}:` : null;
}

/** Las zonas de un almacén: la principal primero y después las demás (con un agente que las admite). */
export function zonasDe(a: Equipo): ZonaVista[] {
  const g = a.resumen?.guarda_copias;
  if (!g?.activo) return [];
  const u = unidadDe(g.carpeta);
  const principal: ZonaVista = {
    almacen: a,
    id: PRINCIPAL,
    principal: true,
    nombre: u ? `Disco ${u[0]}` : null,
    puerto: g.puerto ?? null,
    carpeta: g.carpeta ?? null,
    usuarios: g.usuarios ?? 0,
    espacio: g.espacio ?? null,
    escucha: null,
    repositorios: g.repositorios ?? [],
  };
  const otras = (g.zonas ?? [])
    .filter((z) => zonaIdValido(z.id))
    .map<ZonaVista>((z) => ({
      almacen: a,
      id: z.id,
      principal: false,
      nombre: z.nombre?.trim() || unidadDe(z.carpeta)?.replace(/^(.):$/, "Disco $1") || null,
      puerto: z.puerto,
      carpeta: z.carpeta,
      usuarios: z.usuarios ?? 0,
      espacio: z.espacio ?? null,
      escucha: z.escucha ?? null,
      repositorios: z.repositorios ?? [],
    }));
  return [principal, ...otras];
}

/** «Almacén ALMACEN-01 · Disco E» (o «Almacén ALMACEN-01» si la principal no tiene disco con letra). */
export function nombreZonaPorDefecto(z: Pick<ZonaVista, "almacen" | "nombre">): string {
  return `Almacén ${z.almacen.nombre}${z.nombre ? ` · ${z.nombre}` : ""}`;
}

/** El puerto de una dirección («https://nas:8002/caja/» → 8002), o null. */
export function puertoDe(donde: string | null | undefined): number | null {
  const m = /^(?:[a-z0-9+]+:)?[a-z]+:\/\/[^/]*?:(\d{1,5})(?:\/|$)/i.exec((donde ?? "").trim()) ?? /^[^/]*?:(\d{1,5})(?:\/|$)/.exec((donde ?? "").trim());
  const n = m ? Number(m[1]) : NaN;
  return Number.isInteger(n) && n > 0 && n < 65536 ? n : null;
}

/** Los almacenes del cliente (los que guardan copias ahora). */
export const almacenesDe = (equipos: Equipo[]) => equipos.filter((e) => e.resumen?.guarda_copias?.activo && e.modo !== "trasladado");

/**
 * La zona de un almacén donde está un destino de un equipo (rest): por el
 * almacén (`equipo_almacen`, o como hasta ahora por id o nombre) y por el
 * puerto de su dirección. Sin puerto o con el de la principal, la principal.
 */
export function zonaDeDestino(d: DestinoResumen | undefined, equipos: Equipo[]): ZonaVista | null {
  if (!d || d.tipo !== "rest") return null;
  const a = almacenesDe(equipos).find((x) => esDeAlmacen(d, x));
  if (!a) return null;
  const zonas = zonasDe(a);
  const p = puertoDe(d.donde);
  return (p != null && zonas.find((z) => z.puerto === p)) || zonas[0] || null;
}

export type ClaseDestino = "zona" | "equipo" | "nube" | "suelto";

export interface DestinoVista {
  /** La clave del catálogo (`zona:…`, `nube:…` o el id del destino). */
  clave: string;
  nombre: string;
  nombrePorDefecto: string;
  /** Tiene un nombre puesto en el catálogo. */
  renombrado: boolean;
  clase: ClaseDestino;
  /** `zona`, `rest`, `s3`, `b2`, `sftp`, `local`, `nube`… */
  tipo: string;
  /** Servidor o bucket (nunca una ruta local). */
  donde: string | null;
  zona?: ZonaVista;
  /** Un destino de los equipos (el primero que se vio con ese id). */
  destino?: DestinoResumen;
  /** Los ids de destino de los equipos que caen aquí (para contar sus repositorios). */
  ids: string[];
  /** Los equipos que lo usan (por nombre). */
  equipos: string[];
  /** Una nube conectada en un equipo (para el espejo). */
  nube?: { equipo: Equipo; nombre: string; tipo: string };
  /** Su entrada del catálogo, si la tiene. */
  catalogo?: DestinoCatalogo;
}

/** Los tipos de un destino suelto del catálogo (de red: los demás no se crean sin equipo). */
export const TIPOS_SUELTOS = ["b2", "s3", "rest", "sftp"] as const;
export type TipoSuelto = (typeof TIPOS_SUELTOS)[number];

/** Todos los destinos del cliente, juntos y con su nombre (el del catálogo si lo hay). */
export function destinosDelCliente(equipos: Equipo[], catalogo: DestinoCatalogo[] = []): DestinoVista[] {
  const porClave = new Map<string, DestinoVista>();
  const orden: DestinoVista[] = [];
  const poner = (v: DestinoVista) => {
    porClave.set(v.clave, v);
    orden.push(v);
    return v;
  };
  // 1. Las zonas de los almacenes.
  for (const a of almacenesDe(equipos))
    for (const z of zonasDe(a)) {
      const n = nombreZonaPorDefecto(z);
      poner({ clave: claveZona(a.id, z.id), nombre: n, nombrePorDefecto: n, renombrado: false, clase: "zona", tipo: "zona", donde: null, zona: z, ids: [], equipos: [] });
    }
  // 2. Los destinos de los equipos (agrupados por su id); los de una zona, dentro de ella.
  for (const e of equipos)
    for (const d of e.resumen?.destinos ?? []) {
      const z = zonaDeDestino(d, equipos);
      const clave = z ? claveZona(z.almacen.id, z.id) : d.id;
      const v =
        porClave.get(clave) ??
        poner({ clave, nombre: d.nombre, nombrePorDefecto: d.nombre, renombrado: false, clase: "equipo", tipo: d.tipo, donde: d.tipo === "local" ? null : (d.donde ?? null), destino: d, ids: [], equipos: [] });
      if (!v.ids.includes(d.id)) v.ids.push(d.id);
      if (!v.equipos.includes(e.nombre)) v.equipos.push(e.nombre);
    }
  // 3. Las nubes conectadas en los almacenes (para el espejo).
  for (const a of almacenesDe(equipos))
    for (const n of a.resumen?.guarda_copias?.nubes ?? []) {
      const clave = claveNube(a.id, n.nombre);
      if (!porClave.has(clave)) poner({ clave, nombre: n.nombre, nombrePorDefecto: n.nombre, renombrado: false, clase: "nube", tipo: "nube", donde: null, nube: { equipo: a, nombre: n.nombre, tipo: n.tipo }, ids: [], equipos: [a.nombre] });
    }
  // 4. El catálogo: el nombre de los que ya están y los sueltos (de red, aún sin repositorios).
  for (const c of catalogo) {
    const v = porClave.get(c.id);
    if (v) {
      v.catalogo = c;
      if (c.nombre.trim() && c.nombre.trim() !== v.nombrePorDefecto) {
        v.nombre = c.nombre.trim();
        v.renombrado = true;
      }
    } else if ((TIPOS_SUELTOS as readonly string[]).includes(c.tipo) && c.nombre.trim()) {
      // (Sin nombre: solo lleva lo marcado para la regla 3-2-1 de un destino que ya no está.)
      poner({ clave: c.id, nombre: c.nombre, nombrePorDefecto: c.nombre, renombrado: false, clase: "suelto", tipo: c.tipo, donde: c.donde ?? null, ids: [], equipos: [], catalogo: c });
    }
    // Lo demás (una zona quitada, una nube desconectada) ya no está: no se enseña.
  }
  const rango: Record<ClaseDestino, number> = { zona: 0, equipo: 1, suelto: 1, nube: 2 };
  return orden.sort(
    (a, b) =>
      rango[a.clase] - rango[b.clase] ||
      (a.zona && b.zona ? a.zona.almacen.nombre.localeCompare(b.zona.almacen.nombre) || Number(b.zona.principal) - Number(a.zona.principal) : 0) ||
      a.nombre.localeCompare(b.nombre),
  );
}

/** El nombre que se enseña de un destino de un equipo (el del catálogo, si lo tiene). */
export function nombreDestino(d: DestinoResumen | undefined, equipos: Equipo[], catalogo: DestinoCatalogo[]): string | null {
  if (!d) return null;
  const z = zonaDeDestino(d, equipos);
  const clave = z ? claveZona(z.almacen.id, z.id) : d.id;
  const c = catalogo.find((x) => x.id === clave)?.nombre.trim();
  return c || (z ? nombreZonaPorDefecto(z) : d.nombre);
}

/** Las zonas de los almacenes del cliente en las que un equipo aún no tiene destino. */
export function zonasNuevasPara(equipo: Equipo, equipos: Equipo[]): ZonaVista[] {
  const suyas = new Set((equipo.resumen?.destinos ?? []).map((d) => zonaDeDestino(d, equipos)).filter((z): z is ZonaVista => !!z).map((z) => claveZona(z.almacen.id, z.id)));
  return almacenesDe(equipos)
    .flatMap((a) => zonasDe(a).filter((z) => z.principal || admiteZonas(a)))
    .filter((z) => !suyas.has(claveZona(z.almacen.id, z.id)));
}

/** El id del destino que se crea en el equipo para una zona (`almacen-<8>` en la principal, como siempre). */
export const idDestinoZona = (almacen: string, zona: string) => (zona === PRINCIPAL ? `almacen-${almacen.slice(0, 8)}` : `almacen-${almacen.slice(0, 8)}-${zona}`);

/** El puerto que se propone para una zona nueva: el primero libre de 8002, 8004… (de dos en dos). */
export function puertoParaZona(a: Equipo): number {
  const g = a.resumen?.guarda_copias;
  const usados = new Set([g?.puerto ?? 8000, ...(g?.zonas ?? []).map((z) => z.puerto)]);
  for (let p = (g?.puerto ?? 8000) + 2; p < 65535; p += 2) if (!usados.has(p)) return p;
  return 8002;
}

/**
 * La respuesta sellada de `guarda_copias { anadir, zona }`: tiene que ser de
 * la zona pedida (un agente anterior ignoraría `zona` y daría un usuario de
 * la principal). Devuelve el error, o null si está bien.
 */
export function errorRespuestaZona(acceso: { zona?: string | null; destino?: { donde?: string } }, zona: ZonaVista): string | null {
  if (zona.principal) return acceso.zona ? "El almacén respondió con otra zona." : null;
  if (acceso.zona !== zona.id) return `${zona.almacen.nombre} no ha dado el acceso en esa zona (¿un agente anterior?): no se crea nada. Actualiza su agente.`;
  if (zona.puerto != null && puertoDe(acceso.destino?.donde) !== zona.puerto) return "La dirección que dio el almacén no es la de esa zona: no se crea nada.";
  return null;
}

/** Carpeta normalizada para comparar (Windows: sin distinguir mayúsculas). */
const normal = (p: string, windows: boolean) => {
  const t = p.trim().replace(/[\\/]+$/, "");
  return (windows ? t.toLowerCase().replace(/\//g, "\\") : t).split(windows ? "\\" : "/");
};
/** ¿Una carpeta dentro de la otra (o la misma)? Por partes, como el agente. */
export function seSolapan(a: string, b: string, windows: boolean): boolean {
  if (!a.trim() || !b.trim()) return false;
  const [x, y] = [normal(a, windows), normal(b, windows)];
  const n = Math.min(x.length, y.length);
  return x.slice(0, n).every((p, i) => p === y[i]);
}

/**
 * La carpeta de una zona nueva contra el almacén (como la comprueba el
 * agente): ni dentro de la principal ni de otra zona ni de un destino del
 * espejo (ni al revés), y con su puerto libre en el almacén.
 */
export function errorZonaNueva(a: Equipo, carpeta: string, puerto: number): string | null {
  const g = a.resumen?.guarda_copias;
  const windows = /windows/i.test(a.so);
  if (!g?.activo) return "Ese equipo no guarda copias.";
  if ((g.zonas ?? []).length >= ZONAS_MAX) return `Como mucho ${ZONAS_MAX} zonas además de la principal.`;
  if (!Number.isInteger(puerto) || puerto < 1024 || puerto > 65535) return "Elige un puerto entre 1024 y 65535 (por ejemplo, 8002).";
  if (puerto === g.puerto || (g.zonas ?? []).some((z) => z.puerto === puerto)) return `El puerto ${puerto} ya es de este almacén: elige otro (por ejemplo, ${puertoParaZona(a)}).`;
  if (g.carpeta && seSolapan(carpeta, g.carpeta, windows)) return "No puede estar dentro de la carpeta del almacén (ni al revés): elige otro disco u otra carpeta.";
  if ((g.zonas ?? []).some((z) => seSolapan(carpeta, z.carpeta, windows))) return "Se solapa con otra zona de este almacén.";
  if ((g.espejo?.destinos ?? []).some((d) => d.tipo === "carpeta" && d.carpeta && seSolapan(carpeta, d.carpeta, windows))) return "Se solapa con una carpeta del espejo: elige otra.";
  return null;
}

/** Errores de la dirección de un destino del catálogo (como los comprueba el servidor). */
export function errorDondeCatalogo(tipo: TipoSuelto, donde: string): string | null {
  const d = donde.trim();
  if (!d) return "Falta la dirección (servidor o bucket).";
  if (d.length > 300 || /\s/.test(d)) return "Sin espacios y hasta 300 caracteres.";
  const autoridad = (d.includes("://") ? d.split("://")[1] : d).split("/")[0];
  if (autoridad.includes("@") && (tipo !== "sftp" || autoridad.split("@")[0].includes(":"))) return "Sin usuario ni contraseña: las credenciales se piden al crear un repositorio y van selladas solo para ese equipo.";
  if (/^[a-z]:(?!\/\/)/i.test(d) || /^[\\/~]/.test(d)) return "Una carpeta local no va aquí: se elige al crear el repositorio en su equipo.";
  return null;
}

/** Nombre válido para un destino (como el servidor): 1–80 caracteres, sin caracteres de control. */
export const errorNombreDestino = (n: string) => (!n.trim() ? "Escribe un nombre." : n.trim().length > 80 ? "Hasta 80 caracteres." : /[\u0000-\u001f\u007f]/.test(n) ? "Sin caracteres de control." : null);

/** Un id nuevo para un destino suelto del catálogo (también será su id en los equipos que lo usen). */
export const idDestinoNuevo = () => `destino-${crypto.randomUUID().slice(0, 8)}`;

/** Lo que se ve del tipo de un destino. */
export const TEXTO_TIPO: Record<string, string> = {
  zona: "Zona de un almacén",
  rest: "Servidor de copias",
  local: "Disco o carpeta del equipo",
  s3: "S3",
  b2: "Backblaze B2",
  sftp: "SFTP",
  nube: "Nube conectada",
};
