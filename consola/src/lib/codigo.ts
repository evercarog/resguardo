// Códigos de «Añadir equipo» generados en el navegador (v1.48, docs/api-servidor.md §4).
//
// Antes los generaba el servidor y los guardaba en claro mientras servían (hasta 24 h en el
// instalador listo): un servidor malicioso los veía y podía calcular la `prueba_codigo` del
// `alta` antes que la consola. Ahora los genera la consola con `crypto.getRandomValues`, al
// servidor solo le llega su hash (el mismo que manda el equipo al unirse:
// `protocolo::mensajes::code_hash`) y el código se queda en este navegador hasta el alta.
//
// Sin dependencias de Svelte ni de la red (scripts/vectores-emparejar.ts lo prueba en Node).
import { hashCodigo } from "./cripto/claves";

/** Las mismas letras y cifras que el servidor (`ALFABETO_CODIGO`): sin I, L, O, 0 ni 1. */
export const ALFABETO = "ABCDEFGHJKMNPQRSTUVWXYZ23456789";

/** Código para escribirlo a mano en el equipo (15 min): 10 caracteres, «ABCD-EFGH-JK» (≈ 49,5 bits). */
export const LARGO_A_MANO = 10;
/**
 * Código que viaja dentro del instalador o de la línea de Linux (24 h; nadie lo escribe):
 * 16 caracteres, «ABCD-EFGH-JKMN-PQRS» (≈ 79 bits). Cabe en la cola (8–20 caracteres con
 * guiones, `DatosInstalador::validar`), así que los agentes ≥ 0.7.7 lo leen igual.
 */
export const LARGO_PREPARADO = 16;

/** Bytes aleatorios (inyectable en las pruebas). */
export type Azar = (n: number) => Uint8Array;
const azarDelSistema: Azar = (n) => crypto.getRandomValues(new Uint8Array(n));

/**
 * Un código nuevo de `largo` caracteres (múltiplo de 2), en grupos de 4 separados por guiones.
 * Sin sesgo: los bytes ≥ 248 (8 × 31) se descartan.
 */
export function generarCodigo(largo: number = LARGO_A_MANO, azar: Azar = azarDelSistema): string {
  const tope = Math.floor(256 / ALFABETO.length) * ALFABETO.length;
  let s = "";
  while (s.length < largo) {
    const b = azar(largo * 2);
    for (const x of b) {
      if (s.length === largo) break;
      if (x < tope) s += ALFABETO[x % ALFABETO.length];
    }
    b.fill(0);
  }
  return s.match(/.{1,4}/g)!.join("-");
}

// --- Dónde se guardan hasta el alta -----------------------------------------
// En este navegador (localStorage), por emparejamiento. Hacen falta hasta que el equipo hace el
// alta: un preparado puede tardar 24 h en unirse y, unido, el servidor da al menos otras 24 h para
// comparar el número (y hasta 7 días confirmado sin el alta). Se olvidan con el alta, al anular
// y, como mucho, a los 8 días. En otro navegador no están: allí se puede escribir el código a mano
// (se comprueba con el hash que da el servidor) o anular y preparar otro.

export interface CodigoGuardado {
  /** Id del emparejamiento. */
  id: string;
  cliente: string;
  codigo: string;
  /** Cuándo se generó (ms). */
  creado: number;
  /** Preparados: el nombre del equipo y su sistema (para volver a dar el mismo). */
  nombre?: string;
  so?: "windows" | "linux";
  /**
   * Bloque 7: un código para varios equipos (`id` es el del lote). Se guarda hasta `hasta` (ms):
   * su caducidad más 2 días para confirmar a los últimos que se unan (como mucho `PLAZO_LOTE_MS`).
   */
  lote?: boolean;
  hasta?: number;
}

/** Lo mínimo de `localStorage` (inyectable en las pruebas). */
export interface Almacen {
  getItem(k: string): string | null;
  setItem(k: string, v: string): void;
  removeItem(k: string): void;
}

export const CLAVE_ALMACEN = "resguardo.codigos-emparejar";
/** Lo más que se guarda un código (ms): 8 días. */
export const PLAZO_GUARDADO_MS = 8 * 24 * 3600_000;
/** Lo más que se guarda un código para varios equipos (ms): 30 días que puede servir y 2 más. */
export const PLAZO_LOTE_MS = 32 * 24 * 3600_000;
/** Como mucho, tantos (los más antiguos salen primero). */
export const MAX_GUARDADOS = 200;

function almacenDelNavegador(): Almacen | null {
  try {
    return typeof localStorage === "undefined" ? null : localStorage;
  } catch {
    return null;
  }
}

/** Si el navegador no deja guardar (modo privado estricto), al menos en memoria (esta pestaña). */
const memoria = new Map<string, string>();
const enMemoria: Almacen = { getItem: (k) => memoria.get(k) ?? null, setItem: (k, v) => void memoria.set(k, v), removeItem: (k) => void memoria.delete(k) };

function leerTodos(a: Almacen, ahora: number): CodigoGuardado[] {
  let xs: unknown;
  try {
    xs = JSON.parse(a.getItem(CLAVE_ALMACEN) ?? "[]");
  } catch {
    xs = [];
  }
  if (!Array.isArray(xs)) return [];
  return xs.filter(
    (x): x is CodigoGuardado =>
      !!x &&
      typeof x.id === "string" &&
      typeof x.cliente === "string" &&
      typeof x.codigo === "string" &&
      typeof x.creado === "number" &&
      (ahora - x.creado < PLAZO_GUARDADO_MS || (x.lote === true && typeof x.hasta === "number" && ahora < x.hasta && x.hasta - x.creado <= PLAZO_LOTE_MS)),
  );
}

function escribirTodos(a: Almacen, xs: CodigoGuardado[]) {
  try {
    if (xs.length) a.setItem(CLAVE_ALMACEN, JSON.stringify(xs.slice(-MAX_GUARDADOS)));
    else a.removeItem(CLAVE_ALMACEN);
  } catch {
    /* lleno o bloqueado: queda en memoria */
    if (a !== enMemoria) escribirTodos(enMemoria, xs);
  }
}

/** Los códigos guardados en este navegador. */
export class Codigos {
  private a: Almacen;
  constructor(
    almacen: Almacen | null = almacenDelNavegador(),
    private reloj: () => number = Date.now,
  ) {
    this.a = almacen ?? enMemoria;
  }
  private todos(): CodigoGuardado[] {
    const xs = leerTodos(this.a, this.reloj());
    if (this.a !== enMemoria) for (const x of leerTodos(enMemoria, this.reloj())) if (!xs.some((y) => y.id === x.id)) xs.push(x);
    return xs;
  }
  guardar(x: Omit<CodigoGuardado, "creado">) {
    const xs = this.todos().filter((y) => y.id !== x.id);
    xs.push({ ...x, creado: this.reloj() });
    escribirTodos(this.a, xs);
  }
  /** El código de ese emparejamiento, si está aquí y (si se da) coincide con su hash. */
  de(id: string, hash?: string | null): string | null {
    const x = this.todos().find((y) => y.id === id);
    if (!x) return null;
    if (hash && hashCodigo(x.codigo) !== hash.toLowerCase()) return null;
    return x.codigo;
  }
  /** El código de este cliente con ese hash (bloque 7: el de un lote, para el alta de cada equipo que se unió con él). */
  porHash(cliente: string, hash: string | null | undefined): string | null {
    if (!hash) return null;
    const h = hash.toLowerCase();
    return this.todos().find((y) => y.cliente === cliente && hashCodigo(y.codigo) === h)?.codigo ?? null;
  }
  /** Un preparado de este cliente con ese nombre (sin distinguir mayúsculas) y sistema. */
  preparado(cliente: string, nombre: string, so: "windows" | "linux"): CodigoGuardado | null {
    const n = nombre.trim().toLowerCase();
    return (
      this.todos()
        .filter((y) => y.cliente === cliente && y.so === so && y.nombre?.toLowerCase() === n)
        .sort((p, q) => q.creado - p.creado)[0] ?? null
    );
  }
  olvidar(id: string) {
    escribirTodos(this.a, leerTodos(this.a, this.reloj()).filter((y) => y.id !== id));
    escribirTodos(enMemoria, leerTodos(enMemoria, this.reloj()).filter((y) => y.id !== id));
  }
}

/** ¿Es este el código de ese hash? (para escribirlo a mano en otro navegador). */
export const codigoDeHash = (codigo: string, hash: string | null | undefined) => !!hash && /^[A-Za-z0-9 -]{8,40}$/.test(codigo.trim()) && hashCodigo(codigo) === hash.toLowerCase();
