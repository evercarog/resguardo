// 15nn: los clientes con los que comparar el nombre de uno nuevo. Para el
// propietario del servidor, todos los del servidor (también aquellos de los que
// no es miembro y los archivados); si no, los suyos. Se piden al primer uso y
// se vuelven a pedir pasados 30 s (se acaba de crear o borrar uno).
import * as api from "$lib/api";
import { app } from "$lib/estado.svelte";
import type { ClienteConNombre } from "$lib/nombresClientes";

let delServidor = $state<ClienteConNombre[] | null>(null);
let pedidoEn = 0;

export function todosLosClientes(): ClienteConNombre[] {
  if (app.cuenta?.superusuario && Date.now() - pedidoEn > 30_000) {
    pedidoEn = Date.now();
    api
      .clientesDelServidor({ invisible: true })
      .then((d) => (delServidor = d.clientes))
      .catch(() => (pedidoEn = 0));
  }
  return delServidor ?? app.clientes.map((c) => ({ id: c.id, nombre: c.nombre }));
}
