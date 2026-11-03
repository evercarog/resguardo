// 16m y 16o: lo que necesita atención en Estado, con un solo «qué hago» cada cosa y los
// repetidos juntos («3 equipos sin contacto» en vez de tres líneas iguales), y lo que
// contesta «¿estoy protegido?» arriba (las tres cifras). Sin runas: se prueba en
// scripts/vectores.ts.
import type { Equipo, Informe } from "./tipos";
import { PESO, type Tono } from "./salud";
import { lista, plural } from "./formato";
import { informeDe } from "./repo";
import type { EstadoFiltro } from "./filtrosEquipos";

export interface Urgente {
  tono: Tono;
  texto: string;
  /** Quién (el equipo) y qué le pasa, por separado. */
  titulo?: string;
  detalle?: string;
  accion?: { texto: string; href?: string; orden?: { equipo: Equipo; repo: string; copia: string; nombre: string } };
  /** Los que tienen el mismo `grupo` se juntan en una línea (con `grupoTexto`). */
  grupo?: string;
  /** «sin contacto»: lo que les pasa a todos los del grupo (para «3 equipos sin contacto»). */
  grupoTexto?: string;
  /** El nombre de quien lo tiene (para listar los del grupo). */
  quien?: string;
}

/**
 * Junta los que tienen el mismo `grupo` (dos o más): «3 equipos · sin contacto», los nombres en
 * el detalle y una sola acción, que lleva a la lista filtrada (`hrefGrupo`). Lo más grave, arriba.
 */
export function agruparUrgentes(us: Urgente[], hrefGrupo: (grupo: string) => { texto: string; href: string }): Urgente[] {
  const porGrupo = new Map<string, Urgente[]>();
  const out: Urgente[] = [];
  for (const u of us) {
    if (!u.grupo) {
      out.push(u);
      continue;
    }
    const g = porGrupo.get(u.grupo);
    if (g) g.push(u);
    else {
      porGrupo.set(u.grupo, [u]);
      out.push(u);
    }
  }
  return out
    .map((u) => {
      const g = u.grupo ? porGrupo.get(u.grupo)! : null;
      if (!g || g.length < 2) return u;
      const tono = g.reduce<Tono>((t, x) => (PESO[x.tono] < PESO[t] ? x.tono : t), "ok");
      const nombres = g.map((x) => x.quien ?? x.titulo ?? x.texto);
      const texto = `${g.length} equipos · ${u.grupoTexto ?? "necesitan atención"}`;
      return { tono, texto, titulo: texto, detalle: nombres.length > 4 ? `${nombres.slice(0, 4).join(", ")} y ${nombres.length - 4} más.` : `${lista(nombres)}.`, accion: hrefGrupo(u.grupo!) } satisfies Urgente;
    })
    .sort((a, b) => PESO[a.tono] - PESO[b.tono]);
}

/**
 * El filtro de Equipos que corresponde a un grupo de urgentes (`salud:<texto>`): «Ver los
 * equipos» de «3 equipos · sin contacto» lleva a los que no se conectan, no a todos los que
 * necesitan atención.
 */
export function estadoDeGrupo(grupo: string): EstadoFiltro {
  switch (grupo) {
    case "salud:Sin contacto":
      return "sin_conexion";
    case "salud:Copia fallida":
    case "salud:Detenido":
      return "fallando";
    case "salud:Atrasado":
      return "atrasada";
    case "salud:En pausa":
      return "pausa";
    default:
      return "atencion";
  }
}

/**
 * El titular de Estado. Con filtros que dejan equipos fuera, nunca dice «Todo protegido»: dice
 * en cuántos equipos se mira («… en 8 de 200 equipos»), porque los ocultos pueden fallar.
 * `vistos`: los que pasan los filtros; `total`: todos (sin duplicados); `sinPrimera`: los vistos
 * que aún no han hecho su primera copia; `urgentes`: lo que necesita atención entre los vistos.
 */
export function titularEstado(o: { vistos: number; total: number; conFiltros: boolean; urgentes: number; sinPrimera: number }): { texto: string; filtrado: boolean } {
  const filtrado = o.conFiltros && o.vistos < o.total;
  if (!o.vistos) return { texto: o.total && o.conFiltros ? "Ningún equipo con esos filtros" : "Todavía no hay equipos", filtrado };
  const en = filtrado ? ` en ${o.vistos} de ${plural(o.total, "equipo", "equipos")}` : "";
  if (o.urgentes) return { texto: `${o.urgentes === 1 ? "1 cosa necesita atención" : `${o.urgentes} cosas necesitan atención`}${en}`, filtrado };
  if (o.sinPrimera === o.vistos) return { texto: `Sin copias todavía${en}`, filtrado };
  if (o.sinPrimera)
    return {
      texto: filtrado
        ? `${o.sinPrimera} de los ${o.vistos} equipos filtrados ${o.sinPrimera === 1 ? "aún no tiene copias" : "aún no tienen copias"}`
        : `${plural(o.sinPrimera, "equipo aún sin copias", "equipos aún sin copias")}; el resto, protegido`,
      filtrado,
    };
  return { texto: filtrado ? `Sin problemas${en}` : "Todo protegido", filtrado };
}

/** El plazo tras el que una prueba de restauración ya no cuenta (como en la regla 3-2-1-1-0). */
export const PRUEBA_VALE_DIAS = 45;

/**
 * 16m: la última prueba de restauración del cliente (automática o una restauración real que salió
 * bien) y cuántos repositorios tienen una reciente. `tono`: «ok» si la última salió bien y es de
 * hace menos del plazo; «warn» si es vieja, falló o no hay ninguna.
 */
export function ultimaPrueba(equipos: Equipo[], informes: Record<string, Informe | null | undefined>, ahora = Date.now()): { cuando: string | null; tono: Tono; recientes: number; repos: number } {
  let cuando: string | null = null;
  let bien = true;
  let recientes = 0;
  let repos = 0;
  const limite = ahora - PRUEBA_VALE_DIAS * 86_400_000;
  for (const e of equipos) {
    if (e.modo === "trasladado") continue;
    for (const r of e.resumen?.repositorios ?? []) {
      repos++;
      const inf = informeDe(informes[e.id] ?? null, r.id);
      const p = inf?.prueba_restauracion;
      const real = inf?.restauracion_real?.ultima ?? null;
      const suya = [p?.resultado !== "fallo" ? (p?.ultima ?? null) : null, real].filter((x): x is string => !!x).sort().at(-1) ?? null;
      if (suya && Date.parse(suya) >= limite) recientes++;
      for (const [t, ok] of [[p?.ultima ?? null, p?.resultado !== "fallo"], [real, true]] as const)
        if (t && (!cuando || t > cuando)) {
          cuando = t;
          bien = ok;
        }
    }
  }
  const tono: Tono = cuando && bien && Date.parse(cuando) >= limite ? "ok" : "warn";
  return { cuando, tono, recientes, repos };
}

/**
 * 16o: los avisos repetidos, juntos: los del mismo tipo y el mismo motivo (sin los nombres entre
 * «» ni las cifras, que cambian de un equipo o de una hora a otra) salen en una sola fila, la más
 * reciente, con los demás en `repetidos` («3 copias fallaron por lo mismo»). Los vistos y los sin
 * ver no se mezclan. Mantiene el orden de entrada.
 */
export function agruparAvisos<A extends { id: string; tipo: string; mensaje: string; creado: string; visto_por: string | null }>(xs: A[]): (A & { repetidos: A[] })[] {
  const clave = (a: A) => `${a.tipo}|${a.visto_por ? "v" : "n"}|${a.mensaje.replace(/«[^»]*»/g, "«»").replace(/\d+([.,:]\d+)*/g, "#").trim().toLowerCase()}`;
  const m = new Map<string, A & { repetidos: A[] }>();
  const out: (A & { repetidos: A[] })[] = [];
  for (const a of xs) {
    const k = clave(a);
    const ya = m.get(k);
    if (ya) {
      ya.repetidos.push(a);
      continue;
    }
    const x = { ...a, repetidos: [] as A[] };
    m.set(k, x);
    out.push(x);
  }
  return out;
}

/**
 * 16s: el botón de una página vacía porque aún no hay copias: sin equipos, «Añadir equipo»; con
 * un solo equipo sin copias, crear su primera copia ahí mismo; si no, la lista de equipos (con los
 * que necesitan algo arriba). null si la persona no puede hacer nada de eso.
 */
export function primerPaso(cliente: string, equipos: Pick<Equipo, "id" | "nombre" | "modo" | "resumen">[], administra: boolean): { texto: string; href: string } | null {
  const activos = equipos.filter((e) => e.modo !== "trasladado");
  if (!activos.length) return administra ? { texto: "Añadir equipo", href: `/c/${cliente}/emparejar` } : null;
  const sinCopias = activos.filter((e) => !(e.resumen?.copias ?? []).length);
  if (activos.length === 1 && sinCopias.length === 1 && administra) return { texto: `Crear la primera copia de ${activos[0].nombre}`, href: `/c/${cliente}/equipos/${activos[0].id}/copias?nueva=1` };
  return { texto: "Ver los equipos", href: `/c/${cliente}/equipos` };
}
