// Acciones que se pueden lanzar desde cualquier sitio (la paleta Ctrl+K):
// abren los mismos diálogos que sus botones de siempre («Copiar ahora»,
// «Nuevo repositorio»). Aquí solo se dice cuál abrir; la orden la manda el
// diálogo, como siempre.
import type { DestinoResumen, Equipo } from "./tipos";

export const acciones = $state({
  /** «Copiar ahora» de una copia (OrdenDialog). */
  copiar: null as { equipo: Equipo; repo: string; copia: string; nombre: string } | null,
  /** «Nuevo repositorio» (NuevoRepositorio). */
  nuevoRepo: false,
});

/** Los destinos del cliente, sin repetir (para «Nuevo repositorio»), como en Repositorios y destinos. */
export function destinosDe(equipos: Equipo[]): DestinoResumen[] {
  const m = new Map<string, DestinoResumen>();
  for (const e of equipos) for (const d of e.resumen?.destinos ?? []) if (!m.has(d.id)) m.set(d.id, d);
  return [...m.values()].sort((a, b) => a.nombre.localeCompare(b.nombre));
}
