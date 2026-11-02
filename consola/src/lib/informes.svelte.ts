// El último informe de cada equipo del cliente, para las páginas que listan
// repositorios de varios equipos (versiones, tamaño, verificación…): el
// resumen de un agente anterior no los trae. Una sola petición (GET
// …/informes, v1.8); con un servidor anterior (404), uno por equipo.
//
// 16x (7), medido con 200 equipos (docs/rendimiento.md): todos los informes son
// ~12 MB. Se piden enteros una vez por cliente (no cada vez que cambia la lista de
// equipos: antes, dos veces al entrar en Estado) y los refrescos piden solo los que
// llegaron después (`?desde=`); un servidor anterior no lo mira y los manda todos.
import * as api from "./api";
import { desdeInformes } from "./informesDesde";
import type { Informe } from "./tipos";

export const ultimos = $state<{ cliente: string; porEquipo: Record<string, Informe | null> }>({ cliente: "", porEquipo: {} });

/** El cliente cuyos informes están pedidos o cargados (una sola carga entera por cliente). */
let cargando = "";
/** Con un servidor anterior (sin `GET …/informes`): uno por equipo, y los que ya se pidieron. */
let porEquipo: { cliente: string; pedidos: Set<string> } | null = null;
/** Cuándo se pidieron por última vez (para ponerlos al día al volver al cliente). */
let pedidosEn = 0;
/** Al volver a una pantalla del cliente, si hace más que esto: solo los nuevos. */
const AL_VOLVER_MS = 30_000;

function juntar(xs: (Informe & { equipo: string })[], base: Record<string, Informe | null>) {
  const m: Record<string, Informe | null> = { ...base };
  for (const x of xs) m[x.equipo] = { recibido: x.recibido, datos: x.datos };
  return m;
}

/** Vuelve a pedir los últimos informes del cliente (ya cargados): algo cambió. Solo los nuevos. */
export async function recargarInformes(cliente: string) {
  if (!cliente || ultimos.cliente !== cliente || porEquipo?.cliente === cliente) return;
  pedidosEn = Date.now();
  try {
    const xs = await api.ultimosInformes(cliente, desdeInformes(ultimos.porEquipo));
    if (ultimos.cliente !== cliente || !xs.length) return;
    ultimos.porEquipo = juntar(xs, ultimos.porEquipo);
  } catch {
    /* sin conexión o un servidor anterior (404): se queda lo que había */
  }
}

export async function cargarInformes(cliente: string, equipos: string[]) {
  if (!cliente) return;
  // Un servidor anterior: los equipos que aún no se pidieron, uno por uno.
  if (porEquipo?.cliente === cliente) return void pedirUnoAUno(cliente, equipos);
  if (cliente === cargando) {
    if (Date.now() - pedidosEn > AL_VOLVER_MS) void recargarInformes(cliente);
    return;
  }
  cargando = cliente;
  pedidosEn = Date.now();
  if (ultimos.cliente !== cliente) ultimos.porEquipo = {};
  ultimos.cliente = cliente;
  porEquipo = null;
  try {
    const xs = await api.ultimosInformes(cliente);
    if (ultimos.cliente === cliente) ultimos.porEquipo = juntar(xs, {});
  } catch (err) {
    if (!(err instanceof api.ApiError && err.estado === 404)) {
      cargando = "";
      return;
    }
    porEquipo = { cliente, pedidos: new Set() };
    pedirUnoAUno(cliente, equipos);
  }
}

function pedirUnoAUno(cliente: string, equipos: string[]) {
  const p = porEquipo;
  if (!p) return;
  for (const id of equipos) {
    if (p.pedidos.has(id)) continue;
    p.pedidos.add(id);
    api
      .equipo(cliente, id)
      .then((x) => ultimos.cliente === cliente && (ultimos.porEquipo[id] = x.ultimo_informe ?? null))
      .catch(() => p.pedidos.delete(id));
  }
}
