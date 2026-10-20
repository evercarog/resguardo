// 0.8.1 (H5): lo que necesita el informe de la configuración de copias del navegador:
// la configuración descifrada de cada equipo (con la clave de administración) y bajar
// el PDF o el CSV. Nada de esto sale del navegador: el servidor solo da la
// configuración cifrada, como siempre.
import * as api from "./api";
import { ApiError } from "./api";
import { borrar, deB64 } from "./cripto/bytes";
import { etiquetaValida } from "./cripto/claves";
import { descifrarConfig } from "./cripto/simetrico";
import { comprobarLlaves } from "./fijadas";
import { kcfgDelCliente } from "./ordenar";
import { guardar } from "./descarga";
import type { Cliente, Configuracion, Equipo } from "./tipos";

/**
 * La configuración de cada equipo, abierta con la clave de administración. Los que no se
 * pueden abrir (sin configuración, llaves cambiadas, otra clave) quedan sin ella y se dice
 * por qué en `faltan`. Si la clave no abre ninguno, falla: es otra clave.
 */
export async function configsDescifradas(
  cliente: Cliente,
  equipos: Equipo[],
  clave: string,
  alPaso: (t: string) => void = () => {},
): Promise<{ configs: Record<string, Configuracion | null>; faltan: string[] }> {
  const configs: Record<string, Configuracion | null> = {};
  const faltan: string[] = [];
  alPaso("Preparando la clave…");
  const kcfg = await kcfgDelCliente(cliente, clave);
  try {
    const conEtiqueta = equipos.filter((e) => e.confirmado && e.modo === "gestionado" && etiquetaValida(kcfg, e));
    if (equipos.some((e) => e.confirmado && e.modo === "gestionado") && !conEtiqueta.length) throw new Error("La clave de administración no es correcta.");
    for (const [i, e] of equipos.entries()) {
      alPaso(`Abriendo la configuración de ${e.nombre} (${i + 1} de ${equipos.length})…`);
      if (!conEtiqueta.includes(e)) {
        configs[e.id] = null;
        if (e.confirmado && e.modo === "gestionado") faltan.push(`${e.nombre}: la clave no abre su configuración.`);
        continue;
      }
      if ((await comprobarLlaves(cliente.id, e)) === "cambiada") {
        configs[e.id] = null;
        faltan.push(`${e.nombre}: sus llaves no son las que se confirmaron al emparejarlo.`);
        continue;
      }
      try {
        const c = await api.configEquipo(cliente.id, e.id);
        configs[e.id] = descifrarConfig<Configuracion>(kcfg, e.id, c.seq, deB64(c.cifrado));
      } catch (err) {
        configs[e.id] = null;
        if (!(err instanceof ApiError && err.codigo === "no_existe")) faltan.push(`${e.nombre}: no se pudo abrir su configuración.`);
      }
    }
  } finally {
    borrar(kcfg);
  }
  return { configs, faltan };
}

/** Baja un archivo hecho en el navegador. */
export const bajar = (datos: BlobPart, tipo: string, nombre: string) => guardar(new Blob([datos], { type: tipo }), nombre);
