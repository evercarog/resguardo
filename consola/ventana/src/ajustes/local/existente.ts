// Repositorios que ya existen, en modo local (sin consola): «Usar uno que ya
// existe» (adoptar_repositorio) y «Traer historial» (copiar_historial), por el
// canal local con la clave, como esas órdenes desde la consola. La dirección y
// los cuerpos, los de la consola ($lib/direccion).
import { destinoCuerpo, partirDireccion, type RepoExistente } from "$lib/direccion";
import { servicio } from "../../puente.svelte";

/** Lo que dice el equipo al probarlo (como `Prueba` de $lib/adoptar). */
export interface Prueba {
  mensaje: string;
  versiones: number;
  ultima: string | null;
  solo_anadir: boolean | null;
  /** Si este equipo ya lo usa: su nombre. */
  en_uso: string | null;
  /** Los equipos y las etiquetas de sus versiones. */
  equipos: string[];
  etiquetas: string[];
}

/** Lo que va en `adoptar_repositorio` para un repositorio escrito a mano. */
export function cuerpoExistente(repo: RepoExistente, destinoId?: string) {
  return {
    destino: destinoCuerpo(repo, destinoId ? { id: destinoId } : {}),
    ruta: partirDireccion(repo.tipo, repo.direccion).ruta,
    contrasena: repo.contrasena,
  };
}

/** «Probar»: el equipo lo abre con esa contraseña y dice qué tiene (no guarda nada). */
export async function probarExistente(repo: RepoExistente): Promise<Prueba> {
  const r = await servicio<{ mensaje: string; detalle?: Partial<Prueba> }>("adoptar_repositorio", {
    repositorio: { solo_probar: true, ...cuerpoExistente(repo) },
  });
  const d = r.detalle ?? {};
  return {
    mensaje: r.mensaje,
    versiones: Number(d.versiones ?? 0),
    ultima: d.ultima ?? null,
    solo_anadir: d.solo_anadir ?? null,
    en_uso: d.en_uso ?? null,
    equipos: d.equipos ?? [],
    etiquetas: d.etiquetas ?? [],
  };
}

/** Cómo va el historial que se trae a un repositorio (null: nada pedido desde que arrancó el servicio). */
export interface EstadoHistorial {
  estado: "en_marcha" | "hecha" | "fallida";
  mensaje: string;
}
export const estadoHistorial = (repo: string) => servicio<EstadoHistorial | null>("historial_traido", { repo });
