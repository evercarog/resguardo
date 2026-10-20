// «Ver las bases de la instancia» (plan-mejoras 15u): la orden `bases_sqlserver`, que no
// cambia nada en el equipo. El equipo lee `sys.databases` y devuelve la lista sellada
// para este navegador (`responder_a`): el servidor no la ve.
// Aquí, lo que no habla con el servidor (se prueba sin él); la petición, en basesSqlPedir.ts.

export interface BaseInstancia {
  nombre: string;
  /** `ONLINE`, `RECOVERY_PENDING`, `OFFLINE`, `RESTORING`, `SUSPECT`… */
  estado: string;
  /** `master`, `tempdb`, `model` o `msdb`. */
  sistema: boolean;
}

/** El estado de una base, en palabras. */
export function estadoBase(e: string): string {
  const t: Record<string, string> = {
    ONLINE: "en línea",
    RECOVERY_PENDING: "pendiente de recuperación",
    OFFLINE: "sin conexión",
    RESTORING: "restaurándose",
    RECOVERING: "recuperándose",
    SUSPECT: "dañada",
    EMERGENCY: "en modo de emergencia",
  };
  return t[e] ?? e.toLowerCase().replaceAll("_", " ");
}

/** Las que se pueden elegir: de usuario (y `master` y `msdb`), nunca `tempdb` ni `model`. */
export const elegibles = (bs: BaseInstancia[]) => bs.filter((b) => !b.sistema || ["master", "msdb"].includes(b.nombre.toLowerCase()));

/** Lee la lista que llega en el detalle de la orden (sellada para esta consola). */
export function leerLista(datos: unknown): BaseInstancia[] {
  const bases = (datos as { bases?: unknown })?.bases;
  if (!Array.isArray(bases)) return [];
  return bases
    .filter((b): b is BaseInstancia => !!b && typeof b.nombre === "string" && typeof b.estado === "string")
    .map((b) => ({ nombre: b.nombre, estado: b.estado, sistema: b.sistema === true }));
}
