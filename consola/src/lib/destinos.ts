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
import { NOMBRE_TIPO_DESTINO } from "./direccion";

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
  /**
   * Plan 0.8.0 (B2, 14r): otras entradas del catálogo que son este mismo destino (mismo
   * bucket o mismo servidor, `identidadDeVista`), unidas aquí: un destino es uno solo.
   */
  alias?: DestinoCatalogo[];
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
  // 5. Plan 0.8.0 (B2, 14r): los que son el mismo destino (mismo bucket o mismo servidor), una sola ficha.
  const unidos = unirIguales(orden);
  orden.length = 0;
  orden.push(...unidos);
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

/** Lo que se ve del tipo de un destino: la tabla única de `direccion.ts` (G3). */
export const TEXTO_TIPO: Record<string, string> = NOMBRE_TIPO_DESTINO;

// --- Plan 0.8.0 (B2 y B3; plan-mejoras 14b y 14r): un destino es uno solo ---
//
// Regla para saber si dos destinos son el mismo: el **mismo tipo de almacenamiento y el
// mismo bucket** (B2: el bucket, que es único en todo B2; S3: el servidor y el bucket) o
// la **misma dirección de servidor** (servidor de copias: máquina y puerto; SFTP: la
// máquina). Una nube conectada por rclone (B2 o S3) es el mismo destino que la entrada
// del catálogo o el destino de un equipo con ese bucket. Dropbox, Drive, SMB y WebDAV
// no se unen (no hay un bucket que los identifique). Las zonas de los almacenes ya son
// una por zona.

/** Las partes de una dirección de destino (sin credenciales): servidor, bucket y la ruta que sigue. */
export function partesDonde(tipo: string, donde: string | null | undefined): { servidor: string; bucket: string; ruta: string } | null {
  const d = (donde ?? "").trim().replace(/^(?:s3|b2|rest|sftp):(?!\/\/)/i, "");
  if (!d) return null;
  if (tipo === "b2") {
    // «mi-bucket», «mi-bucket:ruta» (como lo escribe el agente) o «mi-bucket/ruta» (rclone).
    const m = /^([^:/]+)[:/]?(.*)$/.exec(d);
    return m ? { servidor: "", bucket: m[1].toLowerCase(), ruta: m[2].replace(/^\/+|\/+$/g, "") } : null;
  }
  if (tipo === "s3") {
    const p = d.replace(/^https?:\/\//i, "").split("/").filter(Boolean);
    if (!p.length) return null;
    // El primero es el servidor si parece una máquina (lleva punto o puerto); si no, es el bucket.
    const conServidor = p.length > 1 && /[.:]/.test(p[0]);
    const [servidor, bucket, ...ruta] = conServidor ? p : ["", ...p];
    return { servidor: servidor.toLowerCase(), bucket: (bucket ?? "").toLowerCase(), ruta: ruta.join("/") };
  }
  if (tipo === "rest") {
    const m = /^(?:https?:\/\/)?([^/]+)(\/.*)?$/i.exec(d);
    return m ? { servidor: m[1].toLowerCase(), bucket: "", ruta: (m[2] ?? "").replace(/^\/+|\/+$/g, "") } : null;
  }
  if (tipo === "sftp") {
    const m = /^(?:[^@/]+@)?([^:/]+)(?::(.*))?$/.exec(d);
    return m ? { servidor: m[1].toLowerCase(), bucket: "", ruta: (m[2] ?? "").replace(/^\/+|\/+$/g, "") } : null;
  }
  return null;
}

/** La identidad de un destino de red (o null): «b2|mi-bucket», «s3|s3.ejemplo.com|mi-bucket», «rest|nas:8000», «sftp|nas». */
export function identidadDestino(tipo: string, donde: string | null | undefined): string | null {
  const p = partesDonde(tipo, donde);
  if (!p) return null;
  if (tipo === "b2") return p.bucket ? `b2|${p.bucket}` : null;
  if (tipo === "s3") return p.bucket ? `s3|${p.servidor}|${p.bucket}` : null;
  if (tipo === "rest" || tipo === "sftp") return p.servidor ? `${tipo}|${p.servidor}` : null;
  return null;
}

/** Los trabajos (y destinos de antes) del espejo de un equipo que van a la nube `nombre`. */
function espejosANube(e: Equipo, nombre: string): { carpeta: string }[] {
  const g = e.resumen?.guarda_copias?.espejo;
  const t = [...(g?.trabajos ?? []), ...(e.resumen?.espejo_equipo?.trabajos ?? [])].filter((x) => x.adonde?.tipo === "nube" && x.adonde.nube === nombre).map((x) => ({ carpeta: x.adonde.carpeta }));
  const d = (g?.trabajos?.length ? [] : (g?.destinos ?? [])).filter((x) => x.tipo === "nube" && x.nube === nombre).map((x) => ({ carpeta: x.carpeta ?? "" }));
  return [...t, ...d];
}

/** Tipos de nube con bucket (o recurso compartido) delante de la carpeta: B2, S3 y SMB. */
export const nubeConBucket = (tipo: string | null | undefined) => ["b2", "s3", "smb"].includes(tipo ?? "");

/**
 * El bucket y la carpeta base de una nube conectada: lo que recuerda el catálogo
 * (su entrada o la de otro destino con el mismo bucket) o, si no, el bucket que ya
 * usan sus espejos (la primera parte de su carpeta). `deducido`: sacado de los espejos.
 */
export function bucketDeNube(v: Pick<DestinoVista, "nube" | "catalogo" | "alias" | "destino">): { bucket: string | null; base: string | null; deducido: boolean } {
  const n = v.nube;
  if (!n) return { bucket: null, base: null, deducido: false };
  const cats = [v.catalogo, ...(v.alias ?? [])].filter((c): c is DestinoCatalogo => !!c);
  const base = cats.map((c) => c.carpeta?.trim()).find((c) => !!c) ?? null;
  if (!nubeConBucket(n.tipo)) return { bucket: null, base, deducido: false };
  for (const c of cats) {
    if (n.tipo === "smb") {
      if (c.tipo === "nube" && c.donde?.trim()) return { bucket: c.donde.trim(), base, deducido: false };
      continue;
    }
    // En la entrada de la nube, `donde` es el bucket (S3: «servidor/bucket»); en la de un B2 o S3 del catálogo, su dirección.
    const p = partesDonde(c.tipo === "nube" ? n.tipo : c.tipo, c.donde);
    if (p?.bucket) return { bucket: p.bucket, base, deducido: false };
  }
  const del = v.destino && (v.destino.tipo === "b2" || v.destino.tipo === "s3") ? partesDonde(v.destino.tipo, v.destino.donde) : null;
  if (del?.bucket) return { bucket: del.bucket, base, deducido: false };
  const primera = espejosANube(n.equipo, n.nombre)
    .map((x) => x.carpeta.trim().replace(/^\/+/, "").split("/")[0])
    .find((x) => !!x);
  return { bucket: primera ?? null, base, deducido: !!primera };
}

/** La identidad de una nube conectada (B2 o S3 con su bucket), para unirla con el catálogo. */
export function identidadNube(v: Pick<DestinoVista, "nube" | "catalogo" | "alias" | "destino">): string | null {
  if (!v.nube || (v.nube.tipo !== "b2" && v.nube.tipo !== "s3")) return null;
  const { bucket } = bucketDeNube(v);
  if (!bucket) return null;
  if (v.nube.tipo === "b2") return `b2|${bucket.toLowerCase()}`;
  const servidor = [v.catalogo, ...(v.alias ?? [])].map((c) => (c ? (partesDonde("s3", c.donde)?.servidor ?? "") : "")).find((x) => !!x) ?? "";
  return `s3|${servidor}|${bucket.toLowerCase()}`;
}

/** La identidad de una vista (o null si no se une con nada). */
export function identidadDeVista(v: DestinoVista): string | null {
  if (v.zona) return null;
  if (v.nube && v.clase === "nube") return identidadNube(v);
  if (v.clase === "suelto") return identidadDestino(v.tipo, v.catalogo?.donde ?? v.donde);
  if (v.destino) return identidadDestino(v.destino.tipo, v.destino.donde);
  return null;
}

/** Junta en una sola vista los destinos que son el mismo (`identidadDeVista`), sin perder nada. */
function unirIguales(l: DestinoVista[]): DestinoVista[] {
  const primero = new Map<string, DestinoVista>();
  // Mandan, por este orden, el destino de un equipo, la nube conectada y la entrada suelta.
  const prioridad = (v: DestinoVista) => (v.clase === "equipo" ? 0 : v.clase === "nube" ? 1 : 2);
  const quitados = new Set<DestinoVista>();
  for (const v of [...l].sort((a, b) => prioridad(a) - prioridad(b))) {
    const id = identidadDeVista(v);
    const p = id ? primero.get(id) : undefined;
    if (!id || !p) {
      if (id) primero.set(id, v);
      continue;
    }
    // `v` se une a `p`.
    quitados.add(v);
    for (const x of v.ids) if (!p.ids.includes(x)) p.ids.push(x);
    for (const x of v.equipos) if (!p.equipos.includes(x)) p.equipos.push(x);
    p.nube ??= v.nube;
    p.destino ??= v.destino;
    p.donde ??= v.donde;
    if (v.catalogo) {
      if (!p.catalogo) p.catalogo = v.catalogo;
      else (p.alias ??= []).push(v.catalogo);
      // Lo marcado para la regla 3-2-1 (p. ej. «Inmutable 30 días» en el catálogo) vale para todos.
      if (!p.catalogo.atributos && v.catalogo.atributos) p.catalogo = { ...p.catalogo, atributos: v.catalogo.atributos };
    }
    for (const a of v.alias ?? []) (p.alias ??= []).push(a);
    // El nombre puesto por una persona gana al de por defecto.
    if (!p.renombrado && (v.renombrado || v.clase === "suelto")) {
      p.nombre = v.nombre;
      p.renombrado = true;
    }
  }
  return l.filter((v) => !quitados.has(v));
}

/** La entrada del catálogo de red (B2, S3 o servidor de copias, con dirección) de una vista, si la tiene (también unida). */
export const catalogoDeRed = (v: Pick<DestinoVista, "catalogo" | "alias">): DestinoCatalogo | undefined =>
  [v.catalogo, ...(v.alias ?? [])].find((c): c is DestinoCatalogo => !!c && ["b2", "s3", "rest"].includes(c.tipo) && !!c.donde?.trim());

/** Todas las claves con las que se llega a una vista (la suya y las de lo unido a ella). */
export const clavesDe = (v: DestinoVista) => [v.clave, ...(v.catalogo && v.catalogo.id !== v.clave ? [v.catalogo.id] : []), ...(v.alias ?? []).map((a) => a.id)];

/** «Ya tienes este bucket como “…”» al crear un destino (o null). */
export function avisoRepetido(vistas: DestinoVista[], tipo: string, donde: string): string | null {
  const id = identidadDestino(tipo, donde);
  if (!id) return null;
  const v = vistas.find((x) => identidadDeVista(x) === id);
  if (!v) return null;
  return `Ya tienes ${tipo === "rest" || tipo === "sftp" ? "este servidor" : "este bucket"} como «${v.nombre}»: mejor usa ese que crear otro.`;
}

/** Une las partes de una carpeta (sin «/» de más): «mi-bucket», «Resguardo», «oficina» → «mi-bucket/Resguardo/oficina». */
export const unirCarpeta = (...partes: (string | null | undefined)[]) =>
  partes
    .map((p) => (p ?? "").trim().replace(/^\/+|\/+$/g, ""))
    .filter(Boolean)
    .join("/");

/** Si una carpeta de espejo ya guardada empieza por ese bucket y esa base, lo que va detrás (o null si no). */
export function subcarpetaDe(carpeta: string, bucket: string | null, base: string | null): string | null {
  const c = unirCarpeta(carpeta);
  const pre = unirCarpeta(bucket, base);
  if (!pre) return c;
  if (c.toLowerCase() === pre.toLowerCase()) return "";
  return c.toLowerCase().startsWith(`${pre.toLowerCase()}/`) ? c.slice(pre.length + 1) : null;
}

/** El error de una subcarpeta o carpeta base (como la comprueba el servidor: relativa, con «/», sin «..» ni «:»). */
export function errorSubcarpeta(c: string): string | null {
  const t = c.trim().replace(/^\/+|\/+$/g, "");
  if (!t) return null;
  if (t.length > 200) return "Hasta 200 caracteres.";
  if (/[\\:*?"<>|\u0000-\u001f]/.test(t)) return "Con «/» y sin «\\», «:» ni caracteres raros.";
  if (t.split("/").some((p) => !p || p === "." || p === ".." || p.trim() !== p)) return "Sin «..», partes vacías ni espacios al principio o al final.";
  return null;
}

/** El error de un bucket (B2 y S3: letras minúsculas, cifras, puntos y guiones; un recurso de SMB: sin «/» ni «\\»). */
export function errorBucket(tipo: string, b: string): string | null {
  const t = b.trim();
  if (!t) return "Falta.";
  if (tipo === "smb") return /^[^\\/:*?"<>|\u0000-\u001f]{1,80}$/.test(t) ? null : "Solo el nombre del recurso compartido, sin «/».";
  return /^[a-z0-9][a-z0-9.-]{1,61}[a-z0-9]$/.test(t) ? null : "Solo letras minúsculas, cifras, puntos y guiones (de 3 a 63).";
}

/**
 * La dirección para el equipo de un destino del catálogo con su carpeta base (al crear
 * un repositorio allí): B2 «mi-bucket:Resguardo» (como la lee el agente), S3
 * «servidor/mi-bucket/Resguardo»; un servidor de copias, tal cual.
 */
export function dondeConBase(tipo: string, donde: string, base: string | null | undefined): string {
  const b = unirCarpeta(base);
  const d = donde.trim().replace(/\/+$/, "");
  if (!b || !d) return d;
  if (tipo === "b2") return d.includes(":") ? `${d}/${b}` : `${d}:${b}`;
  if (tipo === "s3") return `${d}/${b}`;
  return d;
}

/** Lo que usa un destino, en palabras (para «Quitar» y «Desconectar»). */
export interface UsosVista {
  repos: string[];
  copias: string[];
  espejos: string[];
}
export function usosDeVista(v: DestinoVista, equipos: Equipo[]): UsosVista {
  const out: UsosVista = { repos: [], copias: [], espejos: [] };
  for (const e of equipos) {
    const destinos = (e.resumen?.destinos ?? []).filter((d) => v.ids.includes(d.id));
    if (!destinos.length) continue;
    const ids = new Set(destinos.map((d) => d.id));
    const nombres = new Set(destinos.map((d) => d.nombre));
    const en = (id?: string | null, nombre?: string | null) => (!!id && ids.has(id)) || (!id && !!nombre && nombres.has(nombre));
    for (const r of e.resumen?.repositorios ?? []) {
      if (ids.has(r.destino) || nombres.has(r.destino)) out.repos.push(`«${r.nombre}» (${e.nombre})`);
      if (r.externa && en(r.externa.destino_id, r.externa.destino)) out.copias.push(`la copia derivada de «${r.nombre}» (${e.nombre})`);
      for (const d of r.derivadas ?? []) if (en(d.destino_id, d.destino)) out.copias.push(`una copia derivada de «${r.nombre}» (${e.nombre})`);
    }
  }
  if (v.nube) {
    const e = v.nube.equipo;
    const g = e.resumen?.guarda_copias?.espejo;
    const usa = (t: { adonde: { tipo: string; nube?: string | null } }) => t.adonde?.tipo === "nube" && t.adonde.nube === v.nube!.nombre;
    for (const t of [...(g?.trabajos ?? []), ...(e.resumen?.espejo_equipo?.trabajos ?? [])].filter(usa)) out.espejos.push(`«${t.nombre.trim() || v.nube.nombre}»${t.activo ? "" : " (en pausa)"}`);
    if (!g?.trabajos?.length && g?.destinos?.some((d) => d.tipo === "nube" && d.nube === v.nube!.nombre)) out.espejos.push(`el de ${e.nombre}`);
  }
  if (v.zona) {
    const z = v.zona;
    for (const t of z.almacen.resumen?.guarda_copias?.espejo?.trabajos ?? []) if (t.adonde?.tipo === "zona" && t.adonde.carpeta === z.id) out.espejos.push(`«${t.nombre}»`);
  }
  return out;
}

/** «Lo usan 3 espejos: «A», «B» y «C».» (o null si nada lo usa). */
export function textoUsos(u: UsosVista): string | null {
  const lista = (l: string[]) => (l.length > 3 ? `${l.slice(0, 3).join(", ")} y ${l.length - 3} más` : l.length > 1 ? `${l.slice(0, -1).join(", ")} y ${l[l.length - 1]}` : l[0]);
  const partes: string[] = [];
  if (u.repos.length) partes.push(u.repos.length === 1 ? `el repositorio ${u.repos[0]}` : `${u.repos.length} repositorios: ${lista(u.repos)}`);
  if (u.espejos.length) partes.push(u.espejos.length === 1 ? `el espejo ${u.espejos[0]}` : `${u.espejos.length} espejos: ${lista(u.espejos)}`);
  if (u.copias.length) partes.push(lista(u.copias));
  if (!partes.length) return null;
  const n = u.repos.length + u.espejos.length + u.copias.length;
  return `${n === 1 ? "Lo usa" : "Lo usan"} ${partes.join("; ")}.`;
}

/** Lo mismo, corto, para el botón apagado («Lo usan 3 espejos», «Lo usan 2 repositorios y 1 espejo»). */
export function textoUsosCorto(u: UsosVista): string | null {
  const n = (k: number, uno: string, varios: string) => (k === 1 ? `1 ${uno}` : `${k} ${varios}`);
  const partes = [
    u.repos.length ? n(u.repos.length, "repositorio", "repositorios") : "",
    u.espejos.length ? n(u.espejos.length, "espejo", "espejos") : "",
    u.copias.length ? n(u.copias.length, "copia", "copias") : "",
  ].filter(Boolean);
  if (!partes.length) return null;
  const total = u.repos.length + u.espejos.length + u.copias.length;
  return `${total === 1 ? "Lo usa" : "Lo usan"} ${partes.length > 1 ? `${partes.slice(0, -1).join(", ")} y ${partes[partes.length - 1]}` : partes[0]}`;
}
