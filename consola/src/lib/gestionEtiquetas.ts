// 0.8.1 (B2, plan-mejoras 15a): renombrar, borrar y unir etiquetas en todos los equipos
// del cliente («Organización» → Etiquetas). Lo que no depende de la pantalla, con sus
// pruebas en scripts/vectores-datos-comunes.ts; lo que manda las órdenes, en
// ./gestionEtiquetas.svelte.ts.
//
// - Las etiquetas son de cada equipo (v1.56, `etiquetas_equipo`): cambiar una en todos
//   es una orden por equipo afectado (o `PATCH` con un agente anterior).
// - El color y la plantilla por defecto (datos comunes, `etiqueta.color:` y
//   `etiqueta.plantilla:`) y la categoría (`etiqueta.categoria:`) de la vieja pasan a la
//   nueva; al unir, lo que ya tenga la de destino se queda.
import type { AjusteEtiqueta, Equipo } from "./tipos";
import type { CategoriaEtiqueta } from "./sedes";
import { mismaEtiqueta } from "./etiquetasGrupos";

/**
 * Las etiquetas de un equipo tras cambiar `de` por `a` (renombrar o unir; `null`: borrar),
 * en el mismo sitio y sin repetir (sin distinguir mayúsculas). `null` si no cambia nada.
 */
export function etiquetasTrasCambio(etiquetas: readonly string[], de: string, a: string | null): string[] | null {
  const out: string[] = [];
  for (const t of etiquetas) {
    const x = mismaEtiqueta(t, de) ? a : a && mismaEtiqueta(t, a) ? a : t;
    if (x === null || out.some((o) => mismaEtiqueta(o, x))) continue;
    out.push(x);
  }
  return out.length === etiquetas.length && out.every((t, i) => t === etiquetas[i]) ? null : out;
}

/** Los equipos que llevan `de` y cómo quedan sus etiquetas. */
export function cambiosEnEquipos<E extends Pick<Equipo, "id" | "etiquetas">>(equipos: readonly E[], de: string, a: string | null): { equipo: E; etiquetas: string[] }[] {
  const out: { equipo: E; etiquetas: string[] }[] = [];
  for (const e of equipos) {
    if (!(e.etiquetas ?? []).some((t) => mismaEtiqueta(t, de))) continue;
    const n = etiquetasTrasCambio(e.etiquetas ?? [], de, a);
    if (n) out.push({ equipo: e, etiquetas: n });
  }
  return out;
}

/**
 * Las categorías que cambian: la vieja sale de la suya; si la nueva aún no está en
 * ninguna, entra en su lugar (la categoría pasa a la nueva). Al borrar, solo sale.
 */
export function categoriasTrasCambio(categorias: readonly CategoriaEtiqueta[], de: string, a: string | null): CategoriaEtiqueta[] {
  const nuevaYaEsta = !!a && categorias.some((c) => c.etiquetas.some((t) => mismaEtiqueta(t, a) && !mismaEtiqueta(t, de)));
  const out: CategoriaEtiqueta[] = [];
  for (const c of categorias) {
    if (!c.etiquetas.some((t) => mismaEtiqueta(t, de))) continue;
    const etiquetas: string[] = [];
    for (const t of c.etiquetas) {
      const x = mismaEtiqueta(t, de) ? (a && !nuevaYaEsta ? a : null) : t;
      if (x !== null && !etiquetas.some((o) => mismaEtiqueta(o, x))) etiquetas.push(x);
    }
    out.push({ ...c, etiquetas });
  }
  return out;
}

/**
 * Los ajustes (color y plantilla; los avisos, si se pueden mover) que quedan en la nueva:
 * los suyos si ya los tiene y, si no, los de la vieja. `null` si no hay nada que poner.
 */
export function ajusteTrasCambio(vieja: AjusteEtiqueta | undefined, nueva: AjusteEtiqueta | undefined, a: string, conAvisos: boolean): AjusteEtiqueta | null {
  if (!vieja) return null;
  const color = nueva?.color ?? vieja.color ?? null;
  const plantilla = nueva?.plantilla ?? vieja.plantilla ?? null;
  const avisos = conAvisos ? (nueva?.avisos ?? vieja.avisos ?? null) : (nueva?.avisos ?? null);
  if (color === (nueva?.color ?? null) && plantilla === (nueva?.plantilla ?? null) && avisos === (nueva?.avisos ?? null)) return null;
  return { nombre: a, color, plantilla, avisos };
}

/** Un nombre nuevo para una etiqueta: el error, o null si vale. (Como `limpiarEtiqueta`.) */
export function errorNombreEtiqueta(t: string): string | null {
  const x = t.split(/\s+/).filter(Boolean).join(" ");
  if (!x) return "Escribe el nombre.";
  // eslint-disable-next-line no-control-regex
  if ([...x].length > 32 || /[\u0000-\u001f,]/.test(x)) return "Hasta 32 caracteres, sin comas.";
  return null;
}
