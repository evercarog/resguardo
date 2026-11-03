// 0.8.3 (16d): la página de cada disco físico y de cada conjunto (o disco virtual) anotado
// en Infraestructura: su ficha, lo que vive en él y qué se perdería si falla. Sin runas:
// se prueba en scripts/vectores-083-servidor.ts.
import type { Equipo } from "./tipos";
import {
  discoDe,
  conjuntoDe,
  enFiltroInfra,
  servidorDe,
  sitioDeDestino,
  sitioDeEquipo,
  sitioDeVolumen,
  type ConjuntoDiscos,
  type DiscoFisico,
  type FiltroInfra,
  type Infraestructura,
  type ServidorFisico,
} from "./infra";

/** Los conjuntos que aguantan que falle un disco (con redundancia). */
export const CONJUNTOS_REDUNDANTES = ["raid1", "raid5", "raid6", "raid10", "zfs_espejo", "zfs_raidz1", "zfs_raidz2", "zfs_raidz3"];
/** Cuántos discos pueden fallar a la vez sin perder datos (lo mínimo seguro). */
const AGUANTA: Record<string, number> = { raid1: 1, raid5: 1, raid6: 2, raid10: 1, zfs_espejo: 1, zfs_raidz1: 1, zfs_raidz2: 2, zfs_raidz3: 3 };

export interface EnSoporte {
  equipo: Equipo;
  /** El equipo entero está ahí (si no, solo algunos volúmenes). */
  entero: boolean;
  /** Sus volúmenes que están ahí (C:, /srv…). */
  volumenes: string[];
}

export interface DestinoEnSoporte {
  clave: string;
  nombre: string;
  equipo: string | null;
}

export interface FichaSoporte {
  servidor: ServidorFisico;
  tipo: "disco" | "conjunto";
  disco: DiscoFisico | null;
  conjunto: ConjuntoDiscos | null;
  /** De un disco: los conjuntos de los que forma parte. */
  conjuntosDelDisco: ConjuntoDiscos[];
  /** De un conjunto: sus discos (los anotados uno a uno). */
  discosDelConjunto: DiscoFisico[];
  equipos: EnSoporte[];
  destinos: DestinoEnSoporte[];
  /** Qué pasa si falla, en frases. */
  siFalla: string[];
}

export function fichaSoporte(
  i: Infraestructura | null | undefined,
  servidor: string,
  tipo: "disco" | "conjunto",
  id: string,
  equipos: readonly Equipo[],
  destinos: readonly { clave: string; nombre: string; equipo?: string | null }[],
): FichaSoporte | null {
  const srv = servidorDe(i, servidor);
  if (!srv) return null;
  const disco = tipo === "disco" ? discoDe(srv, id) : null;
  const conjunto = tipo === "conjunto" ? conjuntoDe(srv, id) : null;
  if (!disco && !conjunto) return null;
  const f: FiltroInfra = { servidor, ...(disco ? { disco: disco.id } : { conjunto: conjunto!.id }) };
  const conjuntosDelDisco = disco ? srv.conjuntos.filter((c) => c.miembros?.includes(disco.id)) : [];
  const discosDelConjunto = conjunto ? (conjunto.miembros ?? []).map((m) => discoDe(srv, m)).filter((d): d is DiscoFisico => !!d) : [];

  const enSoporte: EnSoporte[] = [];
  for (const e of equipos) {
    const entero = enFiltroInfra(sitioDeEquipo(i, e.id), f);
    const volumenes = (i?.equipos[e.id]?.volumenes ?? []).filter((v) => enFiltroInfra(sitioDeVolumen(i, e.id, v.punto), f)).map((v) => v.punto);
    if (entero || volumenes.length) enSoporte.push({ equipo: e, entero, volumenes });
  }
  const enDestinos = destinos.filter((d) => enFiltroInfra(sitioDeDestino(i, d.clave, d.equipo ?? null), f)).map((d) => ({ clave: d.clave, nombre: d.nombre, equipo: d.equipo ?? null }));

  return {
    servidor: srv,
    tipo,
    disco,
    conjunto,
    conjuntosDelDisco,
    discosDelConjunto,
    equipos: enSoporte,
    destinos: enDestinos,
    siFalla: siFalla({ disco, conjunto, conjuntosDelDisco, equipos: enSoporte, destinos: enDestinos }),
  };
}

/** Qué se perdería si falla, en frases (solo con lo anotado: Resguardo no ve los discos). */
export function siFalla(x: Pick<FichaSoporte, "disco" | "conjunto" | "conjuntosDelDisco" | "equipos" | "destinos">): string[] {
  const out: string[] = [];
  const nombres = (xs: string[]) => (xs.length <= 3 ? xs.join(", ") : `${xs.slice(0, 3).join(", ")} y ${xs.length - 3} más`);
  // Un disco dentro de un conjunto con redundancia: el conjunto aguanta.
  const redundante = x.disco ? x.conjuntosDelDisco.find((c) => CONJUNTOS_REDUNDANTES.includes(c.tipo)) : x.conjunto && CONJUNTOS_REDUNDANTES.includes(x.conjunto.tipo) ? x.conjunto : null;
  if (x.disco && redundante) {
    out.push(`Si falla solo este disco, «${redundante.nombre}» sigue funcionando (aguanta ${AGUANTA[redundante.tipo] === 1 ? "un disco" : `${AGUANTA[redundante.tipo]} discos`}). Cámbialo pronto: mientras tanto no aguanta otro fallo.`);
    return out;
  }
  if (x.conjunto && redundante) {
    const n = AGUANTA[x.conjunto.tipo] ?? 1;
    out.push(`Aguanta que fallen ${n === 1 ? "un disco" : `${n} discos`} a la vez. Si fallan más (o el servidor entero), se perdería lo de abajo.`);
  } else if (x.conjunto) {
    out.push(x.conjunto.tipo === "raid0" ? "Es un RAID 0: si falla cualquiera de sus discos, se pierde todo lo que guarda." : "Sin redundancia anotada: si falla, se pierde lo que guarda.");
  } else if (x.disco) {
    out.push(x.conjuntosDelDisco.length ? "Forma parte de un conjunto sin redundancia: si falla, se pierde lo que guarda ese conjunto." : "Es un disco suelto: si falla, se pierde lo que guarda.");
  }
  const originales = x.equipos.map((e) => (e.entero ? e.equipo.nombre : `${e.equipo.nombre} (${e.volumenes.join(", ")})`));
  if (originales.length) out.push(`Los datos originales de ${nombres(originales)}: necesitan una copia fuera de este ${x.conjunto ? "conjunto" : "disco"}.`);
  if (x.destinos.length) out.push(`Las copias guardadas en ${nombres(x.destinos.map((d) => d.nombre))}: cuentan como un solo soporte; que haya otra copia en otro disco o fuera.`);
  if (!originales.length && !x.destinos.length) out.push("No hay nada anotado en él.");
  return out;
}

/** La dirección de la página de un disco o de un conjunto. */
export const hrefSoporte = (c: string, servidor: string, tipo: "disco" | "conjunto", id: string) =>
  `/c/${encodeURIComponent(c)}/ajustes/infraestructura/${encodeURIComponent(servidor)}/${tipo}/${encodeURIComponent(id)}`;
