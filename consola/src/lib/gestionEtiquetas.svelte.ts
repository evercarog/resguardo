// 0.8.1 (B2, plan-mejoras 15a): renombrar, borrar y unir una etiqueta en todos los equipos
// del cliente (lógica pura en ./gestionEtiquetas.ts).
//
// Una orden `etiquetas_equipo` por equipo afectado (las guarda el equipo y las ven igual
// todas sus consolas, en su historial común y en la auditoría de cada servidor); con un
// agente anterior, `PATCH` en esta consola, como en la ficha. Primero se mandan todas y
// luego se espera un poco a que contesten: lo que no contesta (sin conexión) queda pedido
// y se aplica al volver. Después, el color, la plantilla y la categoría pasan a la nueva.
import * as api from "./api";
import { actual, cargarCliente, puede } from "./estado.svelte";
import { admiteDatosEquipo } from "./datosEquipo";
import { mandarOrden } from "./ordenar";
import { sincronizar } from "./datosComunes.svelte";
import { guardarOrganizacion } from "./organizacion.svelte";
import { ajusteDe, cuerpoAjuste, mismaEtiqueta } from "./etiquetasGrupos";
import { ajusteTrasCambio, cambiosEnEquipos, categoriasTrasCambio } from "./gestionEtiquetas";
import type * as T from "./tipos";

const FINALES = ["hecha", "fallida", "rechazada", "cancelada", "caducada"];

/**
 * Las etiquetas pedidas a un equipo que aún no contestó (cliente:equipo → lista). Un
 * segundo cambio antes de que conteste parte de lo pedido, no de lo que tenía: si no,
 * la segunda orden desharía la primera al aplicarse las dos en orden.
 */
const pedidas = $state<Record<string, string[]>>({});
const iguales = (a: readonly string[] | undefined, b: readonly string[]) => !!a && a.length === b.length && a.every((x, i) => x === b[i]);

/** Los equipos con lo que ya se les pidió y aún no aplicaron (para contar y para cambiar). */
export function conPedidas<E extends T.Equipo>(equipos: readonly E[]): E[] {
  return equipos.map((e) => {
    const k = `${actual.id}:${e.id}`;
    const p = pedidas[k];
    if (!p || iguales(e.etiquetas, p)) return e;
    return { ...e, etiquetas: p };
  });
}

export interface ResultadoEquipo {
  equipo: T.Equipo;
  /** `hecho`: ya lo tiene; `pedido`: sin contestar aún (se aplica al volver); `fallo`: no se pudo. */
  estado: "hecho" | "pedido" | "fallo";
  texto: string;
}

export interface ResultadoCambio {
  equipos: ResultadoEquipo[];
  /** Lo que no se pudo de los ajustes o la categoría (las etiquetas ya están). */
  avisos: string[];
}

/**
 * Cambia `de` por `a` en todos los equipos que la llevan (`a: null`, la quita) y pasa a
 * la nueva su color, su plantilla y su categoría. `alPaso` recibe «3 de 5 equipos…».
 */
export async function cambiarEtiqueta(de: string, a: string | null, alPaso?: (texto: string) => void, segundos = 20): Promise<ResultadoCambio> {
  const cliente = actual.cliente;
  if (!cliente) throw new Error("Sin cliente.");
  const cambios = cambiosEnEquipos(conPedidas(actual.equipos), de, a);
  const total = cambios.length;
  const res: ResultadoEquipo[] = [];
  const esperando: { equipo: T.Equipo; orden: string; etiquetas: string[] }[] = [];
  for (const [i, c] of cambios.entries()) {
    alPaso?.(`Pidiéndolo a ${c.equipo.nombre} (${i + 1} de ${total})…`);
    try {
      if (admiteDatosEquipo(c.equipo)) {
        const o = await mandarOrden({ cliente, equipo: c.equipo, tipo: "etiquetas_equipo", cuerpo: { etiquetas: c.etiquetas } });
        esperando.push({ equipo: c.equipo, orden: o.id, etiquetas: c.etiquetas });
      } else {
        await api.ponerEtiquetas(cliente.id, c.equipo.id, c.etiquetas);
        res.push({ equipo: c.equipo, estado: "hecho", texto: "Cambiada en esta consola (su agente aún no guarda las etiquetas para todas: actualízalo)." });
      }
    } catch (err) {
      res.push({ equipo: c.equipo, estado: "fallo", texto: (err as Error).message });
    }
  }
  // Esperar a que contesten (todas a la vez, no una tras otra).
  const fin = Date.now() + segundos * 1000;
  while (esperando.length && Date.now() < fin) {
    alPaso?.(`${plural(res.filter((r) => r.estado === "hecho").length, total)} ya lo tienen; esperando a ${esperando.map((x) => x.equipo.nombre).join(", ")}…`);
    await new Promise((r) => setTimeout(r, 1500));
    for (const x of [...esperando]) {
      try {
        const o = (await api.ordenesEquipo(cliente.id, x.equipo.id, 10)).find((y) => y.id === x.orden);
        if (!o || !FINALES.includes(o.estado)) continue;
        esperando.splice(esperando.indexOf(x), 1);
        res.push(o.estado === "hecha" ? { equipo: x.equipo, estado: "hecho", texto: o.mensaje ?? "Hecho." } : { equipo: x.equipo, estado: "fallo", texto: o.mensaje ?? "No pudo hacerlo." });
      } catch {
        // Se vuelve a mirar en la siguiente vuelta.
      }
    }
  }
  for (const r of res) delete pedidas[`${cliente.id}:${r.equipo.id}`];
  for (const x of esperando) pedidas[`${cliente.id}:${x.equipo.id}`] = x.etiquetas;
  for (const x of esperando) res.push({ equipo: x.equipo, estado: "pedido", texto: "Sin conexión ahora: queda pedido y lo aplicará en cuanto conteste." });

  // El color, la plantilla y la categoría pasan a la nueva (o se quitan al borrar).
  const avisos: string[] = [];
  alPaso?.("Pasando el color, la plantilla y la categoría…");
  try {
    await moverAjustes(cliente, de, a, avisos);
  } catch (err) {
    avisos.push(`El color y la plantilla no se pudieron pasar: ${(err as Error).message}`);
  }
  try {
    const cats = categoriasTrasCambio(actual.organizacion.categorias, de, a);
    if (cats.length) await guardarOrganizacion(cliente.id, cats.map((c) => ({ clave: `etiqueta.categoria:${c.id}`, valor: { nombre: c.nombre, etiquetas: c.etiquetas } })));
  } catch (err) {
    avisos.push(`La categoría no se pudo cambiar: ${(err as Error).message}`);
  }
  void cargarCliente(cliente.id, { silencioso: true });
  return { equipos: res, avisos };
}

/** Los ajustes de la vieja, a la nueva; la vieja se queda sin color ni plantilla (y sin avisos, si los puede quitar). */
async function moverAjustes(cliente: T.Cliente, de: string, a: string | null, avisos: string[]) {
  const vieja = ajusteDe(de, actual.etiquetas);
  if (!vieja) return;
  const propietario = puede.propietario(cliente.rol);
  // Cambiar solo las mayúsculas: es la misma etiqueta, con su nombre nuevo.
  if (a && mismaEtiqueta(de, a)) {
    actual.etiquetas = await api.ponerAjusteEtiqueta(cliente.id, cuerpoAjuste(a, vieja.color ?? null, vieja.plantilla ?? null, vieja.avisos));
    void sincronizar();
    return;
  }
  if (a) {
    const n = ajusteTrasCambio(vieja, ajusteDe(a, actual.etiquetas), a, propietario);
    if (n) actual.etiquetas = await api.ponerAjusteEtiqueta(cliente.id, cuerpoAjuste(a, n.color ?? null, n.plantilla ?? null, n.avisos));
  }
  // La vieja: sin color ni plantilla (lo verán así todas las consolas). Los avisos son de
  // cada consola y solo los cambia el propietario: si no lo es, se quedan.
  const quedanAvisos = !propietario && vieja.avisos;
  if (vieja.color != null || vieja.plantilla || (propietario && vieja.avisos)) {
    actual.etiquetas = await api.ponerAjusteEtiqueta(cliente.id, cuerpoAjuste(de, null, null, quedanAvisos ? vieja.avisos : null));
  }
  if (quedanAvisos) avisos.push(`Los avisos propios de «${de}» se quedan con ese nombre: los cambia el propietario del cliente.`);
  void sincronizar();
}

const plural = (n: number, total: number) => `${n} de ${total} ${total === 1 ? "equipo" : "equipos"}`;
