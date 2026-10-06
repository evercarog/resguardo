// Estado compartido de la consola: servidor, cuenta y el cliente abierto.
// Nada de esto es secreto: la cookie de sesión es HttpOnly y las claves y
// contraseñas nunca pasan por aquí.
import { goto } from "$app/navigation";
import * as api from "./api";
import { enFondo } from "./actividad.svelte";
import { conDatosDelEquipo } from "./datosEquipo";
import type * as T from "./tipos";

export const app = $state({
  servidor: null as T.Servidor | null,
  cuenta: null as T.Cuenta | null,
  /** Lista de clientes con su número de equipos y avisos. */
  clientes: [] as T.ClienteResumen[],
  listo: false,
});

/** El cliente abierto (ruta /c/[c]) y lo que se pinta en casi todas sus pantallas. */
export const actual = $state({
  id: "" as string,
  cliente: null as T.Cliente | null,
  equipos: [] as T.Equipo[],
  avisosAbiertos: 0,
  pendientes: 0,
  /** v1.52: los ajustes de las etiquetas de sus equipos (color, plantilla por defecto, avisos). */
  etiquetas: [] as T.AjusteEtiqueta[],
  cargando: false,
  error: "" as string,
  /** Código del último error al cargarlo («prohibido», «no_existe», «red»…), para su ilustración. */
  errorCodigo: "" as string,
  /** Marca de tiempo de la última carga (para «actualizado hace…»). */
  cargado: 0,
});

/** v1.34: la dirección que se da a los agentes y a las otras consolas. En una consola en
 * internet es otra (https://agentes.<dominio>, con la autoridad TLS propia que fijan al
 * vincularse); si no, la de esta consola. */
export function urlAgentes(): string {
  return app.servidor?.url_agentes || (typeof location !== "undefined" ? location.origin : "");
}

/** Para tiempos relativos que se refrescan solos. */
export const reloj = $state({ ahora: Date.now() });
if (typeof window !== "undefined") setInterval(() => (reloj.ahora = Date.now()), 15_000);

let volviendoAEntrar = false;
api.onSinSesion(() => {
  if (volviendoAEntrar || location.pathname.startsWith("/entrar")) return;
  volviendoAEntrar = true;
  app.cuenta = null;
  const volver = location.pathname + location.search;
  void goto(`/entrar?volver=${encodeURIComponent(volver)}`).finally(() => (volviendoAEntrar = false));
});

export async function cargarClientes() {
  app.clientes = await api.clientes();
}

/** Carga el cliente y su resumen («Estado»). Silencioso si ya había datos. */
export async function cargarCliente(id: string, opciones: { silencioso?: boolean } = {}) {
  const cambia = actual.id !== id;
  if (cambia) {
    actual.id = id;
    actual.cliente = null;
    actual.equipos = [];
    actual.etiquetas = [];
    actual.error = "";
    // 0 = aún sin la primera carga de este cliente: las pantallas enseñan su esqueleto, nunca «no hay…».
    actual.cargado = 0;
  }
  if (!opciones.silencioso) actual.cargando = true;
  try {
    const pedirlo = () => Promise.all([cambia || !actual.cliente ? api.cliente(id) : Promise.resolve(actual.cliente), api.resumen(id)]);
    // Los refrescos silenciosos son de fondo: «Actualizando…», no la barra de carga.
    const [cliente, resumen] = await (opciones.silencioso && !cambia ? enFondo(pedirlo) : pedirlo());
    if (actual.id !== id) return;
    actual.cliente = cliente;
    // v1.4x: el nombre y las etiquetas que tiene puestos el equipo mandan (un servidor anterior no los copia).
    actual.equipos = resumen.equipos.map(conDatosDelEquipo);
    actual.avisosAbiertos = resumen.avisos_abiertos;
    actual.pendientes = resumen.pendientes;
    actual.etiquetas = resumen.etiquetas ?? [];
    actual.cargado = Date.now();
    actual.error = "";
  } catch (e) {
    if (actual.id === id) (actual.error = (e as Error).message), (actual.errorCodigo = (e as { codigo?: string }).codigo ?? "");
  } finally {
    if (actual.id === id) actual.cargando = false;
  }
}

export const puede = {
  /** Mandar órdenes (no lectura). */
  ordenar: (rol: T.Rol | undefined) => rol === "propietario" || rol === "administrador" || rol === "tecnico",
  administrar: (rol: T.Rol | undefined) => rol === "propietario" || rol === "administrador",
  propietario: (rol: T.Rol | undefined) => rol === "propietario",
};

export const NOMBRE_ROL: Record<T.Rol, string> = {
  propietario: "Propietario",
  administrador: "Administrador",
  tecnico: "Técnico",
  lectura: "Solo lectura",
};
