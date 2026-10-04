// Vínculo de este equipo con Resguardo Web, compartido por Ajustes (donde se
// cambia) y la tarjeta de Estado (que solo lo muestra).
import * as api from "$lib/api";
import type { WebInfo } from "$lib/api";

export const web = $state<{ info: WebInfo | null; error: string }>({ info: null, error: "" });

export async function refreshWeb() {
  try {
    web.info = await api.webInfo();
    web.error = "";
  } catch (e) {
    web.error = String(e);
  }
}

/** Vinculado y sin revocar. */
export const webLinked = () => !!web.info?.link && !web.info.link.revoked;
