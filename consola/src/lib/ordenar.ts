// Mandar una orden a un equipo, con sus secretos, de principio a fin:
//
// 1. Si pide la clave de administración: Argon2id(clave, sal_cliente) → K_cfg,
//    y se comprueba la etiqueta del equipo (que sus llaves son las que se
//    confirmaron al emparejar). Si no cuadra, NO se sella nada.
// 2. prueba_e = Argon2id(clave, sal_equipo) para ese equipo.
// 3. Se sella la orden v2 para su X25519 y se envía con `siguiente_seq`; si
//    otra orden se adelantó (409), se vuelve a sellar con el número nuevo.
//
// Llaves fijadas (fijadas.ts): antes de sellar una contraseña o una prueba se
// comprueba que las llaves del equipo son las que este navegador ya comprobó
// con la etiqueta. Si cambiaron: alto, alerta y no se envía nada. Si aún no
// están fijadas, una orden con la contraseña del repositorio pide también la
// clave de administración para comprobar la etiqueta y fijarlas.
//
// Las claves y contraseñas solo viven en variables de esta función (y en el
// campo del diálogo mientras se escribe). Nunca se guardan ni se envían en claro.
import * as api from "./api";
import { ApiError } from "./api";
import { argon2Navegador } from "./cripto/argon2";
import { aB64, borrar } from "./cripto/bytes";
import { etiquetaValida, kCfg, materialCliente, pruebaAdmin, pruebaCodigo, verificador } from "./cripto/claves";
import { comprobarLlaves, ErrorLlavesCambiadas, fijar } from "./fijadas";
import { NIVEL, PIDE_TAMBIEN_ADMIN, sellarOrden, type Autorizacion } from "./cripto/ordenes";
import { seguirOrden } from "./pendientes.svelte";
import { app } from "./estado.svelte";
import type * as T from "./tipos";

export interface Secretos {
  claveAdmin?: string;
  /**
   * Prueba de administración ya calculada para este equipo (y su etiqueta ya
   * comprobada), p. ej. en el editor de copias, que la pide al abrirse.
   */
  prueba?: Uint8Array;
  repo?: { repo: string; contrasena: string };
}

export const necesitaAdmin = (tipo: string) => NIVEL[tipo] === "admin" || PIDE_TAMBIEN_ADMIN.has(tipo);
export const necesitaRepo = (tipo: string) => NIVEL[tipo] === "repo";
/** v1.49: ¿guarda el equipo las órdenes con espera (las recibe al momento)? */
const admiteEspera = (e: T.Equipo) => !!e.resumen?.admite?.includes("ordenes_en_espera");

export class ErrorEtiqueta extends Error {}
export class ErrorFaltaAdmin extends Error {
  constructor() {
    super("Es la primera vez que este navegador manda una contraseña a este equipo: confirma también con la clave de administración.");
  }
}
export { ErrorLlavesCambiadas };

/**
 * Deriva K_cfg con la clave de administración y comprueba la etiqueta del
 * equipo; si cuadra, fija sus llaves en este navegador. Devuelve K_cfg (bórrala
 * al terminar). Si las llaves fijadas cambiaron, se para aquí (salvo `refijar`,
 * que usa «Volver a comprobar las llaves» con la clave de administración).
 */
export async function kcfgComprobada(cliente: T.Cliente, equipo: T.Equipo, claveAdmin: string, refijar = false): Promise<Uint8Array> {
  if (!refijar && (await comprobarLlaves(cliente.id, equipo)) === "cambiada") throw new ErrorLlavesCambiadas(equipo.nombre);
  const material = await materialCliente(argon2Navegador, claveAdmin, cliente.sal_cliente);
  const kcfg = kCfg(material);
  borrar(material);
  if (!etiquetaValida(kcfg, equipo)) {
    borrar(kcfg);
    throw new ErrorEtiqueta(
      "La clave de administración no es correcta, o las llaves de este equipo no son las que se confirmaron al emparejarlo. No se ha enviado nada.",
    );
  }
  await fijar(cliente.id, equipo);
  return kcfg;
}

/**
 * Para varios equipos a la vez con la clave de administración (v1.52, «Varios a
 * la vez»): K_cfg se calcula una sola vez para el cliente. Quien la llama la
 * borra al terminar.
 */
export async function kcfgDelCliente(cliente: T.Cliente, claveAdmin: string): Promise<Uint8Array> {
  const material = await materialCliente(argon2Navegador, claveAdmin, cliente.sal_cliente);
  try {
    return kCfg(material);
  } finally {
    borrar(material);
  }
}

/**
 * La prueba de administración para un equipo, con la K_cfg del cliente ya
 * calculada: comprueba sus llaves (fijadas y etiqueta) como `kcfgComprobada` y,
 * si no cuadran, no prepara nada. Quien la llama la borra al terminar.
 */
export async function pruebaParaEquipo(cliente: T.Cliente, equipo: T.Equipo, claveAdmin: string, kcfg: Uint8Array): Promise<Uint8Array> {
  if ((await comprobarLlaves(cliente.id, equipo)) === "cambiada") throw new ErrorLlavesCambiadas(equipo.nombre);
  if (!etiquetaValida(kcfg, equipo)) throw new ErrorEtiqueta("La clave de administración no es correcta, o las llaves de este equipo no son las que se confirmaron al emparejarlo. No se le ha enviado nada.");
  await fijar(cliente.id, equipo);
  return pruebaAdmin(argon2Navegador, claveAdmin, equipo.sal_equipo);
}

/** ¿Hará falta también la clave de administración para una orden con la contraseña del repositorio? */
export async function repoPideAdmin(cliente: string, equipo: T.Equipo) {
  return (await comprobarLlaves(cliente, equipo)) !== "fijada";
}

export async function mandarOrden(opts: {
  cliente: T.Cliente;
  equipo: T.Equipo;
  tipo: string;
  cuerpo?: Record<string, unknown>;
  secretos?: Secretos;
  sesion?: string | null;
  relevo?: { id: string; max_bytes: number } | null;
  responderA?: string | null;
  /** Para «alta»: el cuerpo se calcula aquí con la clave (verificador y K_cfg) y el código de emparejamiento. */
  alta?: { codigo: string };
  alPaso?: (texto: string) => void;
}): Promise<T.Orden> {
  const { cliente, tipo } = opts;
  let equipo = opts.equipo;
  const cuerpo = { ...(opts.cuerpo ?? {}) };
  const autorizacion: Autorizacion = { prueba_admin: null, clave_repo: null };
  const aBorrar: Uint8Array[] = [];
  try {
    if (opts.alta) {
      // Alta: el equipo aún no tiene verificador. Se le da el suyo y K_cfg.
      const clave = opts.secretos?.claveAdmin ?? "";
      if (!clave) throw new Error("Escribe la clave de administración del cliente.");
      opts.alPaso?.("Preparando las claves del equipo…");
      const material = await materialCliente(argon2Navegador, clave, cliente.sal_cliente);
      const kcfg = kCfg(material);
      const prueba = await pruebaAdmin(argon2Navegador, clave, equipo.sal_equipo);
      aBorrar.push(material, kcfg, prueba);
      const ver = aB64(verificador(prueba));
      cuerpo.verificador = ver;
      cuerpo.k_cfg = aB64(kcfg);
      cuerpo.espera_min_horas = cliente.espera_min_horas;
      // El equipo comprueba que la prueba corresponde al verificador y que el
      // alta viene de quien vio el código (el servidor solo tiene su hash).
      autorizacion.prueba_admin = aB64(prueba);
      autorizacion.prueba_codigo = pruebaCodigo(opts.alta.codigo, equipo.id, ver);
      if (!etiquetaValida(kcfg, equipo)) throw new ErrorEtiqueta("La etiqueta del equipo no cuadra con esta clave. No se ha enviado nada.");
      await fijar(cliente.id, equipo);
    } else if (necesitaAdmin(tipo) && opts.secretos?.prueba) {
      // Prueba ya calculada (y llaves ya comprobadas) al abrir el editor: solo se mira que no hayan cambiado.
      if ((await comprobarLlaves(cliente.id, equipo)) === "cambiada") throw new ErrorLlavesCambiadas(equipo.nombre);
      autorizacion.prueba_admin = aB64(opts.secretos.prueba);
    } else if (necesitaAdmin(tipo)) {
      const clave = opts.secretos?.claveAdmin ?? "";
      if (!clave) throw new Error("Escribe la clave de administración del cliente.");
      opts.alPaso?.("Comprobando la clave y las llaves del equipo…");
      aBorrar.push(await kcfgComprobada(cliente, equipo, clave));
      opts.alPaso?.("Preparando la autorización para el equipo…");
      const prueba = await pruebaAdmin(argon2Navegador, clave, equipo.sal_equipo);
      aBorrar.push(prueba);
      autorizacion.prueba_admin = aB64(prueba);
    }
    if (necesitaRepo(tipo)) {
      const r = opts.secretos?.repo;
      if (!r?.contrasena) throw new Error("Escribe la contraseña del repositorio.");
      const llaves = await comprobarLlaves(cliente.id, equipo);
      if (llaves === "cambiada") throw new ErrorLlavesCambiadas(equipo.nombre);
      if (llaves === "sin_fijar" && !necesitaAdmin(tipo)) {
        // Primera vez en este navegador: se comprueban (y fijan) las llaves con la clave de administración.
        const clave = opts.secretos?.claveAdmin ?? "";
        if (!clave) throw new ErrorFaltaAdmin();
        opts.alPaso?.("Comprobando las llaves del equipo con la clave de administración…");
        aBorrar.push(await kcfgComprobada(cliente, equipo, clave));
      }
      autorizacion.clave_repo = { repo: r.repo, contrasena: r.contrasena };
    }

    /** Espera que exige el servidor (si responde 422 con espera_min_horas). */
    let esperaServidor: number | undefined;
    for (let intento = 0; ; intento++) {
      opts.alPaso?.("Sellando la orden para el equipo…");
      const p = sellarOrden({
        cliente: cliente.id,
        equipo,
        seq: equipo.siguiente_seq,
        tipo,
        cuerpo,
        autorizacion,
        responderA: opts.responderA ?? null,
        // La espera que confirmó el equipo manda; si no, la del cliente.
        esperaHoras: esperaServidor ?? equipo.espera_min_horas ?? cliente.espera_min_horas,
        contexto: {
          espejo: equipo.resumen?.guarda_copias?.espejo ?? null,
          espejoEquipo: equipo.resumen?.espejo_equipo ?? null,
          copiasActivas: (equipo.resumen?.copias ?? []).filter((k) => k.activa !== false).length,
        },
        // v1.49: quién la manda (lo ven las demás consolas en sus órdenes en espera y en el historial).
        por: app.cuenta?.nombre ?? null,
        // v1.58: a un agente que no guarda las órdenes con espera, con el número reservado: así
        // las que se manden mientras espera no la dejan «antigua» al llegar su hora.
        seqEspera: admiteEspera(equipo) ? null : (equipo.seq_espera ?? null),
      });
      opts.alPaso?.("Enviando…");
      try {
        const o = await api.enviarOrden(cliente.id, equipo.id, {
          tipo: p.meta.tipo,
          seq: p.meta.seq,
          sellado: p.sellado,
          caduca: p.meta.caduca,
          not_before: p.meta.not_before,
          sesion: opts.sesion ?? null,
          relevo: opts.relevo ?? null,
        });
        // Lo que crea o cambia algo visible se enseña «en camino» donde aparecerá
        // (solo el título y el id: nada del cuerpo, que puede llevar secretos).
        seguirOrden(cliente.id, equipo, o, cuerpo);
        return o;
      } catch (e) {
        // v1.10: una destructiva con un not_before más corto que la espera del
        // servidor vuelve con 422 y su `espera_min_horas`: se vuelve a sellar
        // una vez con esa espera (el seq no se gastó: el servidor no la guardó).
        if (e instanceof ApiError && e.estado === 422 && esperaServidor === undefined && Number.isFinite(Number(e.cuerpo?.espera_min_horas))) {
          esperaServidor = Number(e.cuerpo!.espera_min_horas);
          opts.alPaso?.(`El servidor pide esperar ${esperaServidor} h: se vuelve a preparar…`);
          continue;
        }
        // Otra orden se adelantó: se vuelve a sellar con el número que diga el servidor.
        if (e instanceof ApiError && e.codigo === "conflicto" && intento < 3) {
          const siguiente = Number(e.cuerpo?.siguiente_seq);
          const reservado = Number(e.cuerpo?.seq_espera);
          equipo = Number.isFinite(siguiente)
            ? { ...equipo, siguiente_seq: siguiente, ...(Number.isFinite(reservado) ? { seq_espera: reservado } : {}) }
            : await api.equipo(cliente.id, equipo.id);
          continue;
        }
        throw e;
      }
    }
  } finally {
    borrar(...aBorrar);
    autorizacion.prueba_admin = null;
    autorizacion.clave_repo = null;
  }
}
