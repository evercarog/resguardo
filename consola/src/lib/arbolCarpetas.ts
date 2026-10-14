// 14zz: el árbol de «Elegir en el equipo» con subcarpetas. Marcar una carpeta
// marca todo lo de dentro (también lo que aparezca después); desmarcar una
// subcarpeta la añade a «No copiar» con su ruta exacta y la de arriba queda a
// medias. Funciones puras (con pruebas en scripts/vectores-pulido-final.ts).

export interface Seleccion {
  /** Carpetas que se copian (sin repetir las que ya están dentro de otra). */
  elegidas: string[];
  /** Rutas exactas que no se copian («No copiar»). */
  excluidas: string[];
}

export type EstadoNodo = "si" | "no" | "medio";

/** Ruta comparable: sin separador final y, en Windows, sin distinguir mayúsculas. */
function norma(r: string, win: boolean): string {
  const sin = r.length > 1 && /[\\/]$/.test(r) && !(win && /^[A-Za-z]:\\$/.test(r)) ? r.replace(/[\\/]+$/, "") : r;
  return win ? sin.toLowerCase() : sin;
}

export function misma(a: string, b: string, win: boolean): boolean {
  return norma(a, win) === norma(b, win);
}

/** ¿Está `hija` dentro de `padre` (no la misma)? */
export function dentroDe(hija: string, padre: string, win: boolean): boolean {
  const h = norma(hija, win);
  const p = norma(padre, win);
  if (h === p) return false;
  const sep = win ? "\\" : "/";
  return h.startsWith(p.endsWith(sep) ? p : p + sep);
}

/** ¿Es una ruta completa sin comodines? (Solo esas son exclusiones de «una carpeta»). */
export function esRutaExacta(r: string, win: boolean): boolean {
  const t = r.trim();
  if (/[*?[\]]/.test(t)) return false;
  return win ? /^[A-Za-z]:\\./.test(t) || /^\\\\[^\\]/.test(t) : t.startsWith("/") && t.length > 1;
}

/** Lo que decide sobre `r`: la elegida o excluida más honda que es `r` o la contiene. */
function decide(r: string, s: Seleccion, win: boolean): "elegida" | "excluida" | null {
  let mejor: { largo: number; que: "elegida" | "excluida" } | null = null;
  for (const [lista, que] of [
    [s.elegidas, "elegida"],
    [s.excluidas, "excluida"],
  ] as const)
    for (const x of lista)
      if ((misma(x, r, win) || dentroDe(r, x, win)) && (!mejor || norma(x, win).length > mejor.largo)) mejor = { largo: norma(x, win).length, que };
  return mejor?.que ?? null;
}

/** ¿Se copia `r`? (Con todo lo de dentro salvo lo excluido.) */
export function seCopia(r: string, s: Seleccion, win: boolean): boolean {
  return decide(r, s, win) === "elegida";
}

/** ¿Está dentro de una carpeta excluida? (Entonces no se puede marcar suelta: restic no entra en ella.) */
export function dentroDeExcluida(r: string, s: Seleccion, win: boolean): boolean {
  return decide(r, s, win) === "excluida" && !s.excluidas.some((x) => misma(x, r, win));
}

/** Marcada, sin marcar o a medias (–). */
export function estadoNodo(r: string, s: Seleccion, win: boolean): EstadoNodo {
  if (seCopia(r, s, win)) return s.excluidas.some((x) => dentroDe(x, r, win)) ? "medio" : "si";
  return s.elegidas.some((x) => dentroDe(x, r, win)) ? "medio" : "no";
}

const sin = (xs: string[], f: (x: string) => boolean) => xs.filter((x) => !f(x));

/**
 * Pulsar la casilla de `r`: marcada → sin marcar (si la cubre otra de arriba, va
 * a «No copiar»); sin marcar o a medias → marcada entera (lo de dentro, también).
 */
export function alternarNodo(r: string, s: Seleccion, win: boolean): Seleccion {
  const dentro = (x: string) => dentroDe(x, r, win);
  const ella = (x: string) => misma(x, r, win);
  if (estadoNodo(r, s, win) === "si") {
    const elegidas = sin(s.elegidas, (x) => ella(x) || dentro(x));
    const excluidas = sin(s.excluidas, dentro);
    // Si sigue cubierta por una de arriba, se excluye con su ruta exacta.
    return decide(r, { elegidas, excluidas }, win) === "elegida" ? { elegidas, excluidas: [...excluidas, r] } : { elegidas, excluidas };
  }
  // Marcarla entera: fuera las exclusiones de dentro y las elegidas de dentro (las cubre ella).
  const excluidas = sin(s.excluidas, (x) => ella(x) || dentro(x));
  const base = { elegidas: sin(s.elegidas, dentro), excluidas };
  return decide(r, base, win) === "elegida" ? base : { elegidas: [...base.elegidas, r], excluidas };
}

/** Quitar una exclusión (desde la lista): vuelve a copiarse. */
export function quitarExclusion(r: string, s: Seleccion, win: boolean): Seleccion {
  return { elegidas: s.elegidas, excluidas: sin(s.excluidas, (x) => misma(x, r, win)) };
}

/** Las exclusiones que se enseñan en el árbol: las de dentro de alguna carpeta elegida. */
export function exclusionesVisibles(s: Seleccion, win: boolean): string[] {
  return s.excluidas.filter((x) => s.elegidas.some((e) => dentroDe(x, e, win)));
}

/**
 * «No copiar» al terminar: las reglas que no son una ruta exacta, tal cual, y
 * las rutas exactas que quedan dentro de alguna carpeta elegida (las que ya no
 * lo están no sirven de nada y se quitan).
 */
export function exclusionesFinales(antes: string[], s: Seleccion, win: boolean): string[] {
  const reglas = antes.filter((x) => !esRutaExacta(x, win));
  const rutas = exclusionesVisibles(s, win);
  const vistas = new Set<string>();
  return [...reglas, ...rutas].filter((x) => {
    const k = norma(x, win);
    if (vistas.has(k)) return false;
    vistas.add(k);
    return true;
  });
}

/** La selección inicial: las carpetas de la copia y, de «No copiar», las rutas exactas. */
export function seleccionInicial(carpetas: string[], exclusiones: string[], win: boolean): Seleccion {
  return { elegidas: [...carpetas], excluidas: exclusiones.filter((x) => esRutaExacta(x, win)).map((x) => x.trim()) };
}
