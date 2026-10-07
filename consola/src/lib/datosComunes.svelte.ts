// 0.7.26 (bloque 8): repartir los datos comunes del cliente a sus equipos y traer
// lo de las demás consolas (lógica pura en ./datosComunes.ts; diseño en
// docs/consolas-multiples.md §6.5).
//
// - Lo que no pide clave (colores, nombres de destinos, plantillas cifradas) se
//   manda solo: al abrir el cliente, al cambiarlo aquí y cuando llega un cambio.
//   Lo de antes de compartir va como «semilla»: nunca pisa nada en otra consola
//   (si allí es distinto, allí sale como diferencia).
// - El tipo y las marcas de un destino se mandan con la clave de administración.
// - Las diferencias y las plantillas de otra consola esperan a una persona.
// Solo administradores y propietarios (como cambiar esas cosas aquí).
import * as api from "./api";
import { argon2Navegador } from "./cripto/argon2";
import { aB64, borrar, deB64 } from "./cripto/bytes";
import { kCfg, materialCliente } from "./cripto/claves";
import { cifrarPlantilla, clavePlantillas, descifrarPlantilla } from "./cripto/simetrico";
import { cargarCatalogo } from "./catalogoDestinos.svelte";
import { actual, cargarCliente, puede } from "./estado.svelte";
import { kcfgDelCliente, mandarOrden, pruebaParaEquipo } from "./ordenar";
import { equiposQueGuardan, pendiente, trozos, type DatosComunesCliente, type Diferencia, type EntradaOrden } from "./datosComunes";
import type * as T from "./tipos";

export const comunes = $state({
  cliente: "" as string,
  datos: null as DatosComunesCliente | null,
  /** Mandando a los equipos (para el aviso). */
  enviando: false,
  /** El último fallo al mandar solo (se reintenta más tarde). */
  error: "" as string,
});

/** Lo carga (sin avisar si falla: un servidor anterior no lo tiene). */
export async function cargarDatosComunes(c: string): Promise<DatosComunesCliente | null> {
  try {
    const d = await api.datosComunes(c);
    if (actual.id === c) {
      comunes.cliente = c;
      comunes.datos = d;
    }
    return d;
  } catch {
    if (comunes.cliente === c || actual.id === c) {
      comunes.cliente = c;
      comunes.datos = null;
    }
    return null;
  }
}

/** Después de un cambio que llega o se hace aquí: lo que se pinta (etiquetas, catálogo). */
async function refrescar(c: string) {
  await Promise.all([cargarDatosComunes(c), cargarCliente(c, { silencioso: true }), cargarCatalogo(c, true)]);
}

/**
 * Manda `entradas` a todos los equipos que las guardan (en trozos que caben en una orden).
 * Con `pruebas` (por equipo, ya comprobadas con la clave), como `datos_cliente_admin`.
 * Las marca enviadas si llegó al menos una orden a algún equipo.
 */
async function mandarATodos(cliente: T.Cliente, equipos: T.Equipo[], entradas: EntradaOrden[], pruebas?: Map<string, Uint8Array>): Promise<{ equipos: number; fallos: string[] }> {
  const destino = equiposQueGuardan(equipos);
  const fallos: string[] = [];
  let ok = 0;
  if (!entradas.length || !destino.length) return { equipos: 0, fallos };
  for (const e of destino) {
    const prueba = pruebas?.get(e.id);
    if (pruebas && !prueba) continue;
    let bien = true;
    for (const t of trozos(entradas)) {
      try {
        await mandarOrden({ cliente, equipo: e, tipo: pruebas ? "datos_cliente_admin" : "datos_cliente", cuerpo: { entradas: t }, secretos: prueba ? { prueba } : undefined });
      } catch (err) {
        bien = false;
        fallos.push(`${e.nombre}: ${(err as Error).message}`);
        break;
      }
    }
    if (bien) ok++;
  }
  if (ok) await api.datosComunesEnviados(cliente.id, entradas.map((x) => ({ clave: x.clave, cambiado: x.cambiado })));
  return { equipos: ok, fallos };
}

/** Las pruebas de administración de cada equipo que guarda los datos (comprueba la clave con cada uno). */
async function pruebasDe(cliente: T.Cliente, equipos: T.Equipo[], clave: string, alPaso?: (t: string) => void): Promise<Map<string, Uint8Array>> {
  const destino = equiposQueGuardan(equipos);
  if (!destino.length) throw new Error("Ningún equipo de este cliente guarda aún los datos comunes: actualiza sus agentes.");
  const kcfg = await kcfgDelCliente(cliente, clave);
  const out = new Map<string, Uint8Array>();
  try {
    for (const [i, e] of destino.entries()) {
      alPaso?.(`Comprobando la clave con ${e.nombre} (${i + 1} de ${destino.length})…`);
      out.set(e.id, await pruebaParaEquipo(cliente, e, clave, kcfg));
    }
  } catch (err) {
    for (const p of out.values()) borrar(p);
    throw err;
  } finally {
    borrar(kcfg);
  }
  return out;
}

// ---------------------------------------------------------------------------
// Solo (sin clave)
// ---------------------------------------------------------------------------

const enMarcha = new Map<string, Promise<void>>();
/** Tras un fallo al mandar, no se vuelve a intentar solo hasta pasado este tiempo. */
const ESPERA_TRAS_FALLO = 10 * 60_000;
const ultimoFallo = new Map<string, number>();

/**
 * Lo que se hace solo: carga, comparte como semilla lo de antes de compartir que no
 * pide clave y manda lo que esté por enviar sin clave. Nunca a la vez para el mismo cliente.
 */
export function sincronizar(cliente: T.Cliente | null = actual.cliente, equipos: T.Equipo[] = actual.equipos): Promise<void> {
  if (!cliente) return Promise.resolve();
  const ya = enMarcha.get(cliente.id);
  if (ya) return ya;
  const p = hacerSincronizar(cliente, equipos).finally(() => enMarcha.delete(cliente.id));
  enMarcha.set(cliente.id, p);
  return p;
}

async function hacerSincronizar(cliente: T.Cliente, equipos: T.Equipo[]) {
  if (!puede.administrar(cliente.rol)) return;
  let d = await cargarDatosComunes(cliente.id);
  if (!d || !equiposQueGuardan(equipos).length) return;
  if (Date.now() - (ultimoFallo.get(cliente.id) ?? 0) < ESPERA_TRAS_FALLO) return;
  try {
    let p = pendiente(d);
    if (p.sinCompartir.length) {
      await api.ponerDatosComunes(
        cliente.id,
        p.sinCompartir.slice(0, 100).map((x) => ({ clave: x.clave, valor: x.valor, semilla: true })),
      );
      d = (await cargarDatosComunes(cliente.id)) ?? d;
      p = pendiente(d);
    }
    if (p.porEnviar.length) {
      comunes.enviando = true;
      const r = await mandarATodos(cliente, equipos, p.porEnviar);
      if (!r.equipos && r.fallos.length) throw new Error(r.fallos[0]);
      await cargarDatosComunes(cliente.id);
    }
    comunes.error = "";
  } catch (err) {
    ultimoFallo.set(cliente.id, Date.now());
    comunes.error = (err as Error).message;
  } finally {
    comunes.enviando = false;
  }
}

// ---------------------------------------------------------------------------
// Con una persona
// ---------------------------------------------------------------------------

/**
 * Elige el valor de un lado de una diferencia para todas las consolas: lo pone aquí
 * y lo manda a los equipos (con la clave si es el tipo o las marcas de un destino).
 */
export async function elegir(cliente: T.Cliente, equipos: T.Equipo[], d: Diferencia, lado: "aqui" | "otra", clave = "", alPaso?: (t: string) => void): Promise<string> {
  const pruebas = d.conClave ? await pruebasDe(cliente, equipos, clave, alPaso) : undefined;
  try {
    alPaso?.("Guardando…");
    const { entradas } = await api.ponerDatosComunes(cliente.id, [{ clave: d.clave, valor: lado === "aqui" ? d.valorAqui : d.valorOtra }]);
    alPaso?.("Mandándolo a los equipos…");
    const r = await mandarATodos(cliente, equipos, entradas, pruebas);
    await refrescar(cliente.id);
    if (!r.equipos && r.fallos.length) throw new Error(`Guardado aquí, pero no se pudo mandar a los equipos: ${r.fallos[0]}. Se volverá a intentar.`);
    return lado === "aqui" ? "Elegido el de esta consola: lo verán así todas." : "Elegido el de la otra consola: ya está aquí y lo verán así todas.";
  } finally {
    for (const p of pruebas?.values() ?? []) borrar(p);
  }
}

/** Reparte con la clave lo que la pide (tipo y marcas de destinos): lo cambiado aquí y lo de antes de compartir. */
export async function repartirConClave(cliente: T.Cliente, equipos: T.Equipo[], clave: string, alPaso?: (t: string) => void): Promise<string> {
  const pruebas = await pruebasDe(cliente, equipos, clave, alPaso);
  try {
    let p = pendiente(await cargarDatosComunes(cliente.id));
    if (p.sinCompartirConClave.length) {
      await api.ponerDatosComunes(
        cliente.id,
        p.sinCompartirConClave.slice(0, 100).map((x) => ({ clave: x.clave, valor: x.valor, semilla: true })),
      );
      p = pendiente(await cargarDatosComunes(cliente.id));
    }
    if (!p.porEnviarConClave.length) return "No había nada por repartir.";
    alPaso?.("Mandándolo a los equipos…");
    const r = await mandarATodos(cliente, equipos, p.porEnviarConClave, pruebas);
    await refrescar(cliente.id);
    if (!r.equipos && r.fallos.length) throw new Error(r.fallos[0]);
    return `Repartido a ${r.equipos === 1 ? "1 equipo" : `${r.equipos} equipos`}: lo verán así todas las consolas.`;
  } finally {
    for (const p of pruebas.values()) borrar(p);
  }
}

/**
 * Trae las plantillas de otras consolas: las abre aquí, en el navegador, con la clave de
 * administración y la sal de la consola que las cifró, y las vuelve a cifrar con la de aquí.
 * Nunca se abren en el servidor ni en los equipos.
 */
export async function traerPlantillas(cliente: T.Cliente, clave: string, alPaso?: (t: string) => void): Promise<string> {
  const p = pendiente(await cargarDatosComunes(cliente.id));
  if (!p.porTraer.length) return "No hay plantillas por traer.";
  const kAqui = await kcfgDelCliente(cliente, clave);
  const kPlaAqui = clavePlantillas(kAqui);
  const deOtra = new Map<string, Uint8Array>();
  let traidas = 0;
  let ilegibles = 0;
  try {
    for (const f of p.porTraer) {
      const v = f.valor as { cifrado?: string; sal?: string; cliente?: string } | null;
      const id = f.clave.slice("plantilla:".length);
      if (!v?.cifrado || !v.sal || !v.cliente) continue;
      try {
        let k = deOtra.get(v.sal);
        if (!k) {
          alPaso?.("Preparando la clave de la otra consola…");
          const material = await materialCliente(argon2Navegador, clave, v.sal);
          k = clavePlantillas(kCfg(material));
          borrar(material);
          deOtra.set(v.sal, k);
        }
        const pl = descifrarPlantilla<{ v: number; id: string }>(k, v.cliente, id, deB64(v.cifrado));
        // Lo de dentro tiene que decir lo mismo que lo de fuera.
        if (pl.v !== 1 || pl.id !== id) throw new Error("no cuadra");
        await api.traerPlantillaComun(cliente.id, f.clave, f.cambiado, aB64(cifrarPlantilla(kPlaAqui, cliente.id, id, pl)));
        traidas++;
      } catch {
        ilegibles++;
      }
    }
  } finally {
    borrar(kAqui, kPlaAqui, ...deOtra.values());
  }
  await cargarDatosComunes(cliente.id);
  if (!traidas && ilegibles) throw new Error("La clave de administración no abre las plantillas de la otra consola. Comprueba la clave (es la misma en todas las consolas del cliente).");
  return `${traidas === 1 ? "1 plantilla traída" : `${traidas} plantillas traídas`}${ilegibles ? ` · ${ilegibles} no se pudieron abrir` : ""}.`;
}
