// Paquete de exportación de un cliente (.resguardo-cliente, api-servidor.md §11).
//
// El navegador junta lo que el servidor sabe del cliente (equipos con sus
// llaves públicas y etiquetas, configuraciones cifradas, informes, avisos y
// la auditoría entera con su cadena de hashes; v1.40: también las
// observaciones y los comentarios), lo cifra con
// K_exp = HKDF(Argon2id(clave_admin, sal_cliente), "resguardo-kexp-v1") y
// solo entonces lo descarga o lo sube. En el servidor nuevo (que recibió el
// cliente con la misma sal) se descifra aquí y se importa el historial.
import * as api from "./api";
import { argon2Navegador } from "./cripto/argon2";
import { borrar, deUtf8, utf8 } from "./cripto/bytes";
import { kExp, materialCliente } from "./cripto/claves";
import { cabeceraPaquete, cifrarPaquete, descifrarPaquete } from "./cripto/paquete";
import type { Aviso, Cliente, EntradaAuditoria, Equipo, Informe, NotasExportadas } from "./tipos";

export interface ContenidoPaquete {
  formato: "resguardo-cliente";
  v: 1;
  exportado: string;
  origen: string;
  cliente: { id: string; nombre: string; sal_cliente: string; espera_min_horas: number };
  equipos: Pick<Equipo, "id" | "nombre" | "so" | "rol" | "box_pub" | "sign_pub" | "etiqueta" | "sal_equipo">[];
  configuraciones: { equipo: string; seq: number; cifrado: string }[];
  informes: { equipo: string; recibido: string; datos: Informe["datos"] }[];
  avisos: { equipo: string | null; tipo: string; mensaje: string; creado: string }[];
  auditoria: EntradaAuditoria[];
  /** v1.40: observaciones y comentarios (un paquete anterior no los trae). */
  notas?: NotasExportadas;
}

/** Junta el contenido (en claro, solo en memoria). */
export async function juntar(cliente: Cliente, alPaso: (t: string) => void = () => {}): Promise<ContenidoPaquete> {
  alPaso("Leyendo los equipos…");
  const equipos = await api.equipos(cliente.id);
  const configuraciones: ContenidoPaquete["configuraciones"] = [];
  const informes: ContenidoPaquete["informes"] = [];
  for (const [i, e] of equipos.entries()) {
    alPaso(`Leyendo ${e.nombre} (${i + 1} de ${equipos.length})…`);
    try {
      const c = await api.configEquipo(cliente.id, e.id);
      configuraciones.push({ equipo: e.id, seq: c.seq, cifrado: c.cifrado });
    } catch (err) {
      if (!(err instanceof api.ApiError && err.codigo === "no_existe")) throw err;
    }
    for (const inf of await api.informes(cliente.id, e.id, 500)) informes.push({ equipo: e.id, recibido: inf.recibido, datos: inf.datos });
  }
  alPaso("Leyendo los avisos…");
  const avisos = (await api.avisos(cliente.id, false)).map((a: Aviso) => ({ equipo: a.equipo, tipo: a.tipo, mensaje: a.mensaje, creado: a.creado }));
  alPaso("Leyendo la actividad…");
  const auditoria: EntradaAuditoria[] = [];
  for (let desde = 0; ; ) {
    const pag = await api.auditoria(cliente.id, desde, 500);
    auditoria.push(...pag);
    if (pag.length < 500) break;
    desde = pag[pag.length - 1].n;
  }
  alPaso("Leyendo las observaciones y los comentarios…");
  // Un servidor anterior no las tiene (404): el paquete va sin ellas.
  const notas = await api.notasTodas(cliente.id).catch((err) => {
    if (err instanceof api.ApiError && err.codigo === "no_existe") return undefined;
    throw err;
  });
  return {
    formato: "resguardo-cliente",
    v: 1,
    exportado: new Date().toISOString(),
    origen: location.origin,
    cliente: { id: cliente.id, nombre: cliente.nombre, sal_cliente: cliente.sal_cliente, espera_min_horas: cliente.espera_min_horas },
    equipos: equipos.map((e) => ({ id: e.id, nombre: e.nombre, so: e.so, rol: e.rol, box_pub: e.box_pub, sign_pub: e.sign_pub, etiqueta: e.etiqueta, sal_equipo: e.sal_equipo })),
    configuraciones,
    informes,
    avisos,
    auditoria,
    ...(notas && (notas.observaciones.length || notas.comentarios.length) ? { notas } : {}),
  };
}

async function claveExp(claveAdmin: string, salB64: string) {
  const material = await materialCliente(argon2Navegador, claveAdmin, salB64);
  const k = kExp(material);
  borrar(material);
  return k;
}

/** Cifra el contenido con K_exp: lo único que sale del navegador. */
export async function cifrar(claveAdmin: string, cliente: Cliente, contenido: ContenidoPaquete): Promise<Uint8Array> {
  const k = await claveExp(claveAdmin, cliente.sal_cliente);
  try {
    return cifrarPaquete(k, cliente.sal_cliente, utf8(JSON.stringify(contenido)));
  } finally {
    borrar(k);
  }
}

/** Abre un paquete de este cliente. Falla si es de otro cliente o si la clave no es la suya. */
export async function abrir(claveAdmin: string, cliente: Cliente, paquete: Uint8Array): Promise<ContenidoPaquete> {
  let sal: string;
  try {
    sal = cabeceraPaquete(paquete).sal;
  } catch {
    throw new Error("Ese archivo no es un paquete de Resguardo (.resguardo-cliente).");
  }
  if (sal !== cliente.sal_cliente) throw new Error("Ese paquete es de otro cliente: la sal no coincide con la de este.");
  const k = await claveExp(claveAdmin, sal);
  let json: Uint8Array;
  try {
    json = descifrarPaquete(k, paquete).json;
  } catch {
    throw new Error("No se pudo abrir: la clave de administración no es la de este cliente, o el archivo está dañado.");
  } finally {
    borrar(k);
  }
  const c = JSON.parse(deUtf8(json)) as ContenidoPaquete;
  if (c.formato !== "resguardo-cliente") throw new Error("El contenido del paquete no es el esperado.");
  return c;
}

/** Sube al servidor nuevo el historial (la auditoría, solo si su cadena está entera; una sola vez). */
export const importar = (c: string, x: ContenidoPaquete) =>
  api.importarHistorial(c, { origen: x.origen, auditoria: x.auditoria, informes: x.informes, avisos: x.avisos, ...(x.notas ? { notas: x.notas } : {}) });

export const nombreArchivo = (cliente: Cliente) =>
  `${cliente.nombre.normalize("NFD").replace(/[̀-ͯ]/g, "").replace(/[^A-Za-z0-9]+/g, "-").replace(/^-|-$/g, "").toLowerCase() || "cliente"}-${new Date().toISOString().slice(0, 10)}.resguardo-cliente`;
