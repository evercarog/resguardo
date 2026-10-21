// Plan 0.8.1, C4 (15d): guardar la infraestructura del cliente (lógica pura en
// ./infra.ts). Se guarda entera en su servidor (`PUT …/infraestructura`,
// administradores) y se pone al momento en `actual.infra`.
//
// 0.8.2 (A5): el servidor apunta lo que cambió como datos comunes «por enviar»;
// aquí se reparte enseguida a los equipos lo que no pide clave (los servidores y
// sus discos). Dónde está cada equipo o destino pide la clave de administración:
// se reparte desde el aviso de «Lo mismo en todas las consolas», como las sedes.
import * as api from "./api";
import { actual } from "./estado.svelte";
import { leerInfra, paraGuardar, type Infraestructura } from "./infra";

export async function guardarInfra(cliente: string, i: Infraestructura): Promise<Infraestructura> {
  const r = leerInfra(await api.ponerInfraestructura(cliente, paraGuardar(i)));
  if (actual.id === cliente) {
    actual.infra = r;
    void import("./datosComunes.svelte").then((m) => m.sincronizar()).catch(() => {});
  }
  return r;
}
