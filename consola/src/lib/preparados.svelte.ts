// La lista de equipos preparados de «Añadir equipo», al día cada pocos segundos.
//
// Antes, la página lo hacía en un `$effect` que llamaba a `cargarLista()`, y
// esta leía `lista` (para saber cuáles se acababan de unir) ANTES de su primer
// `await`: el efecto quedaba suscrito a `lista`, cada respuesta la cambiaba y el
// efecto volvía a pedir. Un bucle de peticiones sin pausa (cientos por segundo)
// que en segundos pasaba el límite por cuenta del servidor (1200 por minuto):
// desde ahí TODO daba «Demasiados intentos», también «Vincular este servidor»
// o «Generar el código», aunque se acabara de reiniciar el servicio.
//
// Aquí: la primera carga y el sondeo van fuera del seguimiento (`untrack`), no
// se pisan dos cargas a la vez y, con la pestaña oculta, no se pregunta.
// scripts/vectores-emparejar.ts cuenta las peticiones.
import { untrack } from "svelte";
import type { Preparado } from "./tipos";

export interface OpcionesSondeo {
  /** `GET /api/clientes/{c}/emparejamientos`. */
  pedir: () => Promise<Preparado[]>;
  /** Un preparado acaba de pasar a «unido» (es lo que se está esperando). */
  alUnirse?: (p: Preparado) => void;
  /** Cada cuánto se vuelve a preguntar (5 s). */
  cadaMs?: number;
  /** ¿Se ve la página? (sin pestaña visible no se pregunta). */
  visible?: () => boolean;
  /** Envuelve las cargas del sondeo (la consola: `enFondo`, sin indicador de actividad). */
  fondo?: <T>(f: () => T) => T;
}

export class Preparados {
  lista = $state<Preparado[] | null>(null);
  #cargando: Promise<void> | null = null;

  /** Pide la lista (una sola petición a la vez: si ya hay una en marcha, espera a esa). */
  cargar(o: Pick<OpcionesSondeo, "pedir" | "alUnirse">): Promise<void> {
    this.#cargando ??= (async () => {
      try {
        const nueva = await o.pedir();
        const antes = new Map((untrack(() => this.lista) ?? []).map((p) => [p.id, p.estado]));
        for (const p of nueva) if (p.estado === "unido" && antes.get(p.id) && antes.get(p.id) !== "unido") o.alUnirse?.(p);
        this.lista = nueva;
      } catch {
        this.lista ??= [];
      } finally {
        this.#cargando = null;
      }
    })();
    return this.#cargando;
  }

  /**
   * Carga ya y luego cada `cadaMs`. Se llama desde un `$effect` (que solo debe depender
   * del cliente): devuelve la limpieza. Nada de lo que hace aquí suscribe al efecto.
   */
  seguir(o: OpcionesSondeo): () => void {
    return untrack(() => {
      void this.cargar(o);
      const visible = o.visible ?? (() => typeof document === "undefined" || document.visibilityState === "visible");
      const t = setInterval(() => visible() && void (o.fondo ?? ((f) => f()))(() => this.cargar(o)), o.cadaMs ?? 5000);
      return () => clearInterval(t);
    });
  }
}
