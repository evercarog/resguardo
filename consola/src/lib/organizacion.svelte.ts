// Plan 0.8.0, D1 y D2: cambiar las sedes, la sede de equipos y destinos, las
// categorías de etiqueta y el orden a mano (lógica pura en ./sedes.ts). Todo son
// datos comunes del cliente: se guardan aquí al momento y se reparten a los
// equipos (lo que no pide clave, solo; la sede de un equipo o de un destino, con la
// clave de administración desde el aviso de «Datos comunes», como el tipo de un destino).
import * as api from "./api";
import { actual, cargarCliente } from "./estado.svelte";
import { sincronizar } from "./datosComunes.svelte";
import { idNuevo, MAX_ORDEN, type CategoriaEtiqueta, type Sede } from "./sedes";

type Entrada = { clave: string; valor: unknown };

/** Guarda los cambios (de 100 en 100), recarga el cliente y reparte lo que no pide clave. */
export async function guardarOrganizacion(cliente: string, entradas: Entrada[]): Promise<void> {
  for (let i = 0; i < entradas.length; i += 100) await api.ponerDatosComunes(cliente, entradas.slice(i, i + 100));
  await cargarCliente(cliente, { silencioso: true });
  if (actual.id === cliente) void sincronizar().catch(() => {});
}

/** Crear o cambiar una sede. Con `principal`, las demás dejan de serlo. */
export async function guardarSede(cliente: string, s: { id?: string; nombre: string; notas: string; principal: boolean }): Promise<string> {
  const id = s.id ?? idNuevo(s.nombre);
  const entradas: Entrada[] = [{ clave: `sede:${id}`, valor: { nombre: s.nombre.trim(), ...(s.notas.trim() ? { notas: s.notas.trim() } : {}), ...(s.principal ? { principal: true } : {}) } }];
  if (s.principal)
    for (const o of actual.organizacion.sedes.filter((x: Sede) => x.principal && x.id !== id))
      entradas.push({ clave: `sede:${o.id}`, valor: { nombre: o.nombre, ...(o.notas ? { notas: o.notas } : {}) } });
  await guardarOrganizacion(cliente, entradas);
  return id;
}

/** Quitar una sede: sus equipos y destinos vuelven a la de por defecto (se quitan sus asignaciones). */
export async function quitarSede(cliente: string, id: string): Promise<void> {
  const o = actual.organizacion;
  const entradas: Entrada[] = [{ clave: `sede:${id}`, valor: null }];
  for (const [e, s] of Object.entries(o.equipos)) if (s === id) entradas.push({ clave: `sede.equipo:${e}`, valor: null });
  for (const [d, s] of Object.entries(o.destinos)) if (s === id) entradas.push({ clave: `sede.destino:${d}`, valor: null });
  await guardarOrganizacion(cliente, entradas);
}

/** La sede de varios equipos a la vez (`null`: la de por defecto). */
export const ponerSedeEquipos = (cliente: string, equipos: string[], sede: string | null) =>
  guardarOrganizacion(
    cliente,
    equipos.map((e) => ({ clave: `sede.equipo:${e}`, valor: sede ? { sede } : null })),
  );

/** La sede de un destino del catálogo (`null`: la de su equipo, o ninguna). */
export const ponerSedeDestino = (cliente: string, clave: string, sede: string | null) => guardarOrganizacion(cliente, [{ clave: `sede.destino:${clave}`, valor: sede ? { sede } : null }]);

/** Crear o cambiar una categoría de etiquetas. */
export async function guardarCategoria(cliente: string, c: { id?: string; nombre: string; etiquetas: string[] }): Promise<string> {
  const id = c.id ?? idNuevo(c.nombre);
  await guardarOrganizacion(cliente, [{ clave: `etiqueta.categoria:${id}`, valor: { nombre: c.nombre.trim(), etiquetas: c.etiquetas } satisfies Omit<CategoriaEtiqueta, "id"> }]);
  return id;
}

export const quitarCategoria = (cliente: string, id: string) => guardarOrganizacion(cliente, [{ clave: `etiqueta.categoria:${id}`, valor: null }]);

/** El orden a mano de los equipos (como mucho 500). */
export const ponerOrden = (cliente: string, ids: string[]) => guardarOrganizacion(cliente, [{ clave: "orden.equipos:todos", valor: { ids: ids.slice(0, MAX_ORDEN) } }]);
