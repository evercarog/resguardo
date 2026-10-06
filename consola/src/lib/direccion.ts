// Dirección de un repositorio que ya existe (api-servidor.md §5, v1.14): lo
// que escribe la persona (como en la app de escritorio) partido en el destino
// y la carpeta del repositorio dentro de él, y los cuerpos de las órdenes.
// Sin dependencias del navegador: lo prueban los vectores (scripts/vectores.ts).

export type TipoExistente = "rest" | "local" | "b2" | "s3" | "sftp";

export const ETIQUETA_TIPO_EXISTENTE: Record<TipoExistente, string> = {
  rest: "Servidor de copias",
  local: "Disco o carpeta del equipo",
  b2: "Backblaze B2",
  s3: "S3 compatible",
  sftp: "SFTP",
};

/** Lo que se escribe para llegar a un repositorio que ya existe. */
export interface RepoExistente {
  tipo: TipoExistente;
  /** La dirección completa del repositorio, como en la app de escritorio. */
  direccion: string;
  usuario: string;
  secreto: string;
  ca: string;
  contrasena: string;
}

export const repoExistenteVacio = (): RepoExistente => ({ tipo: "rest", direccion: "", usuario: "", secreto: "", ca: "", contrasena: "" });

/**
 * Parte la dirección completa del repositorio en su destino y su carpeta (el
 * agente guarda el destino y, aparte, la ruta dentro de él):
 *  - `http://192.168.1.30:8001/Siigo` → `http://192.168.1.30:8001` + `Siigo`
 *  - `rest:https://nas:8000/ana/portatil/` → `https://nas:8000/ana` + `portatil`
 *  - `D:\Copias\Siigo` → `D:\Copias` + `Siigo`
 *  - `b2:cubo:siigo` o `cubo:siigo` → `cubo` + `siigo`; `cubo:a/b` → `cubo:a` + `b`
 *  - `https://nas:8000/` (un solo repositorio en la raíz) → igual + `""`
 */
export function partirDireccion(tipo: TipoExistente, direccion: string): { donde: string; ruta: string } {
  let d = direccion.trim();
  const prefijo = `${tipo}:`;
  if (d.toLowerCase().startsWith(prefijo)) d = d.slice(prefijo.length);
  if (tipo === "local") {
    const s = d.replace(/[\\/]+$/, "");
    const i = Math.max(s.lastIndexOf("\\"), s.lastIndexOf("/"));
    // «D:\Siigo» → «D:\» + «Siigo»; «/srv/siigo» → «/srv» + «siigo».
    if (i < 0) return { donde: s, ruta: "" };
    const donde = s.slice(0, i) || "/";
    return { donde: /^[A-Za-z]:$/.test(donde) ? `${donde}\\` : donde, ruta: s.slice(i + 1) };
  }
  if (tipo === "rest") {
    const m = /^(https?:\/\/[^/]+)(\/.*)?$/i.exec(d);
    if (!m) return { donde: d, ruta: "" };
    const camino = (m[2] ?? "").replace(/\/+$/, "");
    const i = camino.lastIndexOf("/");
    return i < 0 ? { donde: m[1], ruta: "" } : { donde: m[1] + camino.slice(0, i), ruta: camino.slice(i + 1) };
  }
  if (tipo === "b2") {
    const s = d.replace(/\/+$/, "");
    const dos = s.indexOf(":");
    if (dos < 0) return { donde: s, ruta: "" };
    const cubo = s.slice(0, dos);
    const camino = s.slice(dos + 1).replace(/^\/+/, "");
    const i = camino.lastIndexOf("/");
    return i < 0 ? { donde: cubo, ruta: camino } : { donde: `${cubo}:${camino.slice(0, i)}`, ruta: camino.slice(i + 1) };
  }
  // s3 y sftp: lo último de la ruta.
  const s = d.replace(/\/+$/, "");
  const i = s.lastIndexOf("/");
  return i < 0 ? { donde: s, ruta: "" } : { donde: s.slice(0, i), ruta: s.slice(i + 1) };
}

/** ¿Está todo lo que hace falta para probarlo? */
export const repoExistenteCompleto = (r: RepoExistente) => !!r.direccion.trim() && !!r.contrasena;

/** El destino (sin la ruta) para el cuerpo de la orden; el usuario y la contraseña, solo si hay. */
export function destinoCuerpo(r: RepoExistente, extra: { id?: string; nombre?: string } = {}) {
  const { donde } = partirDireccion(r.tipo, r.direccion);
  return {
    ...extra,
    tipo: r.tipo,
    donde,
    ...(r.tipo !== "local" && r.usuario.trim() ? { usuario: r.usuario.trim() } : {}),
    ...(r.tipo !== "local" && r.secreto ? { secreto: r.secreto } : {}),
    ...(r.tipo === "rest" && r.ca.trim() ? { ca_pem: r.ca.trim() } : {}),
  };
}

/** El origen de `copiar_historial` y de `crear_repositorio.parametros_de`. */
export function origenCuerpo(r: RepoExistente) {
  return { destino: destinoCuerpo(r), ruta: partirDireccion(r.tipo, r.direccion).ruta, contrasena: r.contrasena };
}

/** Un id para el agente (letras, números y guiones) a partir de un nombre. */
export const idDe = (nombre: string, base = "repositorio") =>
  `${
    nombre
      .normalize("NFD")
      .replace(/[̀-ͯ]/g, "")
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, "-")
      .replace(/^-|-$/g, "")
      .slice(0, 40) || base
  }-${crypto.randomUUID().slice(0, 4)}`;

/**
 * El usuario de un equipo en un almacén («Este equipo guarda copias»): lo
 * último de la dirección de su destino, `https://ip:puerto/<usuario>/` (como
 * la da `guarda_copias { anadir }`). Vacío si no tiene esa forma.
 */
export function usuarioEnAlmacen(donde?: string | null): string {
  const m = /^(?:rest:)?https?:\/\/[^/]+\/([^/]+)\/?$/i.exec((donde ?? "").trim());
  return m ? decodeURIComponent(m[1]) : "";
}

/**
 * Dónde tiene que estar, en el equipo que guarda copias, la carpeta de un
 * repositorio para que el equipo cliente la vea: `<carpeta del almacén>/<usuario>/<nombre>`
 * (rest-server con `--private-repos`), con las barras del sistema del almacén.
 */
export function rutaEnAlmacen(carpeta: string, usuario: string, nombre: string): string {
  const windows = /^[A-Za-z]:/.test(carpeta) || carpeta.includes("\\");
  const sep = windows ? "\\" : "/";
  const base = carpeta.trim().replace(/[\\/]+$/, "");
  return [base, usuario, nombre].filter((x) => x !== "").join(sep);
}
