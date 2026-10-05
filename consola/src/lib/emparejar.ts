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
  codigo: string;
  caduca: string;
  estado: "abierto" | "unido";
}

export interface ApiCodigos {
  /** `GET /api/clientes/{c}/codigo-abierto` (o `null`). */
  codigoAbierto: () => Promise<CodigoAbierto | null>;
  /** `POST /api/clientes/{c}/emparejamientos`. */
  abrir: () => Promise<{ id: string; codigo: string; caduca: string; reutilizado?: boolean }>;
}

/** Margen: un código al que le queda menos no se ofrece para seguir (no da tiempo a escribirlo). */
export const MARGEN_MS = 2 * 60_000;

/** ¿Sirve todavía para seguir con él? (abierto con margen, o ya unido y sin caducar). */
export function sirve(p: CodigoAbierto | null | undefined, ahora = Date.now()): p is CodigoAbierto {
  if (!p) return false;
  const queda = Date.parse(p.caduca) - ahora;
  return p.estado === "unido" ? queda > 0 : queda > MARGEN_MS;
}

/**
 * Al abrir (o recargar) la página: solo PREGUNTA si ya hay un código; nunca lo crea.
 * Con un servidor anterior (sin la ruta: 404) o sin red, `null` (la página sigue igual).
 */
export async function codigoAlCargar(api: Pick<ApiCodigos, "codigoAbierto">, ahora = Date.now()): Promise<CodigoAbierto | null> {
  try {
    const p = await api.codigoAbierto();
    return sirve(p, ahora) ? p : null;
  } catch {
    return null;
  }
}

/**
 * «Generar el código» (acción explícita): el que ya hay si aún sirve; si no, uno nuevo.
 * (El servidor también devuelve el abierto en vez de crear otro: esto ahorra la petición
 * y vale con servidores anteriores, que no lo hacían.)
 */
export async function pedirCodigo(api: ApiCodigos, yaTengo: CodigoAbierto | null, ahora = Date.now()): Promise<{ id: string; codigo: string; caduca: string; estado: "abierto" | "unido"; nuevo: boolean }> {
  if (sirve(yaTengo, ahora)) return { ...yaTengo, nuevo: false };
  const otro = await codigoAlCargar(api, ahora);
  if (otro) return { ...otro, nuevo: false };
  const r = await api.abrir();
  return { id: r.id, codigo: r.codigo, caduca: r.caduca, estado: "abierto", nuevo: !r.reutilizado };
}

/** Segundos que pide esperar un 429 del servidor (`retry_after`), si los dice. */
export function esperaDe(e: unknown): number | null {
  const x = e as { estado?: number; cuerpo?: Record<string, unknown> } | null;
  if (x?.estado !== 429) return null;
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
