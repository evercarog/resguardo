// «Salud de la protección» de cada destino, compartida por el panel, el
// resumen de arriba de cada página, el esquema del destino y el asistente
// «Mejorar la protección». Las reglas están en el backend (protection.rs).
import * as api from "$lib/api";
import type { Protection, ProtectionItem, Repo } from "$lib/api";
import { health } from "$lib/status.svelte";

export const protections = $state<Record<string, Protection>>({});

/** Versión más reciente que conoce la interfaz (para saber si la copia externa va atrasada). */
export function lastSnapshotTime(repoId: string): string | null {
  const snaps = health[repoId]?.snapshots ?? [];
  return snaps.reduce<string | null>((a, s) => (!a || s.time > a ? s.time : a), null);
}

const requests: Record<string, number> = {};

/** Vuelve a calcular la protección de un destino (la última petición manda). */
export function refreshProtection(repo: Repo): Promise<Protection | null> {
  const mine = (requests[repo.id] = (requests[repo.id] ?? 0) + 1);
  return api.protectionStatus(repo.id, lastSnapshotTime(repo.id)).then(
    (p) => {
      if (requests[repo.id] === mine) protections[repo.id] = p;
      return p;
    },
    () => null,
  );
}

export type ProtectionTone = "ok" | "warn" | "bad" | "muted";

/** Como la web: «bad» si algo falla, «warn» si falta algo, «ok» si todo está bien. */
export function protectionTone(p: Protection | null | undefined): ProtectionTone {
  if (!p) return "muted";
  if (p.items.some((i) => i.state === "bad")) return "bad";
  if (p.items.some((i) => i.state !== "ok")) return "warn";
  return "ok";
}

/**
 * Orden en el que conviene arreglar lo que falta (el del asistente y el del
 * resumen): primero que las copias se hagan, luego poder recuperarlas si se
 * pierde todo, y después comprobar que se pueden recuperar.
 */
export const FIX_ORDER: ProtectionItem["id"][] = ["copias", "kit", "externa", "borrado", "restauracion", "verificacion", "retencion"];

/** Lo que falta, en el orden en que conviene arreglarlo (lo que falla, antes). */
export function pendingItems(p: Protection | null | undefined): ProtectionItem[] {
  const list = (p?.items ?? []).filter((i) => i.state !== "ok");
  const rank = (i: ProtectionItem) => (i.state === "bad" ? 0 : 1) * 100 + FIX_ORDER.indexOf(i.id);
  return list.sort((a, b) => rank(a) - rank(b));
}

/** Qué hacer con cada comprobación, como complemento de «Falta: …». */
export const FIX_PHRASE: Record<ProtectionItem["id"], string> = {
  copias: "que las copias se hagan solas",
  kit: "preparar el kit de recuperación",
  externa: "una copia externa (en la nube o en otro disco)",
  borrado: "protegerlo contra borrado",
  restauracion: "una prueba de restauración",
  verificacion: "programar la verificación",
  retencion: "definir la retención",
};
