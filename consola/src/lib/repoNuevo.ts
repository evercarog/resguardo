// «Nuevo repositorio» (en «Repositorios y destinos», en la ficha del equipo y en
// el editor de copias): todos los destinos donde un equipo puede crear uno, con
// si sirve desde ese equipo y, si no, por qué y qué hacer. Nunca se esconde uno
// sin decir por qué (docs/destinos.md, «Elegir el destino de un repositorio»).
//
// - Los almacenes del cliente (su zona principal) a los que aún no copia: lo
//   recomendado; y las otras zonas (otros discos).
// - Sus propios destinos (también una nube ya usada por él).
// - Los del catálogo que aún no tiene (B2, S3, servidor): piden sus credenciales.
// - Las nubes (tarea 4a): conectada en este equipo → se elige; conectada en
//   otro (p. ej. el almacén) → «Conectar Dropbox también en …»; en ninguno →
//   «Conectar Dropbox». Con un agente que no anuncia `repo_en_nube`: «Actualiza
//   el agente de …».
// - «Un destino nuevo…».
//
// Sin dependencias de Svelte: lo prueban los vectores (scripts/vectores-repo-nuevo.ts).
import type { DestinoCatalogo, DestinoResumen, Equipo } from "./tipos";
import { ADMITE, admite, destinosParaPasos, nubeEn, usoNubeDirecta, type Uso } from "./cadenas";
import { claveZona, destinosDelCliente, nombreDestino, nombreZonaPorDefecto, PRINCIPAL, TEXTO_TIPO, zonaDeDestino, zonasNuevasPara, type ZonaVista } from "./destinos";
import { admiteAlmacenPropio, esDeAlmacen } from "./retencion";
import { nombreTipoNube, TIPOS_NUBE } from "./espejo";

/** Lo que hace elegir cada opción. */
export type QueDestino =
  /** «Copiar en …»: el almacén da el acceso (en su zona principal o en otra). */
  | { tipo: "almacen"; almacen: Equipo; zona?: ZonaVista }
  /** Un destino que el equipo ya tiene. */
  | { tipo: "propio"; destino: DestinoResumen }
  /** Uno del catálogo que el equipo aún no tiene: se crea con su mismo id y pide sus credenciales. */
  | { tipo: "catalogo"; clave: string; tipoDestino: "b2" | "s3" | "rest"; nombre: string; donde: string }
  /** Una nube conectada en el equipo, en una carpeta suya (`crear_repositorio` con `tipo: "nube"`). */
  | { tipo: "nube"; nube: string; tipoNube: string }
  /** No se puede elegir (el motivo y la acción van en `uso`). */
  | { tipo: "ninguno" }
  | { tipo: "nuevo" };

export interface OpcionRepo {
  valor: string;
  nombre: string;
  detalle?: string;
  clase: "zona" | "nube" | "equipo" | "suelto" | "carpeta" | "nuevo";
  uso: Uso;
  que: QueDestino;
  /** Lo recomendado (un almacén de otro equipo de la oficina). */
  recomendado?: boolean;
}

/** Los almacenes en los que `equipo` puede tener un repositorio (el suyo, solo si su agente lo admite). */
export function almacenesPara(equipo: Equipo, equipos: Equipo[]): Equipo[] {
  return equipos.filter((a) => (a.id !== equipo.id || admiteAlmacenPropio(a)) && a.confirmado && a.modo !== "trasladado" && a.resumen?.guarda_copias?.activo);
}

/** El tipo de una nube conectada en un equipo («dropbox»…), o «» si no se sabe. */
const tipoNubeEn = (e: Equipo, nombre: string) => [...(e.resumen?.nubes ?? []), ...(e.resumen?.guarda_copias?.nubes ?? [])].find((n) => n.nombre === nombre)?.tipo ?? "";

/** Todas las opciones de destino para un repositorio nuevo de `equipo`, en el orden en que se enseñan. */
export function opcionesRepoNuevo(equipo: Equipo, equipos: Equipo[], catalogo: DestinoCatalogo[] = []): OpcionRepo[] {
  const l: OpcionRepo[] = [];
  const vistas = destinosDelCliente(equipos, catalogo);
  const nombreVista = (clave: string, si: string) => vistas.find((v) => v.clave === clave)?.nombre ?? si;
  const propios = equipo.resumen?.destinos ?? [];
  const almacenes = almacenesPara(equipo, equipos);

  // 1. Los almacenes a los que aún no copia (otro equipo primero: lo recomendado).
  const nuevos = almacenes.filter((a) => !propios.some((d) => esDeAlmacen(d, a))).sort((a, b) => Number(a.id === equipo.id) - Number(b.id === equipo.id));
  for (const a of nuevos) {
    const suyo = a.id === equipo.id;
    l.push({
      valor: `almacen:${a.id}`,
      nombre: suyo ? `Su propio almacén (${a.nombre})` : nombreVista(claveZona(a.id, PRINCIPAL), `Almacén ${a.nombre}`),
      detalle: suyo ? "En este mismo equipo" : "Recomendado · sin escribir direcciones ni contraseñas",
      clase: "zona",
      uso: { ok: true },
      que: { tipo: "almacen", almacen: a },
      recomendado: !suyo,
    });
  }
  // 2. Las otras zonas (otros discos) de los almacenes.
  for (const z of zonasNuevasPara(equipo, equipos).filter((z) => !z.principal && (z.almacen.id !== equipo.id || admiteAlmacenPropio(z.almacen)))) {
    l.push({
      valor: `zona:${z.almacen.id}:${z.id}`,
      nombre: nombreVista(claveZona(z.almacen.id, z.id), nombreZonaPorDefecto(z)),
      detalle: z.almacen.id === equipo.id ? "Otra zona de su propio almacén" : "Otra zona del almacén",
      clase: "zona",
      uso: { ok: true },
      que: { tipo: "almacen", almacen: z.almacen, zona: z },
    });
  }
  // 3. Sus propios destinos.
  for (const d of propios) {
    if (d.tipo === "nube") {
      const nombre = d.nube ?? d.nombre;
      const tipo = tipoNubeEn(equipo, nombre);
      const uso: Uso = nubeEn(equipo, nombre) ? usoNubeDirecta({ equipo, nombre, tipo }, equipo) : { ok: false, motivo: `«${nombre}» ya no está conectada en este equipo` };
      l.push({ valor: uso.ok ? d.id : `no:${d.id}`, nombre: d.nombre, detalle: [nombreTipoNube(tipo || "nube"), "conectada en este equipo", d.donde ? `carpeta ${d.donde}` : ""].filter(Boolean).join(" · "), clase: "nube", uso, que: uso.ok ? { tipo: "propio", destino: d } : { tipo: "ninguno" } });
      continue;
    }
    const z = zonaDeDestino(d, equipos);
    const almacen = almacenes.some((a) => esDeAlmacen(d, a)) || !!z;
    const detalle = almacen
      ? "Almacén de la oficina"
      : d.tipo === "local"
        ? d.red
          ? "Carpeta de la red"
          : d.extraible
            ? "Disco extraíble de este equipo"
            : "En este mismo equipo"
        : [TEXTO_TIPO[d.tipo] ?? d.tipo, d.donde].filter(Boolean).join(" · ");
    l.push({
      valor: d.id,
      nombre: nombreDestino(d, equipos, catalogo) ?? d.nombre,
      detalle: d.inmutable ? `${detalle} · inmutable` : detalle,
      clase: almacen ? "zona" : d.tipo === "local" ? "carpeta" : "equipo",
      uso: { ok: true },
      que: { tipo: "propio", destino: d },
    });
  }
  // 4. Los del catálogo que aún no tiene (de red y con credenciales).
  for (const v of vistas.filter((v) => v.clase === "suelto" && ["b2", "s3", "rest"].includes(v.tipo) && !propios.some((d) => d.id === v.clave))) {
    l.push({
      valor: `catalogo:${v.clave}`,
      nombre: v.nombre,
      detalle: [TEXTO_TIPO[v.tipo] ?? v.tipo, v.donde, "aún sin repositorios · pide sus credenciales"].filter(Boolean).join(" · "),
      clase: "suelto",
      uso: { ok: true },
      que: { tipo: "catalogo", clave: v.clave, tipoDestino: v.tipo as "b2" | "s3" | "rest", nombre: v.nombre, donde: v.donde ?? "" },
    });
  }
  // 5. Las nubes conectadas en algún equipo (una vez cada una, por nombre y tipo).
  const yaUsadas = new Set(propios.filter((d) => d.tipo === "nube").map((d) => (d.nube ?? d.nombre).toLowerCase()));
  const grupos = new Map<string, { nombre: string; tipo: string; en: Equipo[] }>();
  for (const v of destinosParaPasos(equipos, catalogo)) {
    if (!v.nube) continue;
    const k = `${v.nube.tipo}:${v.nube.nombre.toLowerCase()}`;
    const g = grupos.get(k) ?? { nombre: v.nube.nombre, tipo: v.nube.tipo, en: [] };
    if (!g.en.some((e) => e.id === v.nube!.equipo.id)) g.en.push(v.nube.equipo);
    grupos.set(k, g);
  }
  // Las de este equipo que no son de un almacén ni salen ya (las usa uno de sus destinos).
  for (const n of equipo.resumen?.nubes ?? []) {
    const k = `${n.tipo}:${n.nombre.toLowerCase()}`;
    if (!grupos.has(k)) grupos.set(k, { nombre: n.nombre, tipo: n.tipo, en: [equipo] });
  }
  for (const g of grupos.values()) {
    const aqui = g.en.some((e) => e.id === equipo.id) || nubeEn(equipo, g.nombre);
    if (aqui && yaUsadas.has(g.nombre.toLowerCase())) continue;
    const de = aqui ? equipo : g.en[0];
    const uso = usoNubeDirecta({ equipo: de, nombre: g.nombre, tipo: g.tipo }, equipo);
    l.push({
      valor: uso.ok ? `nube:${g.nombre}` : `no:nube:${g.nombre}`,
      nombre: g.nombre,
      detalle: `${nombreTipoNube(g.tipo)} · conectada en ${aqui ? "este equipo" : g.en.map((e) => e.nombre).join(", ")}`,
      clase: "nube",
      uso,
      que: uso.ok ? { tipo: "nube", nube: g.nombre, tipoNube: g.tipo } : { tipo: "ninguno" },
    });
  }
  // Ninguna Dropbox en el cliente: se puede conectar una aquí mismo.
  if (![...grupos.values()].some((g) => g.tipo === "dropbox")) {
    const uso: Uso = !admite(equipo, ADMITE.repoEnNube)
      ? { ok: false, motivo: `Actualiza el agente de ${equipo.nombre} para copiar directo a una nube`, accion: { tipo: "actualizar", equipo } }
      : { ok: false, motivo: "Sin conectar todavía", accion: { tipo: "conectar_nube", equipo, nube: "", tipoNube: "dropbox", texto: `Conectar Dropbox en ${equipo.nombre}` } };
    l.push({ valor: "conectar:dropbox", nombre: "Dropbox", detalle: "Directo a la nube, sin almacén de por medio", clase: "nube", uso, que: { tipo: "ninguno" } });
  }
  // 6. Uno nuevo.
  l.push({ valor: "nuevo", nombre: "Un destino nuevo…", detalle: "Disco o carpeta, servidor de copias, Backblaze B2 o S3", clase: "nuevo", uso: { ok: true }, que: { tipo: "nuevo" } });
  return l;
}

/** La opción que se propone al abrir: un almacén de otro equipo, si no el primero que sirve. */
export function opcionInicial(opciones: OpcionRepo[]): string {
  return (opciones.find((o) => o.recomendado) ?? opciones.find((o) => o.uso.ok && o.clase !== "nuevo") ?? opciones.find((o) => o.uso.ok))?.valor ?? "nuevo";
}

/**
 * La opción de la lista que corresponde a un destino de la página de destinos
 * (`clave` de `destinosDelCliente`: `zona:<almacén>:<zona>`, `nube:<equipo>:<nombre>`
 * o el id del destino), para abrir «Nuevo repositorio» con él ya elegido
 * («Usar en una copia»). Si no está, `undefined`.
 */
export function opcionDeClave(opciones: OpcionRepo[], clave: string, equipos: Equipo[], catalogo: DestinoCatalogo[] = []): string | undefined {
  const z = /^zona:([^:]+):(.+)$/.exec(clave);
  if (z) return opciones.find((o) => o.que.tipo === "almacen" && o.que.almacen.id === z[1] && (o.que.zona?.id ?? PRINCIPAL) === z[2])?.valor;
  if (clave.startsWith("nube:")) {
    const nombre = destinosParaPasos(equipos, catalogo).find((v) => v.clave === clave)?.nube?.nombre;
    if (!nombre) return undefined;
    return opciones.find((o) => o.clase === "nube" && o.nombre === nombre)?.valor ?? opciones.find((o) => o.que.tipo === "propio" && o.que.destino.nube === nombre)?.valor;
  }
  return opciones.find((o) => o.valor === clave || o.valor === `catalogo:${clave}` || o.valor === `no:${clave}`)?.valor;
}

/** ¿No es inmutable (Dropbox, Drive, SMB…)? Para avisar al elegirla. */
export const nubeNoInmutable = (tipoNube: string | undefined) => !!tipoNube && !!TIPOS_NUBE[tipoNube] && !TIPOS_NUBE[tipoNube].inmutable;

/** La carpeta dentro de la nube donde va el repositorio (como la comprueba el agente), o el error. */
export function errorCarpetaNube(c: string): string | null {
  const t = c.trim().replace(/^\/+|\/+$/g, "");
  if (!t) return "Escribe una carpeta (por ejemplo, Resguardo).";
  if (t.length > 200 || /[\u0000-\u001f\u007f]/.test(t) || t.includes(":") || t.includes("\\") || t.split("/").some((p) => p === ".." || p === "." || p === ""))
    return "Una carpeta con «/», sin «..», «\\» ni «:».";
  return null;
}

/** Id de un destino nuevo en una nube (`nube-dropbox-oficina-1a2b`). */
export const idDestinoNube = (nombre: string, sufijo = crypto.randomUUID().slice(0, 4)) =>
  `nube-${
    nombre
      .toLowerCase()
      .normalize("NFD")
      .replace(/[̀-ͯ]/g, "")
      .replace(/[^a-z0-9]+/g, "-")
      .replace(/^-|-$/g, "")
      .slice(0, 30) || "nube"
  }-${sufijo}`;

/**
 * El destino de `crear_repositorio` para la nube `nube` (conectada en el equipo) y la carpeta `carpeta`:
 * el destino que ya tiene el equipo con esa nube y esa carpeta, o uno nuevo.
 */
export function destinoNubeCuerpo(equipo: Equipo, nube: string, carpeta: string, sufijo?: string): { id: string } | { id: string; nombre: string; tipo: "nube"; nube: string; donde: string } {
  const c = carpeta.trim().replace(/^\/+|\/+$/g, "");
  const ya = equipo.resumen?.destinos?.find((d) => d.tipo === "nube" && d.nube === nube && (d.donde ?? "").replace(/^\/+|\/+$/g, "") === c);
  return ya ? { id: ya.id } : { id: idDestinoNube(nube, sufijo), nombre: nube, tipo: "nube", nube, donde: c };
}
