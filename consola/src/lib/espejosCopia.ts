// Lo que cuelga del repositorio de una copia (espejos y copias derivadas),
// agrupado por quién lo hace: el almacén donde está el repositorio (sus
// espejos) o el propio equipo (sus copias derivadas y la copia externa).
// Lo usa la lista de copias del editor (plan 0.7.26, bloque 3.1: «debajo sus
// espejos y derivadas, agrupados por almacén»). Con vectores en
// scripts/vectores-editor-guiado.ts.
//
// Bloque 4: con un agente que manda `espejo.trabajos[]` (espejos como trabajos,
// varios por repositorio y hechos también por el propio equipo), los espejos
// salen de ahí (`espejosDeCopia` de lib/espejoTrabajos.ts); con uno anterior, de
// `pasosDelRepo`. La forma de lo que devuelve no cambia.
import { pasosDelRepo, repoEnAlmacen, type PasoCadena } from "./cadenas";
import { TIPOS_NUBE } from "./espejo";
import { admiteTrabajos, espejosDeCopia as gruposDeTrabajos, textoCuando, textoRetencion, type TrabajoEspejoResumen } from "./espejoTrabajos";
import type { Equipo } from "./tipos";

export interface GrupoEspejos {
  /** Clave estable del grupo (`almacen:<id>` o `equipo:<id>`). */
  clave: string;
  /** Quién lo hace. */
  quien: { id: string; nombre: string; almacen: boolean };
  /** Sus pasos (espejos o copias derivadas), en el orden de siempre. */
  pasos: PasoCadena[];
}

/**
 * Espejos y derivadas del repositorio `repoId` de `equipo`, agrupados: primero
 * el almacén (sus espejos), después el propio equipo (derivadas y copia
 * externa). Vacío si no hay nada o el repositorio no está en el resumen.
 */
export function espejosDeCopia(equipo: Equipo, repoId: string, equipos: Equipo[]): GrupoEspejos[] {
  const pasos = pasosDelRepo(equipo, repoId, equipos).filter((p) => p.nivel > 0);
  const repo = equipo.resumen?.repositorios?.find((r) => r.id === repoId);
  const en = repo ? repoEnAlmacen(equipo, repo, equipos) : null;
  // Bloque 4: con un almacén (o un equipo) que manda sus espejos como trabajos, de ahí
  // (lib/espejoTrabajos.ts, `espejosDeCopia`): nombre, retención y cuándo de cada uno.
  const trabajos = repo ? gruposDeTrabajos(equipo, repo, equipos) : [];
  const conTrabajos = !!en && admiteTrabajos(equipos.find((e) => e.id === en.almacen.id) ?? en.almacen);
  const grupos: GrupoEspejos[] = [];
  const almacenT = trabajos.find((g) => g.quien === "almacen" && !g.fuera);
  if (conTrabajos && almacenT) {
    if (almacenT.espejos.length) grupos.push({ clave: `almacen:${almacenT.id}`, quien: { id: almacenT.id, nombre: almacenT.nombre, almacen: true }, pasos: almacenT.espejos.map((x) => pasoDeTrabajo(x.trabajo, x.hace)) });
  } else {
    const espejos = pasos.filter((p) => p.clase === "espejo");
    if (espejos.length) {
      const a = en?.almacen;
      grupos.push({ clave: `almacen:${a?.id ?? "?"}`, quien: { id: a?.id ?? "", nombre: a?.nombre ?? "El almacén", almacen: true }, pasos: espejos });
    }
  }
  const derivadas = pasos.filter((p) => p.clase !== "espejo");
  // Los espejos que hace el propio equipo (de los repositorios de sus discos) van con sus derivadas.
  const propios = trabajos.find((g) => g.quien === "equipo")?.espejos.map((x) => pasoDeTrabajo(x.trabajo, x.hace)) ?? [];
  if (derivadas.length || propios.length) grupos.push({ clave: `equipo:${equipo.id}`, quien: { id: equipo.id, nombre: equipo.nombre, almacen: false }, pasos: [...propios, ...derivadas] });
  return grupos;
}

/** Un trabajo de espejo como paso de la cadena (para pintarlo como los demás). */
function pasoDeTrabajo(t: TrabajoEspejoResumen, hace: Equipo): PasoCadena {
  const tipoNube = t.adonde.tipo === "nube" ? [...(hace.resumen?.nubes ?? []), ...(hace.resumen?.guarda_copias?.nubes ?? [])].find((n) => n.nombre === t.adonde.nube)?.tipo : undefined;
  const noInmutable = tipoNube && TIPOS_NUBE[tipoNube] && !TIPOS_NUBE[tipoNube].inmutable ? `${TIPOS_NUBE[tipoNube].nombre} no es inmutable: un ransomware con acceso a la cuenta podría borrar lo de allí.` : undefined;
  return {
    clase: "espejo",
    texto: t.nombre,
    detalle: ["espejo", textoRetencion(t).toLowerCase(), textoCuando(t).toLowerCase(), t.activo ? "" : "en pausa"].filter(Boolean).join(" · "),
    despues: !!(t.cuando.tras_copia || t.cuando.cadena || t.cuando.despues),
    nivel: 1,
    destinoId: t.adonde.tipo === "zona" ? `zona:${hace.id}:${t.adonde.carpeta}` : undefined,
    tipoDestino: t.adonde.tipo,
    inmutable: t.bloqueo || t.bloqueo_dias ? true : tipoNube ? (TIPOS_NUBE[tipoNube]?.inmutable ?? null) || null : null,
    noInmutable,
    fueraRetencion: t.retencion.modo === "nunca",
  };
}
