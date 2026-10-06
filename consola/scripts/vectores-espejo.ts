// Pruebas del espejo por destino (src/lib/espejo.ts, docs/espejo.md).
// `npm run test:vectores` (con las demás).
import { admiteEspejoFlexible, cuandoEspejo, destinoParaOrden, horaParaConsolasAnteriores, horarioDiario } from "../src/lib/espejo";

let fallos = 0;
let total = 0;
function igual(nombre: string, obtenido: unknown, esperado: unknown) {
  total++;
  const ok = JSON.stringify(obtenido) === JSON.stringify(esperado);
  if (!ok) fallos++;
  console.log(`${ok ? "ok  " : "MAL "} ${nombre}${ok ? "" : `\n       obtenido: ${JSON.stringify(obtenido)}\n       esperado: ${JSON.stringify(esperado)}`}`);
}

console.log("\n· Espejo: cuándo (3a)");
igual("un agente que lo admite", admiteEspejoFlexible({ resumen: { admite: ["espejo_flexible"] } }), true);
igual("uno anterior, no", admiteEspejoFlexible({ resumen: { admite: ["verificacion_horario"] } }), false);
igual("la hora de antes es «cada día a esa hora»", horarioDiario("03:30"), { dias: [1, 2, 3, 4, 5, 6, 7], horas: ["03:30"] });
igual("sin horario propio: cada día a la hora del espejo", cuandoEspejo({}, "02:00"), "Cada día a las 02:00");
const cadaHora = { dias: [], horas: [], reglas: [{ tipo: "intervalo" as const, dias: [1, 2, 3, 4, 5], cada_min: 60, desde: "08:00", hasta: "19:00" }] };
igual("con su horario y después de cada copia", cuandoEspejo({ horario: cadaHora, tras_copia: true }, "02:00").endsWith(" y después de cada copia nueva"), true);
igual("…dice su horario, no la hora del espejo", cuandoEspejo({ horario: cadaHora }, "02:00").includes("02:00"), false);
// Lo que se reenvía de un destino: sus opciones, nunca sus resultados.
igual(
  "un destino del resumen, en la forma de la orden",
  destinoParaOrden({ tipo: "nube", nube: "Dropbox Sur", carpeta: "Sur", ultima: "x", resultado: "ERROR: y", horario: cadaHora, tras_copia: true, proxima: "z" }),
  { tipo: "nube", nube: "Dropbox Sur", carpeta: "Sur", horario: cadaHora, tras_copia: true },
);
igual("uno de antes queda como antes", destinoParaOrden({ tipo: "carpeta", carpeta: "E:\\espejo", horario: null, tras_copia: false }), { tipo: "carpeta", carpeta: "E:\\espejo" });
igual("la hora para una consola anterior: la del primer horario", horaParaConsolasAnteriores([{ tipo: "carpeta", carpeta: "E:\\x", horario: cadaHora }], "02:00"), "08:00");
igual("…o la de antes si no hay ninguna", horaParaConsolasAnteriores([{ tipo: "carpeta", carpeta: "E:\\x" }], "02:00"), "02:00");

console.log(`\n${total - fallos} de ${total} comprobaciones correctas.`);
if (fallos) process.exit(1);
