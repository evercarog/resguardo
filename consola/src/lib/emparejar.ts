// «Añadir equipo»: cuándo se pide un código y cuándo se vuelve a enseñar el que
// ya hay. Abrir la página, recargarla o cambiar de opción NO crea códigos: solo
// pulsar «Generar el código» (o preparar un instalador). Si la cuenta ya tiene
// uno que sirve, se enseña ese con su caducidad y «Anular». El servidor limita
// los códigos nuevos por cuenta y por cliente (instaladores.rs, `limite_codigos`)
// y, pasado el límite, dice cuánto esperar (`retry_after`).
//
// Sin dependencias de Svelte ni de la red: la página le pasa las llamadas a la
// API (y scripts/vectores-emparejar.ts las simula).

/** Lo que da `GET …/codigo-abierto`: el código de 15 min de esta cuenta que aún sirve. */
export interface CodigoAbierto {
  id: string;
  /** v1.4x: `null` si lo generó un navegador (el servidor solo tiene `codigo_hash`). */
  codigo: string | null;
  caduca: string;
  estado: "abierto" | "unido";
  codigo_hash?: string;
  codigo_navegador?: boolean;
}

/** Un código que esta consola puede enseñar (con el código en claro). */
export type CodigoConocido = CodigoAbierto & { codigo: string };

export interface ApiCodigos {
  /** `GET /api/clientes/{c}/codigo-abierto` (o `null`; con `?navegador=1` si `navegador`). */
  codigoAbierto: () => Promise<CodigoAbierto | null>;
  /** `POST /api/clientes/{c}/emparejamientos` (v1.4x: con el hash del código si `navegador`). */
  abrir: (codigoHash?: string) => Promise<{ id: string; codigo?: string | null; caduca: string; reutilizado?: boolean; codigo_navegador?: boolean }>;
  /**
   * v1.4x: el servidor acepta códigos del navegador (`codigo_navegador` en `GET /api/servidor`).
   * Entonces el código se genera aquí y se guarda en `codigos` hasta el alta.
   */
  navegador?: {
    generar: () => string;
    hash: (codigo: string) => string;
    /** El código guardado de ese emparejamiento (comprobado con su hash), o `null`. */
    de: (id: string, hash?: string | null) => string | null;
    guardar: (id: string, codigo: string) => void;
  };
}

/** Margen: un código al que le queda menos no se ofrece para seguir (no da tiempo a escribirlo). */
export const MARGEN_MS = 2 * 60_000;

/** ¿Sirve todavía para seguir con él? (abierto con margen, o ya unido y sin caducar; y con el código a mano). */
export function sirve(p: CodigoAbierto | null | undefined, ahora = Date.now()): p is CodigoConocido {
  if (!p || !p.codigo) return false;
  const queda = Date.parse(p.caduca) - ahora;
  return p.estado === "unido" ? queda > 0 : queda > MARGEN_MS;
}

/**
 * Al abrir (o recargar) la página: solo PREGUNTA si ya hay un código; nunca lo crea.
 * Con un servidor anterior (sin la ruta: 404) o sin red, `null` (la página sigue igual).
 * v1.4x: si el código lo generó un navegador, solo vale si es este (lo tiene guardado y
 * coincide con su hash); si se pidió en otro, `null` (se puede pedir otro).
 */
export async function codigoAlCargar(api: Pick<ApiCodigos, "codigoAbierto" | "navegador">, ahora = Date.now()): Promise<CodigoConocido | null> {
  try {
    let p = await api.codigoAbierto();
    if (p && !p.codigo && p.codigo_navegador) p = { ...p, codigo: api.navegador?.de(p.id, p.codigo_hash) ?? null };
    return sirve(p, ahora) ? p : null;
  } catch {
    return null;
  }
}

/** El 409 de un hash que ya existía (muy raro): se genera otro y se vuelve a pedir, una vez. */
const repetido = (e: unknown) => (e as { estado?: number })?.estado === 409;

/**
 * «Generar el código» (acción explícita): el que ya hay si aún sirve; si no, uno nuevo.
 * (El servidor también devuelve el abierto en vez de crear otro: esto ahorra la petición
 * y vale con servidores anteriores, que no lo hacían.) v1.4x: con `navegador`, el código
 * nuevo lo genera esta consola y al servidor solo le llega su hash.
 */
export async function pedirCodigo(api: ApiCodigos, yaTengo: CodigoAbierto | null, ahora = Date.now()): Promise<CodigoConocido & { nuevo: boolean }> {
  if (sirve(yaTengo, ahora)) return { ...yaTengo, nuevo: false };
  const otro = await codigoAlCargar(api, ahora);
  if (otro) return { ...otro, nuevo: false };
  const nav = api.navegador;
  if (nav) {
    for (let intento = 0; ; intento++) {
      const codigo = nav.generar();
      const codigo_hash = nav.hash(codigo);
      try {
        const r = await api.abrir(codigo_hash);
        // Un servidor que no entendiera el hash daría su propio código: se usa ese (forma de antes).
        if (r.codigo) return { id: r.id, codigo: r.codigo, caduca: r.caduca, estado: "abierto", nuevo: !r.reutilizado };
        nav.guardar(r.id, codigo);
        return { id: r.id, codigo, codigo_hash, codigo_navegador: true, caduca: r.caduca, estado: "abierto", nuevo: true };
      } catch (e) {
        if (!repetido(e) || intento > 0) throw e;
      }
    }
  }
  const r = await api.abrir();
  if (!r.codigo) throw new Error("El servidor no dio el código. Vuelve a intentarlo.");
  return { id: r.id, codigo: r.codigo, caduca: r.caduca, estado: "abierto", nuevo: !r.reutilizado };
}

/** Segundos que pide esperar un 429 del servidor (`retry_after`), si los dice. */
export function esperaDe(e: unknown): number | null {
  const x = e as { estado?: number; cuerpo?: Record<string, unknown> } | null;
  if (x?.estado !== 429) return null;
  // Solo el límite de códigos («cuenta» o «ip» son los generales: los explica su mensaje).
  const limite = x.cuerpo?.limite;
  if (typeof limite === "string" && limite !== "codigos") return null;
  const s = Number(x.cuerpo?.retry_after);
  return Number.isFinite(s) && s > 0 ? Math.ceil(s) : null;
}

/** «Podrás pedir otro código en N min» (redondeando hacia arriba; al menos 1). */
export function podrasPedirEn(segundos: number): string {
  const min = Math.max(1, Math.ceil(segundos / 60));
  if (min >= 60) {
    const h = Math.floor(min / 60);
    const m = min % 60;
    return `Podrás pedir otro código en ${h} h${m ? ` ${m} min` : ""}.`;
  }
  return `Podrás pedir otro código en ${min} min.`;
}

/** El texto del límite de códigos: qué pasa, por qué (seguridad) y cuándo se podrá. */
export function mensajeLimite(segundos: number): string {
  return `Se han pedido muchos códigos en poco tiempo. Por seguridad hay un máximo por hora: cada código deja entrar a un equipo nuevo, así que no se reparten sin medida. ${podrasPedirEn(segundos)} Mientras tanto, usa o anula los que ya tienes.`;
}

/** El mensaje de un error al pedir un código: el del límite con su espera, o el del servidor. */
export function mensajeAlPedir(e: unknown): string {
  const s = esperaDe(e);
  return s ? mensajeLimite(s) : ((e as Error)?.message ?? "Algo salió mal. Vuelve a intentarlo.");
}

// --- La línea de Linux -------------------------------------------------------
// La consola compone `sudo resguardo-agente vincular …` con lo que da el servidor
// (código, dirección y huella) y la persona la pega en una terminal como root. Un
// servidor malicioso podría colar ahí `;`, `$(…)` o un salto de línea: antes de
// enseñarla, cada parte tiene que tener su forma (como `DatosInstalador::validar`
// en crates/protocolo/src/instalador.rs) y nada que la shell interprete.

/** Código de emparejamiento: `ABCD-EFGH-JK` (8–20 letras, cifras y guiones). */
export function codigoValido(c: string): boolean {
  return /^[A-Za-z0-9-]{8,20}$/.test(c);
}

/** Dirección del servidor: `https://servidor:puerto` (con ruta si va detrás de un proxy), o `http://` a este mismo equipo. */
export function servidorValido(s: string): boolean {
  const m = /^(https?):\/\/([A-Za-z0-9.\-:[\]]{1,200})(\/[A-Za-z0-9._~\-/]{0,200})?$/.exec(s);
  if (!m) return false;
  const [, esquema, host] = m;
  if (host.startsWith(":") || host.endsWith(":")) return false;
  return esquema === "https" || /^(localhost|127\.0\.0\.1|\[::1\])(:\d{1,5})?$/.test(host);
}

/** Huella de la autoridad TLS: 32 pares hexadecimales separados por `:`. */
export function huellaValida(h: string): boolean {
  return /^[0-9A-Fa-f]{2}(:[0-9A-Fa-f]{2}){31}$/.test(h);
}

/**
 * `sudo resguardo-agente vincular <código> --servidor <dirección> [--huella-ca <huella>]`,
 * o `""` si alguna parte no tiene su forma (la página no enseña la línea y lo dice).
 */
export function lineaVincular(codigo: string, servidor: string, huellaCa?: string): string {
  if (!codigoValido(codigo) || !servidorValido(servidor)) return "";
  if (huellaCa !== undefined && !huellaValida(huellaCa)) return "";
  return `sudo resguardo-agente vincular ${codigo} --servidor ${servidor}` + (huellaCa !== undefined ? ` --huella-ca ${huellaCa}` : "");
}
