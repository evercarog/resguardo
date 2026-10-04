// Venir de la app de escritorio (api-servidor.md §5, v1.14): usar un
// repositorio que ya existe (adoptar_repositorio) y traer el historial de
// otro (copiar_historial). Aquí, lo que no es pantalla: probar un repositorio
// con el equipo (respuesta sellada para este navegador) y esperar resultados.
// La dirección y los cuerpos, en direccion.ts.
import * as api from "./api";
import { argon2Navegador } from "./cripto/argon2";
import { aB64, borrar, deB64, deUtf8 } from "./cripto/bytes";
import { pruebaAdmin } from "./cripto/claves";
import { abrir, parEfimero } from "./cripto/sobre";
import { kcfgComprobada, mandarOrden } from "./ordenar";
import type * as T from "./tipos";
import {
  destinoCuerpo,
  partirDireccion,
  repoExistenteVacio,
  type RepoExistente,
} from "./direccion";

export * from "./direccion";

/** Lo que dice el equipo al probar un repositorio (detalle de `adoptar_repositorio { solo_probar }`). */
export interface Prueba {
  mensaje: string;
  versiones: number;
  ultima: string | null;
  solo_anadir: boolean | null;
  en_uso: string | null;
  equipos: string[];
  etiquetas: string[];
}

const FINALES = ["hecha", "fallida", "rechazada", "cancelada", "caducada"];

/** Espera el resultado final de una orden (como mucho `segundos`). */
export async function esperarRespuesta(
  cliente: string,
  equipo: T.Equipo,
  o: T.Orden,
  segundos = 180,
): Promise<T.Orden> {
  for (let i = 0; i < segundos / 1.5; i++) {
    const x = (await api.ordenesEquipo(cliente, equipo.id, 10)).find(
      (y) => y.id === o.id,
    );
    if (x && FINALES.includes(x.estado)) return x;
    await new Promise((r) => setTimeout(r, 1500));
  }
  throw new Error(
    `${equipo.nombre} no ha respondido todavía. Mira sus órdenes en un momento.`,
  );
}

/**
 * Comprueba la clave de administración (y las llaves del equipo) y calcula su
 * prueba una sola vez: así «Probar» y «Usar» no la vuelven a derivar. Bórrala
 * (`borrar`) al cerrar.
 */
export async function pruebaParaEquipo(
  cliente: T.Cliente,
  equipo: T.Equipo,
  claveAdmin: string,
): Promise<Uint8Array> {
  borrar(await kcfgComprobada(cliente, equipo, claveAdmin));
  return pruebaAdmin(argon2Navegador, claveAdmin, equipo.sal_equipo);
}

/**
 * «Probar»: el equipo abre el repositorio con esos datos y dice cuántas
 * versiones tiene. La respuesta (con los nombres de los equipos que copiaron
 * en él) llega sellada solo para este navegador.
 */
export async function probarRepositorio(opts: {
  cliente: T.Cliente;
  equipo: T.Equipo;
  prueba: Uint8Array;
  repo: RepoExistente;
  /** En un destino que el equipo ya tiene: `{ destino: { id }, ruta }` (entonces `repo` solo aporta la contraseña). */
  lugar?: { destino: { id: string }; ruta: string };
  alPaso?: (t: string) => void;
}): Promise<Prueba> {
  const eph = parEfimero();
  try {
    const o = await mandarOrden({
      cliente: opts.cliente,
      equipo: opts.equipo,
      tipo: "adoptar_repositorio",
      cuerpo: {
        solo_probar: true,
        ...(opts.lugar ?? { destino: destinoCuerpo(opts.repo), ruta: partirDireccion(opts.repo.tipo, opts.repo.direccion).ruta }),
        contrasena: opts.repo.contrasena,
      },
      secretos: { prueba: opts.prueba },
      responderA: aB64(eph.publica),
      alPaso: opts.alPaso,
    });
    opts.alPaso?.(`Esperando a ${opts.equipo.nombre}…`);
    const r = await esperarRespuesta(opts.cliente.id, opts.equipo, o);
    if (r.estado !== "hecha")
      throw new Error(
        r.mensaje ?? `${opts.equipo.nombre} no pudo abrir el repositorio.`,
      );
    let d: Partial<Prueba> = {};
    try {
      const sellado = r.detalle
        ? (JSON.parse(r.detalle) as { sellado?: string }).sellado
        : undefined;
      if (sellado)
        d = JSON.parse(
          deUtf8(abrir(eph.secreta, deB64(sellado))),
        ) as Partial<Prueba>;
    } catch {
      // Sin detalle legible: basta el mensaje.
    }
    return {
      mensaje: r.mensaje ?? "Se abre con esa contraseña.",
      versiones: Number(d.versiones ?? 0),
      ultima: d.ultima ?? null,
      solo_anadir: d.solo_anadir ?? null,
      en_uso: d.en_uso ?? null,
      equipos: d.equipos ?? [],
      etiquetas: d.etiquetas ?? [],
    };
  } finally {
    borrar(eph.secreta);
  }
}

/** Lo que se explica al crear un repositorio «para traer el historial» de otro. */
export const TEXTO_TROCEADO =
  "El repositorio nuevo usa la misma forma de trocear los archivos que el de origen: al traer el historial ocupa lo mismo que el original, y las copias nuevas aprovechan todo lo anterior. El original no se toca.";

const claveOrigen = (cliente: string, equipo: string, repo: string) =>
  `resguardo.origen-historial.${cliente}.${equipo}.${repo}`;

/**
 * Recuerda en este navegador de dónde se va a traer el historial de un
 * repositorio creado «para traer el historial» (tipo, dirección, usuario y
 * autoridad; nunca contraseñas), para que «Traer historial» salga ya puesto.
 */
export function recordarOrigen(
  cliente: string,
  equipo: string,
  repo: string,
  r: RepoExistente,
) {
  try {
    localStorage.setItem(
      claveOrigen(cliente, equipo, repo),
      JSON.stringify({
        tipo: r.tipo,
        direccion: r.direccion.trim(),
        usuario: r.usuario.trim(),
        ca: r.ca.trim(),
      }),
    );
  } catch {
    // Sin almacenamiento: se escribe otra vez al traerlo.
  }
}

/** El origen recordado (sin contraseñas), o null. */
export function origenRecordado(
  cliente: string,
  equipo: string,
  repo: string,
): RepoExistente | null {
  try {
    const x = JSON.parse(
      localStorage.getItem(claveOrigen(cliente, equipo, repo)) ?? "null",
    ) as Partial<RepoExistente> | null;
    if (!x?.direccion || !x.tipo) return null;
    return {
      ...repoExistenteVacio(),
      tipo: x.tipo,
      direccion: x.direccion,
      usuario: x.usuario ?? "",
      ca: x.ca ?? "",
    };
  } catch {
    return null;
  }
}
