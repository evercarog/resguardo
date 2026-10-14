// «Nubes conectadas» de un equipo (plan 0.7.26, 1.1; docs/destinos.md
// «Desconectar una nube»): en la página de cada equipo, no solo en los
// almacenes. Lo que se enseña y lo que dice «Desconectar», sin nada de Svelte
// (lo prueban los vectores de scripts/vectores-nubes.ts).
//
// - Las nubes vienen de `resumen.nubes` (cualquier equipo, tarea 4a) y de
//   `resumen.guarda_copias.nubes` (almacenes): se juntan por nombre.
// - Qué la usa (lo que dice el resumen): repositorios en un destino «nube» con
//   ese nombre, copias externas o derivadas a ese destino y los espejos (del almacén y
//   del propio equipo, cada uno por su nombre, también los que están en pausa).
//   Con un agente con `admite: "nube_revocar"`, si algo la usa el equipo no deja
//   desconectarla: la consola lo dice antes y no ofrece el botón.
// - Con un agente anterior, desconectar solo olvida el permiso en el equipo: el
//   aviso de que sigue vivo en Dropbox y cómo quitarlo (desconecta todos los equipos).
import type { Equipo } from "./tipos";

/** Lo que dice el agente que anula el permiso en el proveedor al desconectar. */
export const ADMITE_REVOCAR = "nube_revocar";

export const admiteRevocar = (e: Pick<Equipo, "resumen"> | null | undefined) => !!e?.resumen?.admite?.includes(ADMITE_REVOCAR);

export interface NubeDelEquipo {
  nombre: string;
  tipo: string;
  /** Qué la usa en este equipo («el repositorio «Contabilidad»», «el espejo de este almacén»…). */
  usos: string[];
}

/** Una desconectada cuyo permiso aún no se pudo anular (el agente lo reintenta). */
export interface NubePorAnular {
  nombre: string;
  tipo: string;
  desde?: string | null;
  hasta?: string | null;
}

/** Qué del equipo usa la nube `nombre` (según su resumen). */
export function usosDeNube(e: Pick<Equipo, "resumen"> | null | undefined, nombre: string): string[] {
  const r = e?.resumen;
  if (!r) return [];
  const usos: string[] = [];
  const destinos = (r.destinos ?? []).filter((d) => d.tipo === "nube" && d.nube === nombre);
  const nombres = new Set(destinos.map((d) => d.nombre));
  const ids = new Set(destinos.map((d) => d.id));
  const en = (destino?: string | null, id?: string | null) => (!!id && ids.has(id)) || (!!destino && nombres.has(destino));
  for (const repo of r.repositorios ?? []) {
    if (en(repo.destino)) usos.push(`el repositorio «${repo.nombre}»`);
    if (repo.externa && en(repo.externa.destino, repo.externa.destino_id)) usos.push(`la copia derivada de «${repo.nombre}»`);
    if ((repo.derivadas ?? []).some((d) => en(d.destino, d.destino_id))) usos.push(`una copia derivada de «${repo.nombre}»`);
  }
  usos.push(...usosEspejoDeNube(e, nombre));
  return usos;
}

/**
 * Plan 0.8.0 (B2): los espejos que usan la nube `nombre` en este equipo, por su nombre:
 * los trabajos del almacén y los del propio equipo, también los que están en pausa
 * (el resumen de los destinos de antes solo trae los activos). Sin trabajos (un agente
 * anterior), el espejo de antes.
 */
export function usosEspejoDeNube(e: Pick<Equipo, "resumen"> | null | undefined, nombre: string): string[] {
  const r = e?.resumen;
  if (!r) return [];
  const usa = (t: { adonde: { tipo: string; nube?: string | null } }) => t.adonde?.tipo === "nube" && t.adonde.nube === nombre;
  const trabajos = r.guarda_copias?.espejo?.trabajos;
  const usos: string[] = [];
  if (trabajos?.length) {
    for (const t of trabajos.filter(usa)) usos.push(`el espejo «${t.nombre.trim() || `a «${nombre}»`}»${t.activo ? "" : " (en pausa)"}`);
  } else if (r.guarda_copias?.espejo?.destinos?.some((d) => d.tipo === "nube" && d.nube === nombre)) usos.push("el espejo de este almacén");
  for (const t of (r.espejo_equipo?.trabajos ?? []).filter(usa)) usos.push(`el espejo «${t.nombre.trim() || `a «${nombre}»`}» de este equipo${t.activo ? "" : " (en pausa)"}`);
  return usos;
}

/** Las nubes conectadas en el equipo (de cualquier sitio del resumen, una vez cada una), con lo que las usa. */
export function nubesDelEquipo(e: Pick<Equipo, "resumen"> | null | undefined): NubeDelEquipo[] {
  const vistas = new Map<string, string>();
  for (const n of [...(e?.resumen?.nubes ?? []), ...(e?.resumen?.guarda_copias?.nubes ?? [])]) {
    if (n?.nombre && !vistas.has(n.nombre)) vistas.set(n.nombre, n.tipo ?? "");
  }
  return [...vistas].map(([nombre, tipo]) => ({ nombre, tipo, usos: usosDeNube(e, nombre) }));
}

/** Las desconectadas con el permiso aún por anular (agente con `nube_revocar`). */
export function nubesPorAnular(e: Pick<Equipo, "resumen"> | null | undefined): NubePorAnular[] {
  const l = e?.resumen?.nubes_por_anular;
  return Array.isArray(l) ? l.filter((x) => !!x && typeof x.nombre === "string") : [];
}

/** Lo que pasa en el proveedor al desconectar (para el diálogo). */
export function queHaceAlDesconectar(e: Pick<Equipo, "resumen"> | null | undefined, tipo: string): string {
  if (!admiteRevocar(e)) {
    if (tipo === "dropbox")
      return "Este equipo tiene un agente anterior: solo olvida el permiso. El permiso sigue vivo en Dropbox: quítalo desde la web de Dropbox → Aplicaciones conectadas (ojo: desconecta todos los equipos).";
    if (tipo === "drive") return "Este equipo tiene un agente anterior: solo olvida el permiso. Quítalo también en tu cuenta de Google.";
    return "Si ya no la usa nadie, borra también esa clave o ese usuario en el servicio.";
  }
  if (tipo === "dropbox") return "El equipo borra sus credenciales y anula ese permiso en Dropbox (solo el de este equipo). Si ahora no puede, lo reintenta durante 7 días.";
  if (tipo === "drive") return "El equipo borra sus credenciales. El permiso de Google se quita desde tu cuenta de Google (afecta a todos los equipos que la usan).";
  return "El equipo borra sus credenciales. La clave o la contraseña siguen valiendo en el proveedor: anúlala allí si ya no la usa nadie.";
}

/**
 * Si el equipo no dejará desconectarla (agente con `nube_revocar` y algo la usa):
 * el porqué. Con un agente anterior, `null` (él decide, como siempre).
 */
export function motivoNoDesconectar(e: Pick<Equipo, "resumen"> | null | undefined, n: NubeDelEquipo): string | null {
  // Plan 0.8.0 (B2): un espejo con trabajos (agente 0.7.26) que la usa bloquea siempre, también en pausa.
  const conTrabajos = !!e?.resumen?.guarda_copias?.espejo?.trabajos?.length || !!e?.resumen?.espejo_equipo?.trabajos?.length;
  if (!n.usos.length || (!admiteRevocar(e) && !(conTrabajos && usosEspejoDeNube(e, n.nombre).length))) return null;
  const lista = n.usos.length > 3 ? `${n.usos.slice(0, 3).join(", ")} y ${n.usos.length - 3} más` : n.usos.join(", ");
  return `La usa ${lista}. Quítala antes de ahí para poder desconectarla.`;
}
