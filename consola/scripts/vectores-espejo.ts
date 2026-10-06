// Pruebas del espejo por destino (src/lib/espejo.ts, docs/espejo.md).
// `npm run test:vectores` (con las demás).
import { diaLegible, errorDiasRetencion, textoRetencion, textoVerificacion } from "../src/lib/espejo";
import { admiteEspejoFlexible, conRepos, cuandoEspejo, destinoParaOrden, espejoDelRepo, horaParaConsolasAnteriores, horarioDiario, nombreEnAlmacen, nombresRepos, nuevosEn, textoRepos } from "../src/lib/espejo";
import { esDestructiva } from "../src/lib/cripto/ordenes";

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

console.log("\n· Espejo: qué repositorios (3f)");
const todos = nombresRepos([{ usuario: "caja-1", repos: ["caja", "siigo"] }, { usuario: "srv", repos: ["."] }, { usuario: "raro", repos: ["a/b"] }]);
igual("los nombres del almacén: <usuario>/<repo> o <usuario>", todos, ["caja-1/caja", "caja-1/siigo", "srv"]);
igual("todos: no hay nuevos", nuevosEn({}, todos), []);
igual("con selección: los que no se vieron", nuevosEn({ repos: ["caja-1/caja"], vistos: ["caja-1/caja", "caja-1/siigo"] }, todos), ["srv"]);
igual("textos", [textoRepos({}), textoRepos({ repos: ["srv"] }), textoRepos({ repos: ["a", "b"] }, (r) => r.toUpperCase())], ["Todos los repositorios", "Solo srv", "2 repositorios: A, B"]);
igual("añadir los nuevos", conRepos({ tipo: "carpeta", carpeta: "E:\\x", repos: ["srv"], vistos: ["srv"] }, ["caja-1/caja"], todos), { tipo: "carpeta", carpeta: "E:\\x", repos: ["srv", "caja-1/caja"], vistos: todos });
igual("la selección se reenvía", destinoParaOrden({ tipo: "carpeta", carpeta: "E:\\x", repos: ["srv"], vistos: ["srv", "caja-1/caja"] }), { tipo: "carpeta", carpeta: "E:\\x", repos: ["srv"], vistos: ["srv", "caja-1/caja"] });
const ctx = (repos?: string[]) => ({ espejo: { destinos: [{ tipo: "carpeta" as const, carpeta: "E:\\x", repos }] } });
const orden = (repos?: string[]) => ({ espejo: { destinos: [{ tipo: "carpeta", carpeta: "E:\\x", repos }], hora: "02:00" } });
igual("pasar de todos a algunos espera", esDestructiva("guarda_copias", orden(["a"]), undefined, ctx(undefined)), true);
igual("quitar uno de la selección espera", esDestructiva("guarda_copias", orden(["a"]), undefined, ctx(["a", "b"])), true);
igual("añadir uno no espera", esDestructiva("guarda_copias", orden(["a", "b"]), undefined, ctx(["a"])), false);
igual("pasar a todos no espera", esDestructiva("guarda_copias", orden(undefined), undefined, ctx(["a"])), false);
igual("el nombre de un repositorio en su almacén", [nombreEnAlmacen("rest:https://10.0.0.5:8000/caja-1/", { id: "caja" }), nombreEnAlmacen("https://almacen:8000/srv", { id: "x", ruta: "." }), nombreEnAlmacen("E:\\copias", { id: "x" })], ["caja-1/caja", "srv", null]);
const esp = { hora: "02:00", destinos: [{ tipo: "carpeta" as const, carpeta: "E:\\x" }, { tipo: "nube" as const, nube: "B2", carpeta: "y", repos: ["caja-1/siigo"] }] };
igual("desde un repositorio, solo los destinos a los que va", espejoDelRepo(esp, "caja-1/caja")?.destinos?.length, 1);
igual("…y los dos si entra en la selección", espejoDelRepo(esp, "caja-1/siigo")?.destinos?.length, 2);
igual("…y ninguno si no va a ninguno", espejoDelRepo({ hora: "02:00", destinos: [esp.destinos[1]] }, "caja-1/caja"), null);

console.log("\n· Espejo: comprobar (3d)");
igual("sin comprobar, nada", textoVerificacion({ verificar_pct: 0 }), null);
igual("sin hacerla aún", textoVerificacion({ verificar_pct: 5 }), "Comprueba el 5 % cada día");
igual("hecha y bien", textoVerificacion({ verificar_pct: 100, verificacion: { ultima: "x", archivos: 1, mal: 0 } }), "Lo comprueba todo cada día · 1 archivo la última vez, bien");
igual("con alguno mal", textoVerificacion({ verificar_pct: 5, verificacion: { ultima: "x", archivos: 40, mal: 2 } }), "Comprueba el 5 % cada día · 40 archivos la última vez, 2 mal");
igual("el % se reenvía", destinoParaOrden({ tipo: "carpeta", carpeta: "E:\\x", verificar_pct: 10, verificacion: { ultima: "x", archivos: 1, mal: 0 } }), { tipo: "carpeta", carpeta: "E:\\x", verificar_pct: 10 });

console.log("\n· Espejo: retención (3b)");
igual("textos", [textoRetencion({}), textoRetencion({ retencion_dias: 30 }), textoRetencion({ bloqueo: true, retencion_dias: 30 })], ["Nunca borra", "Borra lo que ya no está en el almacén a los 30 días", "Con bloqueo de objetos: nunca borra"]);
igual("días válidos", [errorDiasRetencion(30), errorDiasRetencion(7), errorDiasRetencion(6), errorDiasRetencion(3651), errorDiasRetencion(7.5)].map((x) => x === null), [true, true, false, false, false]);
igual("un día, sin saltos de zona", diaLegible("2026-11-01"), "1 de noviembre de 2026");
igual("la retención se reenvía", destinoParaOrden({ tipo: "carpeta", carpeta: "E:\\x", retencion_dias: 30, por_borrar: { archivos: 1, bytes: 1 }, freno: "x" }), { tipo: "carpeta", carpeta: "E:\\x", retencion_dias: 30 });
igual("…y el bloqueo (sin retención)", destinoParaOrden({ tipo: "carpeta", carpeta: "E:\\x", bloqueo: true, retencion_dias: 30 }), { tipo: "carpeta", carpeta: "E:\\x", bloqueo: true });
const ctxR = (x: object) => ({ espejo: { destinos: [{ tipo: "carpeta" as const, carpeta: "E:\\x", ...x }] } });
const ordenR = (x: object) => ({ espejo: { destinos: [{ tipo: "carpeta", carpeta: "E:\\x", ...x }], hora: "02:00" } });
igual("poner retención espera", esDestructiva("guarda_copias", ordenR({ retencion_dias: 30 }), undefined, ctxR({})), true);
igual("acortarla espera", esDestructiva("guarda_copias", ordenR({ retencion_dias: 10 }), undefined, ctxR({ retencion_dias: 30 })), true);
igual("alargarla no", esDestructiva("guarda_copias", ordenR({ retencion_dias: 60 }), undefined, ctxR({ retencion_dias: 30 })), false);
igual("quitarla no", esDestructiva("guarda_copias", ordenR({}), undefined, ctxR({ retencion_dias: 30 })), false);
igual("quitar el bloqueo espera", esDestructiva("guarda_copias", ordenR({}), undefined, ctxR({ bloqueo: true })), true);
igual("confirmar el freno espera", esDestructiva("guarda_copias", { espejo_freno: { tipo: "carpeta", carpeta: "E:\\x" } }, undefined, ctxR({})), true);

console.log(`\n${total - fallos} de ${total} comprobaciones correctas.`);
if (fallos) process.exit(1);
