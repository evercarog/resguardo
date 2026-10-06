// «Retención en detalle» en el modo de demostración: las vueltas de la
// retención que anotaría el equipo (v1.4x, `tipo: "retencion"` en su
// historial): una cada domingo a las 03:00 de las últimas 8 semanas, con las
// versiones que quitó (ids inventados, de antes de las que trae el informe) y
// una que falló.
import type { EntradaRetencion } from "../lib/retencionDetalle";
import type * as T from "../lib/tipos";

export function retencionesMock(eq: { id: string; resumen: T.Equipo["resumen"] }): EntradaRetencion[] {
  const out: EntradaRetencion[] = [];
  const ahora = new Date();
  for (const r of eq.resumen?.repositorios ?? []) {
    if (!r.retencion && !r.retencion_regla) continue;
    const copias = (eq.resumen?.copias ?? []).filter((k) => k.repo === r.id);
    const regla: T.Regla = r.retencion_regla ?? { diarias: 7, semanales: 4, mensuales: 12, anuales: 2 };
    let quedan = 40;
    for (let semana = 0; semana < 8; semana++) {
      const domingo = new Date(ahora.getFullYear(), ahora.getMonth(), ahora.getDate() - ((ahora.getDay() + 7) % 7) - semana * 7, 3, 0, 4);
      if (domingo.getTime() > ahora.getTime()) continue;
      const id = `${r.id}-ret-${semana}`;
      if (semana === 3) {
        out.push({ id, hora: domingo.toISOString(), tipo: "retencion", origen: "equipo", por: "orden", repo: r.id, regla, resultado: "fallo", mensaje: "No se pudo conectar con el destino (se volverá a intentar).", antes: quedan });
        continue;
      }
      const n = 3 + ((semana * 5 + r.id.length) % 6);
      const versiones: NonNullable<EntradaRetencion["versiones"]> = [];
      for (let i = 0; i < n; i++) {
        // De la semana anterior a la vuelta, más o menos un día cada una (algunas el mismo día).
        const t = domingo.getTime() - (8 + i * 0.7) * 86_400_000 - (i % 3) * 3600_000;
        const motivo = i % 4 === 0 ? 1 : 0;
        versiones.push([`${(0xa1000000 + semana * 4096 + i * 17 + r.id.length).toString(16).slice(0, 8)}`, Math.round(t / 1000), i % Math.max(1, copias.length), 1_200_000_000 + i * 3_000_000, motivo]);
      }
      out.push({
        id,
        hora: domingo.toISOString(),
        inicio: new Date(domingo.getTime() - 4 * 60_000).toISOString(),
        tipo: "retencion",
        origen: "equipo",
        por: semana % 2 ? "ventana" : "orden",
        repo: r.id,
        regla,
        resultado: "ok",
        mensaje: "Retención aplicada.",
        antes: quedan + n,
        quedan,
        quitadas: n,
        liberado: n * 37_000_000,
        grupos: copias.length ? copias.map((k) => ({ copia: k.id, quedan: Math.round(quedan / copias.length) })) : [{ copia: null, quedan }],
        motivos: ["cupo:diarias", "repe:diarias"],
        versiones,
      });
      quedan += 2;
    }
  }
  return out;
}
