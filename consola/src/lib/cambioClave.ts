// Cambiar la clave de administración de un cliente (orden `cambiar_clave_admin`,
// api-servidor.md §5; con varias consolas, consolas-multiples.md §4.2).
//
// Lo que no es pantalla, sin nada del navegador (lo usa también el escenario
// e2e): el cuerpo de la orden para cada equipo, qué equipos tienen ya qué
// clave y en qué va el cambio.
//
// - El verificador es **del equipo** (SHA-256 de Argon2id(clave, sal_equipo)) y
//   `K_cfg` de cada consola sale de la sal del cliente **en esa consola**: la
//   de aquí (`sal_cliente`) y, con otras consolas, la de cada una (la dice el
//   resumen del equipo, `consolas[].sal_cliente`) en `k_cfg_consolas`.
// - El servidor no guarda ningún verificador del cliente: solo su sal, que no
//   cambia (así la clave anterior sigue sirviendo, con la misma sal, en los
//   equipos que aún no han aplicado el cambio: el cliente queda «a medias»
//   hasta que todos lo apliquen).
import { aB64, borrar } from "./cripto/bytes";
import { etiquetaValida, kCfg, materialCliente, pruebaAdmin, verificador, type Argon2 } from "./cripto/claves";
import type { Equipo, Orden } from "./tipos";

/** Lo que sale de la clave nueva para un cliente: su K_cfg aquí y la de otras sales (se calculan una vez). */
export class ClaveNueva {
  private kcfgs = new Map<string, Uint8Array>();
  constructor(
    private argon2: Argon2,
    private clave: string,
    readonly salCliente: string,
  ) {}

  /** K_cfg con la sal `sal` (la de este cliente si no se dice). */
  async kcfg(sal = this.salCliente): Promise<Uint8Array> {
    let k = this.kcfgs.get(sal);
    if (!k) {
      const m = await materialCliente(this.argon2, this.clave, sal);
      k = kCfg(m);
      borrar(m);
      this.kcfgs.set(sal, k);
    }
    return k;
  }

  /**
   * El cuerpo de `cambiar_clave_admin` para un equipo: `{ verificador, k_cfg,
   * k_cfg_consolas? }`. `k_cfg_consolas` lleva la K_cfg nueva de cada **otra**
   * consola cuya sal se sabe; sin sal (un agente anterior no la da), el equipo
   * pone la nueva en las que tenían la misma K_cfg que esta.
   */
  async cuerpo(equipo: Pick<Equipo, "sal_equipo" | "resumen">): Promise<{ verificador: string; k_cfg: string; k_cfg_consolas?: Record<string, string> }> {
    const prueba = await pruebaAdmin(this.argon2, this.clave, equipo.sal_equipo);
    const ver = aB64(verificador(prueba));
    borrar(prueba);
    const otras: Record<string, string> = {};
    for (const c of equipo.resumen?.consolas ?? []) {
      if (c.esta || !c.sal_cliente || !c.identidad) continue;
      otras[c.identidad] = aB64(await this.kcfg(c.sal_cliente));
    }
    return { verificador: ver, k_cfg: aB64(await this.kcfg()), ...(Object.keys(otras).length ? { k_cfg_consolas: otras } : {}) };
  }

  /** Las otras consolas que gestionan el equipo (sus nombres), para avisar de que allí también hará falta la clave nueva. */
  static otrasConsolas(equipos: Pick<Equipo, "resumen">[]): string[] {
    const n = new Set<string>();
    for (const e of equipos) for (const c of e.resumen?.consolas ?? []) if (!c.esta) n.add(c.nombre || c.url);
    return [...n];
  }

  olvidar() {
    for (const k of this.kcfgs.values()) borrar(k);
    this.kcfgs.clear();
    this.clave = "";
  }
}

/** Los equipos a los que se manda: confirmados y gestionados desde aquí. */
export const equiposDelCambio = (equipos: Equipo[]) => equipos.filter((e) => e.confirmado && e.modo === "gestionado");

/**
 * Qué clave tiene cada equipo según su etiqueta (la sube el equipo con su K_cfg):
 * la actual (se le manda el cambio), ya la nueva (no hace falta) u otra (no
 * se puede: p. ej. aún tiene pendiente un cambio anterior).
 */
export function repartir<E extends Pick<Equipo, "id" | "box_pub" | "sign_pub" | "etiqueta">>(equipos: E[], kcfgActual: Uint8Array, kcfgNueva: Uint8Array) {
  const conActual: E[] = [];
  const yaNueva: E[] = [];
  const otra: E[] = [];
  for (const e of equipos) {
    if (etiquetaValida(kcfgActual, e)) conActual.push(e);
    else if (etiquetaValida(kcfgNueva, e)) yaNueva.push(e);
    else otra.push(e);
  }
  return { conActual, yaNueva, otra };
}

export type EstadoCambio = "aplicada" | "pendiente" | "en_marcha" | "rechazada" | "cancelada";

/** En qué va el cambio de un equipo, según su última orden `cambiar_clave_admin`. */
export function estadoCambio(o: Pick<Orden, "estado">): EstadoCambio {
  switch (o.estado) {
    case "hecha":
      return "aplicada";
    case "pendiente":
    case "entregada":
      return "pendiente";
    case "en_marcha":
      return "en_marcha";
    case "cancelada":
    case "caducada":
      return "cancelada";
    default:
      return "rechazada";
  }
}

/**
 * Los equipos con el cambio aún por aplicar (el cliente «a medias»): de las
 * órdenes `cambiar_clave_admin` sin terminar, la última de cada equipo.
 */
export function pendientesDeCambio(ordenes: (Pick<Orden, "tipo" | "estado" | "emitida" | "caduca"> & { equipo?: string })[]): { equipo: string; caduca: string }[] {
  const ultima = new Map<string, { estado: string; emitida: string; caduca: string }>();
  for (const o of ordenes) {
    if (o.tipo !== "cambiar_clave_admin" || !o.equipo) continue;
    const prev = ultima.get(o.equipo);
    if (!prev || o.emitida > prev.emitida) ultima.set(o.equipo, o);
  }
  return [...ultima.entries()].filter(([, o]) => ["pendiente", "entregada", "en_marcha"].includes(o.estado)).map(([equipo, o]) => ({ equipo, caduca: o.caduca }));
}

/** Una clave fuerte: 5 grupos de 5 (≈ 125 bits), sin letras que se confundan (como al dar de alta el primer equipo). */
export function claveGenerada(aleatorio: (n: number) => Uint8Array): string {
  const letras = "ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
  const b = aleatorio(25);
  const t = Array.from(b, (x) => letras[x % 32]).join("");
  b.fill(0);
  return t.match(/.{5}/g)!.join("-");
}
