// Plan 0.8.1, C4 (15d): guardar la infraestructura del cliente (lógica pura en
// ./infra.ts). Se guarda entera en su servidor (`PUT …/infraestructura`,
// administradores) y se pone al momento en `actual.infra`.
import * as api from "./api";
import { actual } from "./estado.svelte";
import { leerInfra, paraGuardar, type Infraestructura } from "./infra";

export async function guardarInfra(cliente: string, i: Infraestructura): Promise<Infraestructura> {
  const r = leerInfra(await api.ponerInfraestructura(cliente, paraGuardar(i)));
  if (actual.id === cliente) actual.infra = r;
  return r;
}
