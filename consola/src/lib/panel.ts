// Cálculos del panel de Estado: los cuadros de cada equipo (todas sus copias
// juntas), las cifras de arriba y cuánto se guarda en cada destino con su
// tendencia. Todo sale del resumen y del último informe de cada equipo.
import type { DestinoResumen, Equipo, Informe, RepoInforme } from "./tipos";
import { bytesRepo, claveDia, destinoDe, dias, informeDe, type Dia } from "./repo";

const DIA = 86_400_000;

/** Los cuadros de un equipo: las versiones y vueltas de todos sus repositorios juntas. */
export function diasEquipo(informe: Informe | null | undefined, cuantos = 14, ahora = Date.now()): Dia[] {
  const repos = informe?.datos?.repos ?? [];
  const junto: RepoInforme = {
    id: "",
    nombre: "",
    versiones: repos.flatMap((r) => r.versiones ?? []),
    versiones_leidas: null,
    ejecuciones: repos.flatMap((r) => r.ejecuciones ?? []),
    espacio: null,
    verificacion: null,
    prueba_restauracion: null,
    externa: null,
    proteccion: null,
  };
  return dias(junto, cuantos, ahora);
}

/** Versiones guardadas en las últimas 24 h y vueltas fallidas en ese tiempo, de todos los equipos. */
export function ultimas24h(equipos: Equipo[], informes: Record<string, Informe | null>, ahora = Date.now()) {
  let versiones = 0;
  let fallos = 0;
  for (const e of equipos) {
    let suyos = 0;
    for (const r of informes[e.id]?.datos?.repos ?? []) {
      versiones += (r.versiones ?? []).filter((v) => Date.parse(v.hora) > ahora - DIA).length;
      suyos += (r.ejecuciones ?? []).filter((x) => x.resultado === "fallo" && Date.parse(x.hora) > ahora - DIA).length;
    }
    // El resumen puede saber de un fallo antes que el informe: nunca decir «sin vueltas fallidas» si una copia acaba de fallar.
    const enResumen = (e.resumen?.copias ?? []).filter((k) => k.ultima?.estado === "fallo" && Date.parse(k.ultima.cuando) > ahora - DIA).length;
    fallos += Math.max(suyos, enResumen);
  }
  return { versiones, fallos };
}

/** La próxima copia programada de todos los equipos (activa y no en el pasado). */
export function proximaDeTodos(equipos: Equipo[], ahora = Date.now()): { cuando: string; equipo: Equipo; copia: string } | null {
  let mejor: { cuando: string; equipo: Equipo; copia: string } | null = null;
  for (const e of equipos)
    for (const k of e.resumen?.copias ?? []) {
      if (k.activa === false || !k.proxima || Date.parse(k.proxima) < ahora) continue;
      if (!mejor || k.proxima < mejor.cuando) mejor = { cuando: k.proxima, equipo: e, copia: k.nombre };
    }
  return mejor;
}

export type TipoAlmacen = DestinoResumen["tipo"] | "almacen";

export interface Almacen {
  clave: string;
  nombre: string;
  tipo: TipoAlmacen;
  /** Dónde (servidor o bucket), sin credenciales. */
  donde?: string;
  /** Si es un equipo del cliente que guarda copias: su id (para enlazarlo). */
  equipo?: string;
  inmutable: boolean;
  repos: number;
  /** Lo que ocupan los archivos protegidos (la última versión de cada repositorio). */
  protegido: number;
  /** Lo que ocupa en disco (comprimido y sin duplicados), si los equipos lo informan. */
  enDisco: number | null;
  /** Datos protegidos al final de cada uno de los últimos `dias` días (el más reciente al final). */
  serie: { dia: string; bytes: number }[];
}

/**
 * Dónde se guardan las copias del cliente y cuánto: los repositorios
 * agrupados por destino (un equipo que guarda copias cuenta como uno solo
 * aunque cada equipo lo llame a su manera).
 */
export function almacenes(equipos: Equipo[], informes: Record<string, Informe | null>, cuantos = 30, ahora = Date.now()): Almacen[] {
  const porId = new Map(equipos.map((e) => [e.id, e]));
  const m = new Map<string, Almacen & { _repos: RepoInforme[]; _bytes: number[] }>();
  for (const e of equipos) {
    if (e.modo === "trasladado") continue;
    for (const r of e.resumen?.repositorios ?? []) {
      const d = destinoDe(e.resumen?.destinos, r);
      const alm = d?.equipo_almacen ? porId.get(d.equipo_almacen) : undefined;
      const clave = alm ? `eq|${alm.id}` : d ? `${d.tipo}|${d.donde ?? d.nombre}` : `?|${r.destino}`;
      const x =
        m.get(clave) ??
        m
          .set(clave, {
            clave,
            nombre: alm?.nombre ?? d?.nombre ?? r.destino,
            tipo: alm ? "almacen" : (d?.tipo ?? "otro"),
            donde: alm ? undefined : d?.donde,
            equipo: alm?.id,
            inmutable: !!d?.inmutable,
            repos: 0,
            protegido: 0,
            enDisco: null,
            serie: [],
            _repos: [],
            _bytes: [],
          })
          .get(clave)!;
      const inf = informeDe(informes[e.id], r.id);
      x.repos++;
      x.protegido += bytesRepo(r, inf) ?? 0;
      const disco = inf?.espacio?.en_disco_bytes;
      if (disco != null) x.enDisco = (x.enDisco ?? 0) + disco;
      if (inf) x._repos.push(inf);
      else x._bytes.push(bytesRepo(r, inf) ?? 0);
    }
  }
  const hoy = new Date(ahora);
  const finDe = (i: number) => new Date(hoy.getFullYear(), hoy.getMonth(), hoy.getDate() - (cuantos - 1 - i) + 1).getTime();
  for (const x of m.values()) {
    x.serie = Array.from({ length: cuantos }, (_, i) => {
      const fin = finDe(i);
      let total = x._bytes.reduce((a, b) => a + b, 0);
      for (const inf of x._repos) {
        // La última versión de ese día o antes; si el informe no llega tan atrás, la más antigua que trae.
        const vs = (inf.versiones ?? []).filter((v) => v.total_bytes != null);
        const antes = vs.find((v) => Date.parse(v.hora) < fin);
        total += (antes ?? vs.at(-1))?.total_bytes ?? 0;
      }
      return { dia: claveDia(new Date(fin - 1)), bytes: total };
    });
  }
  return [...m.values()]
    .map(({ _repos: _r, _bytes: _b, ...x }) => x)
    .sort((a, b) => b.protegido - a.protegido || a.nombre.localeCompare(b.nombre));
}
