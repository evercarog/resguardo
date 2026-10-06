// Geometría del «Mapa de la protección» (MapaProteccion.svelte, docs/diseno.md §4):
// dónde va cada tarjeta y por dónde pasa cada trazo. TypeScript sin runas, para
// probarlo en scripts/vectores-mapa.ts con grafos de ejemplo.
//
// Es un dibujo por capas de izquierda a derecha (el método de Sugiyama, en
// pequeño):
//
// 1. Cada columna del mapa es una capa. Un trazo que salta capas (un repositorio
//    a su copia externa, por encima de la columna de los destinos) deja un
//    «hueco» en cada capa que cruza: una fila reservada, así que el trazo pasa
//    entre dos tarjetas y nunca por detrás de una.
// 2. El orden de cada capa (salvo la primera, que manda: lo urgente arriba) se
//    elige por baricentros, bajando y subiendo varias veces, y se queda el que
//    menos cruces da.
// 3. La altura: cada tarjeta, lo más cerca posible de la media de las que
//    tiene a cada lado, sin pisarse y sin cambiar el orden (regresión isotónica
//    con «pool adjacent violators»: exacta y rápida).
// 4. Los trazos van por los canales que quedan entre columnas (curvas de
//    Bézier con las tangentes en horizontal, que no se salen del canal) y, en
//    las capas que cruzan, en recta por su hueco. Todos los que salen de una
//    tarjeta salen de un solo puerto, a su derecha.

export interface Punto {
  x: number;
  y: number;
}
export interface Caja {
  x: number;
  y: number;
  w: number;
  h: number;
}
export type Tramo = { tipo: "curva" | "recta"; de: Punto; a: Punto };
export interface Ruta {
  /** El id de la arista. */
  id: string;
  de: string;
  a: string;
  tramos: Tramo[];
  /** El trazo en SVG. */
  d: string;
  /** Dónde va su marca (el círculo con el icono del estado), si la lleva. */
  marca: Punto | null;
}

export interface EntradaDisposicion {
  /** Las tarjetas, con su columna y en el orden preferido (el de `ordenarMapa`). */
  nodos: { id: string; col: number }[];
  /** Los trazos; `marca`: lleva el círculo del estado (deja más sitio en sus huecos). */
  aristas: { id: string; de: string; a: string; marca?: boolean }[];
  /** El alto medido de cada tarjeta (sin medir: `altoPorDefecto`). */
  altos?: Record<string, number>;
  /** El ancho de cada columna (por su número de columna). */
  anchos?: Record<number, number>;
  anchoPorDefecto?: number;
  altoPorDefecto?: number;
  /** Separación entre columnas (el canal por donde van los trazos). */
  separacionCol?: number;
  /** Separación entre tarjetas de una columna (por columna, o la misma para todas). */
  separacionFila?: number | Record<number, number>;
}

export interface Disposicion {
  cajas: Record<string, Caja>;
  rutas: Ruta[];
  /** Las tarjetas de cada capa, de arriba abajo (para recorrerlas con las flechas). */
  capas: string[][];
  ancho: number;
  alto: number;
  /** Cuántos cruces quedaron entre capas vecinas (para las pruebas). */
  cruces: number;
}

const ALTO_HUECO = 6;
const ALTO_HUECO_MARCA = 24;
const SEP_HUECO = 6;
const MARCA = 22;

interface Item {
  id: string;
  capa: number;
  hueco: boolean;
  alto: number;
}

/** Regresión isotónica ponderada (no decreciente): la solución de mínimos cuadrados con orden. */
export function isotonica(v: number[], w: number[]): number[] {
  const bloques: { suma: number; peso: number; n: number }[] = [];
  for (let i = 0; i < v.length; i++) {
    bloques.push({ suma: v[i] * w[i], peso: w[i], n: 1 });
    while (bloques.length > 1) {
      const b = bloques[bloques.length - 1];
      const a = bloques[bloques.length - 2];
      if (a.suma / a.peso <= b.suma / b.peso) break;
      bloques.splice(-2, 2, { suma: a.suma + b.suma, peso: a.peso + b.peso, n: a.n + b.n });
    }
  }
  return bloques.flatMap((b) => Array<number>(b.n).fill(b.suma / b.peso));
}

const media = (xs: number[]) => xs.reduce((s, x) => s + x, 0) / xs.length;

/** Un punto de la curva de un tramo (Bézier cúbica con las tangentes en horizontal). */
export function puntoEn(t: Tramo, u: number): Punto {
  if (t.tipo === "recta") return { x: t.de.x + (t.a.x - t.de.x) * u, y: t.de.y + (t.a.y - t.de.y) * u };
  const [c1, c2] = controles(t);
  const m = 1 - u;
  const k0 = m * m * m;
  const k1 = 3 * m * m * u;
  const k2 = 3 * m * u * u;
  const k3 = u * u * u;
  return { x: k0 * t.de.x + k1 * c1.x + k2 * c2.x + k3 * t.a.x, y: k0 * t.de.y + k1 * c1.y + k2 * c2.y + k3 * t.a.y };
}

function controles(t: Tramo): [Punto, Punto] {
  const dx = t.a.x > t.de.x ? (t.a.x - t.de.x) / 2 : 40;
  return [
    { x: t.de.x + dx, y: t.de.y },
    { x: t.a.x - dx, y: t.a.y },
  ];
}

const r = (n: number) => Math.round(n * 10) / 10;

function trazoSvg(tramos: Tramo[]): string {
  if (!tramos.length) return "";
  let d = `M${r(tramos[0].de.x)},${r(tramos[0].de.y)}`;
  for (const t of tramos) {
    if (t.tipo === "recta") d += ` L${r(t.a.x)},${r(t.a.y)}`;
    else {
      const [c1, c2] = controles(t);
      d += ` C${r(c1.x)},${r(c1.y)} ${r(c2.x)},${r(c2.y)} ${r(t.a.x)},${r(t.a.y)}`;
    }
  }
  return d;
}

/** Dónde va cada tarjeta y por dónde pasa cada trazo. */
export function disponer(e: EntradaDisposicion): Disposicion {
  const anchoPorDefecto = e.anchoPorDefecto ?? 220;
  const altoPorDefecto = e.altoPorDefecto ?? 64;
  const sepCol = e.separacionCol ?? 64;
  const cols = [...new Set(e.nodos.map((n) => n.col))].sort((a, b) => a - b);
  if (!cols.length) return { cajas: {}, rutas: [], capas: [], ancho: 0, alto: 0, cruces: 0 };
  const capaDeCol = new Map(cols.map((c, i) => [c, i]));
  const ancho = (capa: number) => e.anchos?.[cols[capa]] ?? anchoPorDefecto;
  const sepFila = (capa: number) => (typeof e.separacionFila === "number" ? e.separacionFila : (e.separacionFila?.[cols[capa]] ?? 14));

  const items = new Map<string, Item>();
  const capas: string[][] = cols.map(() => []);
  for (const n of e.nodos) {
    if (items.has(n.id)) continue;
    const capa = capaDeCol.get(n.col)!;
    items.set(n.id, { id: n.id, capa, hueco: false, alto: e.altos?.[n.id] ?? altoPorDefecto });
    capas[capa].push(n.id);
  }

  // Los trazos, partidos en tramos entre capas vecinas (con un hueco en cada capa que cruzan).
  const cadenas = new Map<string, string[]>();
  const antes = new Map<string, string[]>();
  const despues = new Map<string, string[]>();
  const enlazar = (de: string, a: string) => {
    antes.set(a, [...(antes.get(a) ?? []), de]);
    despues.set(de, [...(despues.get(de) ?? []), a]);
  };
  for (const a of e.aristas) {
    const de = items.get(a.de);
    const hasta = items.get(a.a);
    if (!de || !hasta || de.hueco || hasta.hueco || cadenas.has(a.id)) continue;
    if (hasta.capa <= de.capa) {
      // Hacia atrás o en la misma columna (no pasa en el mapa): una curva sin más.
      cadenas.set(a.id, [a.de, a.a]);
      continue;
    }
    const ids = [a.de];
    for (let c = de.capa + 1; c < hasta.capa; c++) {
      const id = `\u0000${a.id}#${c}`;
      items.set(id, { id, capa: c, hueco: true, alto: a.marca && c === de.capa + 1 ? ALTO_HUECO_MARCA : ALTO_HUECO });
      capas[c].push(id);
      ids.push(id);
    }
    ids.push(a.a);
    for (let i = 0; i + 1 < ids.length; i++) enlazar(ids[i], ids[i + 1]);
    cadenas.set(a.id, ids);
  }

  // 2. El orden de cada capa: baricentros abajo y arriba; la primera no se mueve.
  const pos = new Map<string, number>();
  const numerar = (c: number) => capas[c].forEach((id, i) => pos.set(id, i));
  capas.forEach((_, c) => numerar(c));
  const ordenar = (c: number, vecinos: Map<string, string[]>) => {
    const claves = capas[c].map((id, i) => {
      const xs = (vecinos.get(id) ?? []).map((v) => pos.get(v)!);
      return { id, k: xs.length ? media(xs) : i };
    });
    claves.sort((a, b) => a.k - b.k);
    capas[c] = claves.map((x) => x.id);
    numerar(c);
  };
  const contarCruces = () => {
    let n = 0;
    for (let c = 0; c + 1 < capas.length; c++) {
      const ls: [number, number][] = [];
      for (const id of capas[c]) for (const v of despues.get(id) ?? []) if (items.get(v)!.capa === c + 1) ls.push([pos.get(id)!, pos.get(v)!]);
      for (let i = 0; i < ls.length; i++) for (let j = i + 1; j < ls.length; j++) if ((ls[i][0] - ls[j][0]) * (ls[i][1] - ls[j][1]) < 0) n++;
    }
    return n;
  };
  // Primera bajada (coloca los huecos, que empiezan al final de su capa).
  for (let c = 1; c < capas.length; c++) ordenar(c, antes);
  let mejor = capas.map((x) => [...x]);
  let menos = contarCruces();
  for (let vuelta = 0; vuelta < 8 && menos > 0; vuelta++) {
    for (let c = capas.length - 2; c >= 1; c--) ordenar(c, despues);
    for (let c = 1; c < capas.length; c++) ordenar(c, antes);
    const n = contarCruces();
    if (n < menos) {
      menos = n;
      mejor = capas.map((x) => [...x]);
    }
  }
  mejor.forEach((x, c) => {
    capas[c] = x;
    numerar(c);
  });

  // 3. La altura: cerca de la media de los vecinos, sin pisarse y en orden.
  const centro = new Map<string, number>();
  const separacion = (c: number, a: Item, b: Item) => (a.hueco || b.hueco ? SEP_HUECO : sepFila(c));
  const colocar = (c: number, vecinos: Map<string, string[]> | null) => {
    const its = capas[c].map((id) => items.get(id)!);
    const off: number[] = [];
    let acc = 0;
    its.forEach((it, i) => {
      if (i > 0) acc += its[i - 1].alto / 2 + separacion(c, its[i - 1], it) + it.alto / 2;
      off.push(acc);
    });
    const v: number[] = [];
    const w: number[] = [];
    its.forEach((it, i) => {
      const xs = vecinos ? (vecinos.get(it.id) ?? []).map((x) => centro.get(x)).filter((x): x is number => x != null) : [];
      const ahora = centro.get(it.id);
      const objetivo = xs.length ? media(xs) : (ahora ?? null);
      // Sin vecinos ni sitio previo: detrás del anterior (peso casi nulo).
      v.push(objetivo != null ? objetivo - off[i] : (v[i - 1] ?? 0));
      w.push(objetivo != null ? (it.hueco ? 0.6 : 1) : 1e-6);
    });
    const z = isotonica(v, w);
    its.forEach((it, i) => centro.set(it.id, z[i] + off[i]));
  };
  colocar(0, null);
  for (let c = 1; c < capas.length; c++) colocar(c, antes);
  for (let vuelta = 0; vuelta < 4; vuelta++) {
    for (let c = capas.length - 2; c >= 0; c--) colocar(c, despues);
    for (let c = 1; c < capas.length; c++) colocar(c, antes);
  }
  let arriba = Infinity;
  let abajo = -Infinity;
  for (const it of items.values()) {
    const y = centro.get(it.id) ?? 0;
    arriba = Math.min(arriba, y - it.alto / 2);
    abajo = Math.max(abajo, y + it.alto / 2);
  }
  if (!Number.isFinite(arriba)) arriba = abajo = 0;

  // Las columnas en horizontal.
  const xCapa: number[] = [];
  let x = 0;
  capas.forEach((_, c) => {
    xCapa.push(x);
    x += ancho(c) + sepCol;
  });
  const anchoTotal = Math.max(0, x - sepCol);
  const cajas: Record<string, Caja> = {};
  const cajaDe = (id: string): Caja => {
    const it = items.get(id)!;
    return { x: xCapa[it.capa], y: centro.get(id)! - arriba - it.alto / 2, w: ancho(it.capa), h: it.alto };
  };
  for (const it of items.values()) if (!it.hueco) cajas[it.id] = cajaDe(it.id);

  // 4. Los trazos: curvas por los canales y rectas por los huecos.
  const rutas: Ruta[] = [];
  const marcas: Punto[] = [];
  const libre = (p: Punto) => marcas.every((m) => Math.hypot(m.x - p.x, m.y - p.y) >= MARCA);
  for (const a of e.aristas) {
    const ids = cadenas.get(a.id);
    if (!ids) continue;
    const tramos: Tramo[] = [];
    const s = cajaDe(ids[0]);
    let p: Punto = { x: s.x + s.w, y: s.y + s.h / 2 };
    for (let i = 1; i < ids.length; i++) {
      const k = cajaDe(ids[i]);
      const entra = { x: k.x, y: k.y + k.h / 2 };
      tramos.push({ tipo: "curva", de: p, a: entra });
      if (i < ids.length - 1) {
        const sale = { x: k.x + k.w, y: entra.y };
        tramos.push({ tipo: "recta", de: entra, a: sale });
        p = sale;
      }
    }
    let marca: Punto | null = null;
    if (a.marca) {
      // En la recta de su primer hueco si salta columnas; si no, en la mitad de la curva. Sin pisar otra.
      const donde = tramos.find((t) => t.tipo === "recta") ?? tramos[0];
      const us = [0.5, 0.38, 0.62, 0.28, 0.72, 0.2, 0.8];
      marca = us.map((u) => puntoEn(donde, u)).find(libre) ?? puntoEn(donde, 0.5);
      marcas.push(marca);
    }
    rutas.push({ id: a.id, de: a.de, a: a.a, tramos, d: trazoSvg(tramos), marca });
  }

  return { cajas, rutas, capas: capas.map((c) => c.filter((id) => !items.get(id)!.hueco)), ancho: anchoTotal, alto: abajo - arriba, cruces: menos };
}

/** Puntos a lo largo de un trazo (para comprobarlo). */
export function muestras(ruta: Ruta, porTramo = 24): Punto[] {
  const out: Punto[] = [];
  for (const t of ruta.tramos) for (let i = out.length ? 1 : 0; i <= porTramo; i++) out.push(puntoEn(t, i / porTramo));
  return out;
}

/** ¿El segmento p–q entra en la caja (encogida `margen` por cada lado)? (Liang–Barsky.) */
export function cortaCaja(p: Punto, q: Punto, c: Caja, margen = 0.5): boolean {
  const x0 = c.x + margen;
  const x1 = c.x + c.w - margen;
  const y0 = c.y + margen;
  const y1 = c.y + c.h - margen;
  if (x1 <= x0 || y1 <= y0) return false;
  let t0 = 0;
  let t1 = 1;
  const dx = q.x - p.x;
  const dy = q.y - p.y;
  for (const [pp, qq] of [
    [-dx, p.x - x0],
    [dx, x1 - p.x],
    [-dy, p.y - y0],
    [dy, y1 - p.y],
  ]) {
    if (pp === 0) {
      if (qq < 0) return false;
    } else {
      const t = qq / pp;
      if (pp < 0) t0 = Math.max(t0, t);
      else t1 = Math.min(t1, t);
      if (t0 > t1) return false;
    }
  }
  return true;
}

/**
 * Lo que está mal en un dibujo: un trazo que pasa por una tarjeta que no es
 * ninguno de sus dos extremos, o dos tarjetas que se pisan. Vacío si todo bien.
 */
export function problemas(d: Disposicion): string[] {
  const out: string[] = [];
  const ids = Object.keys(d.cajas);
  for (const ruta of d.rutas) {
    const ps = muestras(ruta);
    for (const id of ids) {
      if (id === ruta.de || id === ruta.a) continue;
      const c = d.cajas[id];
      for (let i = 0; i + 1 < ps.length; i++)
        if (cortaCaja(ps[i], ps[i + 1], c)) {
          out.push(`«${ruta.id}» pasa por «${id}»`);
          break;
        }
    }
  }
  for (let i = 0; i < ids.length; i++)
    for (let j = i + 1; j < ids.length; j++) {
      const a = d.cajas[ids[i]];
      const b = d.cajas[ids[j]];
      if (a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h) out.push(`«${ids[i]}» pisa «${ids[j]}»`);
    }
  return out;
}
