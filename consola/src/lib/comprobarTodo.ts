// 16v (plan 0.8.3): «Comprobar todo» de un cliente. Con las órdenes que ya existen
// y sin capacidades nuevas en los equipos:
//
// - cada repositorio con versiones: «Verificar ahora» (`verificar_ahora { repo }`);
// - cada copia derivada: su verificación (`verificar_ahora { repo, derivada }`);
// - cada espejo al que llega el equipo dueño sin preguntar nada: una prueba de
//   restauración desde él (`probar_restauracion { repo, espejo }`, la de 14j); los que
//   piden elegir una nube o una carpeta se dejan dichos, para hacerlos desde su página;
// - los kits, con la persona: la contraseña la escribe ella, uno a uno (la orden de
//   siempre, `comprobar_contrasena`); aquí solo se dice cuáles faltan y cómo salieron.
//
// Todas son inofensivas (no borran ni cambian nada, sin clave de administración ni
// espera). El veredicto sale de lo que cada equipo cuenta después en su informe.
import { csv } from "./csv";
import { opcionesPruebaEspejo } from "./pruebasFuncionales";
import type { ReglaCopia } from "./regla321";
import type { Equipo, Informe, Orden, RepositorioResumen } from "./tipos";

export type ClaseComprobacion = "repo" | "derivada" | "espejo";

export interface Comprobacion {
  /** Única en el plan (equipo, repositorio y qué). */
  clave: string;
  clase: ClaseComprobacion;
  equipo: Equipo;
  repo: RepositorioResumen;
  /** Lo que se enseña: el repositorio, o su derivada o espejo. */
  nombre: string;
  /** El id de la derivada o de la prueba del espejo (para leer su resultado). */
  id: string | null;
  /** La orden que se manda; null: no se manda (y `nota` dice por qué). */
  orden: { tipo: string; cuerpo: Record<string, unknown> } | null;
  nota: string;
}

/** Los equipos que cuentan: confirmados y gestionados desde aquí. */
export const equiposAComprobar = (equipos: Equipo[]) => equipos.filter((e) => e.confirmado !== false && e.modo === "gestionado");

/** Lo que se va a comprobar en el cliente, en orden: por equipo, repositorios, derivadas y espejos. */
export function planComprobar(equipos: Equipo[], reglas: ReglaCopia[], todos: Equipo[] = equipos): Comprobacion[] {
  const out: Comprobacion[] = [];
  for (const e of equiposAComprobar(equipos)) {
    const espejos = !!e.resumen?.admite?.includes("prueba_espejo");
    for (const r of e.resumen?.repositorios ?? []) {
      const base = { equipo: e, repo: r };
      const conVersiones = (r.versiones ?? 0) > 0;
      if (r.solo_lectura) {
        out.push({ ...base, clave: `${e.id}|${r.id}|repo`, clase: "repo", nombre: r.nombre, id: null, orden: null, nota: "Solo lectura en este equipo: lo verifica el equipo que copia en él." });
        continue;
      }
      out.push({
        ...base,
        clave: `${e.id}|${r.id}|repo`,
        clase: "repo",
        nombre: r.nombre,
        id: null,
        orden: conVersiones ? { tipo: "verificar_ahora", cuerpo: { repo: r.id } } : null,
        nota: conVersiones ? "" : "Aún no tiene versiones: no hay nada que verificar.",
      });
      for (const d of r.derivadas ?? []) {
        if (d.activa === false) continue;
        out.push({
          ...base,
          clave: `${e.id}|${r.id}|derivada|${d.id}`,
          clase: "derivada",
          nombre: d.nombre || `${r.nombre} (copia derivada)`,
          id: d.id,
          orden: conVersiones ? { tipo: "verificar_ahora", cuerpo: { repo: r.id, derivada: d.id } } : null,
          nota: conVersiones ? (d.destino ? `En «${d.destino}».` : "") : "Su repositorio aún no tiene versiones.",
        });
      }
      // Los espejos, de las copias que guardan en este repositorio (uno por espejo).
      const vistos = new Set<string>();
      for (const rc of reglas) {
        if (rc.equipo.id !== e.id || rc.repo.id !== r.id) continue;
        for (const o of opcionesPruebaEspejo(rc.pasos, e, todos)) {
          if (vistos.has(o.id)) continue;
          vistos.add(o.id);
          const listo = espejos && o.modo === "listo" && conVersiones;
          out.push({
            ...base,
            clave: `${e.id}|${r.id}|espejo|${o.id}`,
            clase: "espejo",
            nombre: o.nombre,
            id: o.id,
            orden: listo ? { tipo: "probar_restauracion", cuerpo: { repo: r.id, espejo: o.orden } } : null,
            nota: listo
              ? "Prueba de restauración desde el espejo: unos archivos a una carpeta temporal, comparados. No toca tus archivos."
              : !espejos
                ? `Actualiza Resguardo en «${e.nombre}» para probar a restaurar desde sus espejos.`
                : !conVersiones
                  ? "Su repositorio aún no tiene versiones."
                  : o.modo === "no"
                    ? o.nota
                    : `${o.nota} Hazla desde la página del repositorio («Pruebas desde los espejos»).`,
          });
        }
      }
    }
  }
  return out;
}

export type Veredicto = "bien" | "aviso" | "mal" | "esperando" | "no_se_manda";

export interface Resultado {
  veredicto: Veredicto;
  texto: string;
  /** Cuándo lo hizo el equipo (si ya lo contó). */
  cuando: string | null;
}

const FINAL_MAL = ["fallida", "rechazada", "caducada", "cancelada"];
/** Un poco antes de pedirlo: los relojes del equipo y del navegador no van exactos. */
export const MARGEN_RELOJ_MS = 60_000;

/** Una tarea del informe hecha después de `desde` (ms), con su resultado. */
function tarea(t: { ultima?: string | null; resultado?: string | null; mensaje_corto?: string | null } | null | undefined, desde: number): Resultado | null {
  const cuando = t?.ultima ? Date.parse(t.ultima) : Number.NaN;
  if (!t || !Number.isFinite(cuando) || cuando < desde - MARGEN_RELOJ_MS) return null;
  const v: Veredicto = t.resultado === "ok" ? "bien" : t.resultado === "aviso" ? "aviso" : "mal";
  return { veredicto: v, texto: t.mensaje_corto || (v === "bien" ? "Bien." : v === "aviso" ? "Con avisos." : "Falló."), cuando: t.ultima ?? null };
}

/**
 * Cómo quedó una comprobación pedida en `desde` (ms): lo que dice su orden si no llegó
 * a hacerse, y si no, lo que cuenta el equipo en su informe (`informe`) o en su resumen
 * (`equipo`, el de ahora).
 */
export function resultadoDe(c: Comprobacion, desde: number | null, informe: Informe | null | undefined, equipo: Equipo | undefined, orden?: Orden | null): Resultado {
  if (!c.orden) return { veredicto: "no_se_manda", texto: c.nota, cuando: null };
  if (desde === null) return { veredicto: "esperando", texto: "Sin pedir.", cuando: null };
  if (orden && FINAL_MAL.includes(orden.estado))
    return { veredicto: "mal", texto: orden.mensaje || (orden.estado === "caducada" ? "El equipo no la recibió a tiempo." : "El equipo no la hizo."), cuando: null };
  const inf = informe?.datos.repos?.find((x) => x.id === c.repo.id);
  if (c.clase === "repo") {
    const r = tarea(inf?.verificacion, desde);
    if (r) return r;
    // Un agente cuyo informe aún no llegó: la fecha de su resumen (solo dice que fue bien).
    const v = equipo?.resumen?.repositorios?.find((x) => x.id === c.repo.id)?.verificado;
    if (v && Date.parse(v) >= desde - MARGEN_RELOJ_MS) return { veredicto: "bien", texto: "Verificado.", cuando: v };
  } else if (c.clase === "derivada") {
    const r = tarea(inf?.derivadas?.find((x) => x.id === c.id)?.verificacion, desde);
    if (r) return r;
  } else {
    const p = inf?.pruebas_espejo?.find((x) => x.id === c.id);
    const r = tarea(p ? { ultima: p.ultima, resultado: p.resultado === "ok" ? "ok" : p.resultado ? "fallo" : null, mensaje_corto: p.mensaje_corto } : null, desde);
    if (r) return r;
  }
  if (equipo && !equipo.conectado) return { veredicto: "esperando", texto: "Sin conexión: lo hará cuando vuelva.", cuando: null };
  if (!orden || orden.estado === "pendiente" || orden.estado === "entregada") return { veredicto: "esperando", texto: "Pedida: esperando al equipo…", cuando: null };
  return { veredicto: "esperando", texto: "En marcha o en cola…", cuando: null };
}

/** Las cuentas del informe: cuántas bien, con avisos, mal, sin terminar y sin mandar. */
export function cuentas(rs: Resultado[]) {
  const n = { bien: 0, aviso: 0, mal: 0, esperando: 0, no_se_manda: 0 };
  for (const r of rs) n[r.veredicto]++;
  return n;
}

export interface KitFila {
  equipo: Equipo;
  repo: RepositorioResumen;
  /** El equipo sabe comprobarla (`admite: "comprobar_contrasena"`). */
  admite: boolean;
  resultado: "abre" | "no_abre" | "sin_respuesta" | null;
  cuando: string | null;
}

/** Los kits (contraseñas de los repositorios) del cliente, con su última comprobación. */
export function kitsDe(equipos: Equipo[]): KitFila[] {
  return equiposAComprobar(equipos).flatMap((e) =>
    (e.resumen?.repositorios ?? [])
      .filter((r) => !r.solo_lectura)
      .map((r) => ({
        equipo: e,
        repo: r,
        admite: !!e.resumen?.admite?.includes("comprobar_contrasena"),
        resultado: r.contrasena_comprobada?.resultado ?? null,
        cuando: r.contrasena_comprobada?.ultima ?? null,
      })),
  );
}

/** El informe en CSV (como los demás de la consola: «;», BOM y sin fórmulas). */
export function informeCsv(filas: { c: Comprobacion; r: Resultado }[], kits: KitFila[]): string {
  const V: Record<Veredicto, string> = { bien: "Bien", aviso: "Con avisos", mal: "Mal", esperando: "Sin terminar", no_se_manda: "No se comprobó" };
  const Q: Record<ClaseComprobacion, string> = { repo: "Repositorio", derivada: "Copia derivada", espejo: "Espejo" };
  const K = { abre: "La abre", no_abre: "No la abre", sin_respuesta: "Sin respuesta" } as const;
  return csv(
    ["Equipo", "Qué", "Nombre", "Repositorio", "Resultado", "Detalle", "Cuándo"],
    [
      ...filas.map(({ c, r }) => [c.equipo.nombre, Q[c.clase], c.nombre, c.repo.nombre, V[r.veredicto], r.texto, r.cuando ?? ""]),
      ...kits.map((k) => [k.equipo.nombre, "Kit (contraseña)", k.repo.nombre, k.repo.nombre, k.resultado ? K[k.resultado] : k.admite ? "Sin comprobar" : "El equipo no sabe comprobarla", "", k.cuando ?? ""]),
    ],
  );
}
