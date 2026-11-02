// 16x (7): desde cuándo pedir los informes nuevos (`GET …/informes?desde=`): el más
// reciente de los que ya hay. El servidor da los de ese mismo segundo o después
// (alguno se repite: da igual, se junta). Sin ninguno, todos.
import type { Informe } from "./tipos";

export function desdeInformes(porEquipo: Record<string, Informe | null | undefined>): string | undefined {
  let max = Number.NEGATIVE_INFINITY;
  let texto: string | undefined;
  for (const i of Object.values(porEquipo)) {
    const t = i?.recibido ? Date.parse(i.recibido) : Number.NaN;
    if (Number.isFinite(t) && t > max) {
      max = t;
      texto = i!.recibido;
    }
  }
  return texto;
}
