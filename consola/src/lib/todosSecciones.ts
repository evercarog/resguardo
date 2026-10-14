// Plan 0.8.0, D4 (14xx): las secciones de «Todos los clientes» (Copias, Equipos,
// Avisos, Órdenes en espera, En curso y Restauraciones) con la misma lógica que las
// de cada cliente, juntando lo de todos con su cliente. Sin runas: lo prueba
// scripts/vectores-todos.ts.
//
// Cómo se piden: lo de los equipos (resumen, informes, sedes, lo que está en
// marcha) ya llega en `GET /api/panel`; los avisos, las órdenes con espera de esta
// consola y las restauraciones, en `GET /api/panel/{avisos,ordenes,restauraciones}`,
// una petición para todos (el servidor solo da los clientes de los que la cuenta es
// miembro). Con un servidor anterior (404), cliente a cliente.
import type { PanelCliente, PanelDe } from "./global";
import { informesDe } from "./global";
import { filasCopias, type FilaCopia } from "./copiasCliente";
import { filasEnEspera, type FilaEspera } from "./espera";
import { juntar, type ConMotivo } from "./restauraciones";
import { leerOrganizacion, sedeDeEquipo } from "./sedes";
import { saludEquipo, sinDuplicados, PESO, type Tono } from "./salud";
import { grupoSistema, sistemaDe } from "./datosBasicos";
import type { Aviso, Equipo, Orden, Restauraciones } from "./tipos";

export type ClienteFila = Pick<PanelCliente, "id" | "nombre" | "rol" | "marca">;
const cli = (c: PanelCliente): ClienteFila => ({ id: c.id, nombre: c.nombre, rol: c.rol, marca: c.marca ?? null });

/** Todas las copias de todos los clientes, con su cliente (y los pasos de su camino). */
export function copiasTodos(clientes: readonly PanelCliente[], ahora: number): (FilaCopia & { cliente: ClienteFila })[] {
  return clientes.flatMap((c) => filasCopias(c.equipos, informesDe(c), ahora, [], { cliente: c.id }).map((x) => ({ ...x, clave: `${c.id}|${x.clave}`, cliente: cli(c) })));
}

export interface FilaEquipoTodos {
  clave: string;
  cliente: ClienteFila;
  equipo: Equipo;
  /** La sede («Oficina Norte»), si el cliente tiene sedes; `sedeAsignada: false` si es la de por defecto. */
  sede: string | null;
  sedeAsignada: boolean;
  sistema: string;
  grupoSistema: string;
  tono: Tono;
  estado: string;
}

/** Todos los equipos de todos los clientes (sin duplicados sin confirmar ni trasladados), lo urgente primero. */
export function equiposTodos(clientes: readonly PanelCliente[], ahora: number): FilaEquipoTodos[] {
  return clientes
    .flatMap((c) => {
      const o = leerOrganizacion(c.organizacion);
      return sinDuplicados(c.equipos)
        .filter((e) => e.modo !== "trasladado")
        .map((e): FilaEquipoTodos => {
          const s = sedeDeEquipo(o, e.id);
          const salud = saludEquipo(e, ahora);
          return { clave: `${c.id}|${e.id}`, cliente: cli(c), equipo: e, sede: s.sede?.nombre ?? null, sedeAsignada: s.asignada, sistema: sistemaDe(e), grupoSistema: grupoSistema(e), tono: salud.tono, estado: salud.texto };
        });
    })
    .sort((a, b) => PESO[a.tono] - PESO[b.tono] || a.cliente.nombre.localeCompare(b.cliente.nombre, "es") || a.equipo.nombre.localeCompare(b.equipo.nombre, "es", { numeric: true }));
}

export interface AvisoTodos extends Aviso {
  clave: string;
  cliente: ClienteFila;
  equipoNombre: string | null;
}

/** Los avisos sin revisar de todos, del más reciente al más antiguo. */
export function avisosTodos(p: PanelDe<Aviso[]> | null | undefined, clientes: readonly PanelCliente[]): AvisoTodos[] {
  const porId = new Map(clientes.map((c) => [c.id, c]));
  return (p?.clientes ?? [])
    .flatMap((x) => {
      const c = porId.get(x.id);
      const base: ClienteFila = c ? cli(c) : { id: x.id, nombre: x.nombre, rol: x.rol, marca: null };
      return (x.datos ?? []).filter((a) => !a.visto_por).map((a) => ({ ...a, clave: `${x.id}|${a.id}`, cliente: base, equipoNombre: c?.equipos.find((e) => e.id === a.equipo)?.nombre ?? null }));
    })
    .sort((a, b) => b.creado.localeCompare(a.creado));
}

export type EsperaTodos = FilaEspera & { clave: string; cliente: ClienteFila };

/**
 * Las órdenes que esperan su turno en todos los clientes: las de esta consola (`propias`,
 * de `/api/panel/ordenes`) y las que los equipos dicen tener de otras consolas (su
 * resumen), sin repetir. De la que antes se aplica a la que más tarda.
 */
export function esperaTodos(clientes: readonly PanelCliente[], propias: PanelDe<Orden[]> | null | undefined, ahora: number): EsperaTodos[] {
  const deCliente = new Map((propias?.clientes ?? []).map((x) => [x.id, x.datos ?? []]));
  return clientes
    .flatMap((c) => filasEnEspera(deCliente.get(c.id) ?? [], c.equipos, ahora).map((f) => ({ ...f, clave: `${c.id}|${f.id}`, cliente: cli(c) })))
    .sort((a, b) => (a.aplica || "").localeCompare(b.aplica || ""));
}

export type RestauracionTodos = ConMotivo & { clave: string; cliente: ClienteFila; equipoNombre: string | null };

/** Las restauraciones de todos, la más reciente primero. */
export function restauracionesTodos(p: PanelDe<Restauraciones> | null | undefined, clientes: readonly PanelCliente[]): RestauracionTodos[] {
  const porId = new Map(clientes.map((c) => [c.id, c]));
  return (p?.clientes ?? [])
    .flatMap((x) => {
      const c = porId.get(x.id);
      const base: ClienteFila = c ? cli(c) : { id: x.id, nombre: x.nombre, rol: x.rol, marca: null };
      return x.datos ? juntar(x.datos).map((r) => ({ ...r, clave: `${x.id}|${r.equipo}|${r.id}`, cliente: base, equipoNombre: c?.equipos.find((e) => e.id === r.equipo)?.nombre ?? null })) : [];
    })
    .sort((a, b) => b.hora.localeCompare(a.hora));
}
