// 0.8.1 (D8, plan-mejoras 15kk): apagar o hibernar el equipo al terminar una copia.
import type { Equipo } from "./tipos";

export const ADMITE_APAGAR = "apagar_al_terminar";

/**
 * ¿Es un servidor? (Un almacén, un Windows Server, o con una etiqueta «servidor».) Ahí
 * apagar al terminar solo se ofrece con confirmación expresa.
 */
export function esServidor(e: Pick<Equipo, "rol" | "so" | "etiquetas" | "resumen">): boolean {
  return e.rol === "almacenamiento" || !!e.resumen?.guarda_copias?.activo || /server/i.test(e.so ?? "") || (e.etiquetas ?? []).some((t) => /^servidor(es)?$/i.test(t.trim()));
}

/** «se apaga» / «hiberna». */
export const textoAlTerminar = (m: string | null | undefined) => (m === "apagar" ? "al terminar, apaga el equipo" : m === "hibernar" ? "al terminar, hiberna el equipo" : null);
