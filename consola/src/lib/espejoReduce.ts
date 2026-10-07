// ¿Reduce la protección un cambio de los trabajos de espejo? (plan 0.7.26, bloque 4).
// Aparte y sin dependencias para que lo use la orden sellada (cripto/ordenes.ts)
// sin ciclos. Lo mismo que el agente (`espejo_trabajos::reduce`), con vectores
// compartidos (crates/protocolo/vectors/espejo-trabajos.json).
import type { FrenoEspejo, RetencionEspejo, TrabajoEspejo } from "./tipos";

export const FRENO_DEFECTO: FrenoEspejo = { pct: 10, min_archivos: 100, min_faltan: 20, accion: "confirmar" };
const PRINCIPAL = "principal";

/** Lo que identifica su destino (dos trabajos al mismo sitio), como el agente. */
export function claveDestinoTrabajo(t: Pick<TrabajoEspejo, "quien" | "adonde" | "zona">, windows = true): string {
  let carpeta = t.adonde.carpeta.trim().replace(/[\\/]+$/, "");
  if (windows || t.adonde.tipo === "nube") carpeta = carpeta.toLowerCase();
  return `${t.quien}|${t.adonde.tipo}|${(t.adonde.nube ?? "").trim()}|${carpeta}|${t.zona && t.zona !== PRINCIPAL ? t.zona : PRINCIPAL}`;
}

const diasHastaBorrar = (r: RetencionEspejo) => (r.modo === "nunca" ? Infinity : r.modo === "igual" ? 0 : r.dias);

/** ¿Reduce la protección pasar de `antes` a `ahora`? (la orden espera): quitar o pausar uno, cambiar su destino, dejar fuera repositorios, borrar antes, quitar el bloqueo o aflojar el freno. */
export function reduceTrabajos(antes: TrabajoEspejo[], ahora: TrabajoEspejo[]): boolean {
  return antes
    .filter((a) => a.activo !== false)
    .some((a) => {
      const n = ahora.find((x) => x.id === a.id);
      if (!n || n.activo === false || claveDestinoTrabajo(n) !== claveDestinoTrabajo(a)) return true;
      let menos: boolean;
      if (n.que.tipo === "todos") menos = false;
      else if (a.que.tipo === "todos") menos = true;
      else if (a.que.tipo === "equipos" && n.que.tipo === "equipos") {
        const y = n.que.equipos;
        menos = a.que.equipos.some((r) => !y.includes(r));
      } else if (a.que.tipo === "repos" && n.que.tipo === "repos") {
        const y = n.que.repos;
        menos = a.que.repos.some((r) => !y.includes(r));
      } else menos = true;
      const borraAntes = diasHastaBorrar(n.retencion) < diasHastaBorrar(a.retencion);
      const bloqueo = (!!a.bloqueo && !n.bloqueo) || (!!a.bloqueo_dias && (!n.bloqueo_dias || n.bloqueo_dias < a.bloqueo_dias));
      const f = a.freno ?? FRENO_DEFECTO;
      const g = n.freno ?? FRENO_DEFECTO;
      const freno = g.pct > f.pct || g.min_archivos > f.min_archivos || g.min_faltan > f.min_faltan || (f.accion === "confirmar" && g.accion === "avisar");
      return menos || borraAntes || bloqueo || freno;
    });
}
