// «¿Cuándo se llena?» (docs/diseno.md §4): cuánto ocupa cada almacén, destino
// y destino del espejo, a qué ritmo crece y, si se sabe su capacidad, cuándo
// se llenaría al ritmo actual. Es una estimación lineal y aproximada:
//
// - el ritmo sale de lo que añadieron las versiones de los últimos 60 días
//   (lo nuevo empaquetado de cada una; el informe no trae más atrás) y no
//   descuenta lo que quite la retención;
// - la capacidad, del propio almacén (`guarda_copias.espacio`, v1.31) y de
//   cada destino del espejo (carpeta: su disco; nube: la cuenta);
// - la historia se reconstruye hacia atrás desde lo que ocupa hoy, restando
//   lo que añadió cada día.
import type { EspacioVolumen, Equipo, Informe, RepoInforme } from "./tipos";
import { anadidoDe, destinoDe, informeDe } from "./repo";
import { claveDestino } from "./mapa";
import { bytes } from "./formato";
import type { Tono } from "./salud";

const DIA = 86_400_000;
/** Días de historia que se miran (los del informe). */
export const VENTANA = 60;
const MES = 30.44;

export type IconoLlenado = "almacen" | "disco" | "nube" | "servidor";

export interface Prevision {
  clave: string;
  nombre: string;
  /** «Almacén», «Espejo de ALMACEN-SUR», «Backblaze B2»… */
  sub: string;
  icono: IconoLlenado;
  href?: string;
  /** Ocupado hoy: del disco entero si se sabe la capacidad; si no, lo que ocupan sus repositorios. */
  usado: number | null;
  total: number | null;
  libre: number | null;
  /** Cuándo se midió el espacio. */
  medido: string | null;
  /** Bytes por día (null: sin versiones con lo añadido). */
  porDia: number | null;
  /** Días con datos para el ritmo. */
  dias: number;
  /** Ocupado al final de cada día de la ventana (reconstruido), el más antiguo primero. */
  serie: { t: number; v: number }[];
  /** Cuándo se llenaría (ms), o null. */
  lleno: number | null;
  tono: Tono;
  estado: string;
  frase: string;
}

const fmtMes = new Intl.DateTimeFormat("es", { month: "long", year: "numeric" });
/** «marzo de 2028». */
export const mesYAno = (t: number) => fmtMes.format(new Date(t));

/** «~14 meses», «~20 días», «~3 años», «más de 5 años». */
export function plazoEnPalabras(dias: number): string {
  if (dias > 5 * 365) return "más de 5 años";
  if (dias < 45) return `~${Math.max(1, Math.round(dias))} ${Math.round(dias) === 1 ? "día" : "días"}`;
  const meses = dias / MES;
  if (meses < 24) return `~${Math.round(meses)} meses`;
  const anos = Math.round((meses / 12) * 2) / 2;
  return `~${String(anos).replace(".", ",")} años`;
}

/** «~1,5 GB al mes». */
export const ritmoEnPalabras = (porDia: number) => `~${bytes(porDia * MES)} al mes`;

interface Origen {
  infs: RepoInforme[];
  enDisco: number | null;
}

/** Bytes añadidos por día (clave «AAAA-MM-DD» local) y los días que cubren las versiones. */
function anadidos(infs: RepoInforme[], ahora: number): { porDia: number | null; dias: number; despues: (t: number) => number } {
  const vs = infs.flatMap((i) => i.versiones ?? []).filter((v) => anadidoDe(v) != null && Date.parse(v.hora) > ahora - VENTANA * DIA);
  if (!vs.length) return { porDia: null, dias: 0, despues: () => 0 };
  const primera = Math.min(...vs.map((v) => Date.parse(v.hora)));
  const dias = Math.max(7, (ahora - primera) / DIA);
  const total = vs.reduce((s, v) => s + (anadidoDe(v) ?? 0), 0);
  const ordenadas = vs.map((v) => ({ t: Date.parse(v.hora), b: anadidoDe(v) ?? 0 }));
  return { porDia: total / dias, dias: Math.round(dias), despues: (t) => ordenadas.filter((x) => x.t > t).reduce((s, x) => s + x.b, 0) };
}

function prevision(
  base: Omit<Prevision, "usado" | "total" | "libre" | "medido" | "porDia" | "dias" | "serie" | "lleno" | "tono" | "estado" | "frase">,
  origen: Origen,
  espacio: EspacioVolumen | null | undefined,
  ahora: number,
  sinCapacidad: string,
): Prevision {
  const { porDia, dias, despues } = anadidos(origen.infs, ahora);
  const total = espacio?.total ?? null;
  const libre = espacio ? Math.min(espacio.libre, espacio.total) : null;
  const usado = espacio ? espacio.total - (libre ?? 0) : origen.enDisco;
  const serie: Prevision["serie"] = [];
  if (usado != null) {
    const hoy = new Date(ahora);
    for (let i = VENTANA - 1; i >= 0; i--) {
      const t = new Date(hoy.getFullYear(), hoy.getMonth(), hoy.getDate() - i + 1).getTime() - 1;
      const fin = Math.min(t, ahora);
      serie.push({ t: fin, v: Math.max(0, usado - despues(fin)) });
    }
  }
  const lleno = libre != null && porDia && porDia > 0 ? ahora + (libre / porDia) * DIA : null;
  const quedan = lleno ? (lleno - ahora) / DIA : null;
  let tono: Tono = "neutral";
  let estado = "Sin datos";
  let frase: string;
  if (porDia == null) frase = usado != null ? `Ocupa ${bytes(usado)}. Todavía no hay versiones recientes para saber a qué ritmo crece.` : "Todavía no hay versiones para saber a qué ritmo crece.";
  else if (porDia < 1024 * 1024) {
    tono = total != null ? "ok" : "neutral";
    estado = total != null ? "Con sitio" : "Sin crecer";
    frase = `En los últimos ${dias} días casi no ha crecido.${total != null && libre != null ? ` Quedan ${bytes(libre)} libres de ${bytes(total)}.` : ""}`;
  } else if (quedan == null) {
    estado = "Sin capacidad";
    frase = `Crece ${ritmoEnPalabras(porDia)}. ${sinCapacidad}`;
  } else {
    tono = quedan < 30 ? "bad" : quedan < 90 ? "warn" : "ok";
    estado = quedan < 30 ? "Se llena pronto" : quedan < 90 ? "Queda poco sitio" : "Con sitio";
    frase =
      quedan > 5 * 365
        ? `Al ritmo actual (${ritmoEnPalabras(porDia)}), tiene sitio para más de 5 años.`
        : `Al ritmo actual (${ritmoEnPalabras(porDia)}), se llena en ${plazoEnPalabras(quedan)} (${mesYAno(lleno!)}).`;
  }
  return { ...base, usado, total, libre, medido: espacio?.leido ?? null, porDia, dias, serie, lleno, tono, estado, frase };
}

/** Las previsiones del cliente: cada destino (un almacén cuenta una vez) y cada destino del espejo de un almacén. */
export function previsiones(equipos: Equipo[], informes: Record<string, Informe | null | undefined>, cliente: string, ahora = Date.now(), todos: Equipo[] = equipos): Prevision[] {
  const grupos = new Map<string, { nombre: string; sub: string; icono: IconoLlenado; href?: string; almacen?: Equipo; origen: Origen }>();
  for (const e of equipos) {
    if (e.modo === "trasladado") continue;
    for (const r of e.resumen?.repositorios ?? []) {
      const { clave, almacen } = claveDestino(e, r, todos);
      const d = destinoDe(e.resumen?.destinos, r);
      const g =
        grupos.get(clave) ??
        grupos
          .set(clave, {
            nombre: almacen?.nombre ?? d?.nombre ?? r.destino,
            sub: almacen ? "Almacén" : ({ rest: "Servidor de copias", local: "Disco del equipo", s3: "S3", b2: "Backblaze B2", sftp: "SFTP", otro: "Destino" } as const)[d?.tipo ?? "otro"],
            icono: almacen ? "almacen" : d?.tipo === "local" ? "disco" : d?.tipo === "rest" || d?.tipo === "sftp" ? "servidor" : "nube",
            href: almacen ? `/c/${cliente}/equipos/${almacen.id}` : undefined,
            almacen,
            origen: { infs: [], enDisco: null },
          })
          .get(clave)!;
      const inf = informeDe(informes[e.id], r.id);
      if (inf) g.origen.infs.push(inf);
      const disco = inf?.espacio?.en_disco_bytes;
      if (disco != null) g.origen.enDisco = (g.origen.enDisco ?? 0) + disco;
    }
  }
  const out: Prevision[] = [];
  for (const [clave, g] of grupos) {
    const gc = g.almacen?.resumen?.guarda_copias;
    const nube = g.icono === "nube";
    const sin = g.almacen
      ? `Para saber cuándo se llena, actualiza el agente de ${g.almacen.nombre} (dirá el espacio de su disco).`
      : nube
        ? "El proveedor no dice un límite de espacio."
        : "El equipo no informa del espacio de ese disco.";
    out.push(prevision({ clave, nombre: g.nombre, sub: g.sub, icono: g.icono, href: g.href }, g.origen, gc?.espacio, ahora, sin));
    // El espejo recibe todo lo del almacén: crece igual (y nunca borra).
    for (const [i, d] of (gc?.espejo?.destinos ?? []).entries()) {
      const nubeD = d.tipo === "nube";
      out.push(
        prevision(
          { clave: `${clave}|es${i}`, nombre: (nubeD ? d.nube : d.carpeta) ?? "Espejo", sub: `Espejo de ${g.nombre}`, icono: nubeD ? "nube" : "disco", href: g.href },
          g.origen,
          d.espacio,
          ahora,
          nubeD ? "La nube aún no ha dicho su espacio (se lee tras cada espejo)." : `Para saber cuándo se llena, actualiza el agente de ${g.nombre}.`,
        ),
      );
    }
  }
  const PESO: Record<Tono, number> = { bad: 0, warn: 1, ok: 2, info: 3, paused: 3, neutral: 4 };
  return out.sort((a, b) => PESO[a.tono] - PESO[b.tono] || (a.lleno ?? Infinity) - (b.lleno ?? Infinity) || a.nombre.localeCompare(b.nombre));
}

/** El valor (ocupado) en un momento: la historia hasta hoy y la línea recta después. */
export function valorEn(p: Prevision, t: number, ahora: number): number | null {
  if (p.usado == null) return null;
  if (t >= ahora) return p.porDia != null ? p.usado + ((t - ahora) / DIA) * p.porDia : p.usado;
  const s = p.serie;
  if (!s.length || t < s[0].t) return null;
  let i = s.findIndex((x) => x.t >= t);
  if (i < 0) i = s.length - 1;
  return s[i].v;
}
