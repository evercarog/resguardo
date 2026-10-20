// Pruebas del bloque D del plan 0.8.1 («Las copias de cada día, más fáciles»; docs/plan-0.8.1.md):
// el nombre que se propone con las carpetas (15t), las etiquetas de las versiones (15v) y
// la retención «Personalizada» desde cero (15z). `npm run test:vectores`.
import type { Configuracion } from "../src/lib/tipos";
import { copiaNueva, etiquetasEnUso, limpiarEtiquetaVersion, nombrePropuesto, ponerCarpetas } from "../src/lib/copiaGuiada";
import { configParaEnviar, type Admite } from "../src/lib/configEnvio";
import { reglaSinNada, reglaVacia, sinPeriodo } from "../src/lib/retencion";

let fallos = 0;
let total = 0;
function igual(nombre: string, obtenido: unknown, esperado: unknown) {
  total++;
  const ok = JSON.stringify(obtenido) === JSON.stringify(esperado);
  if (!ok) fallos++;
  console.log(`${ok ? "ok  " : "MAL "} ${nombre}${ok ? "" : `\n       obtenido: ${JSON.stringify(obtenido)}\n       esperado: ${JSON.stringify(esperado)}`}`);
}

// --- 15t: el nombre se propone con las carpetas mientras nadie lo cambie a mano ---
{
  igual("una carpeta", nombrePropuesto(["C:\\Users\\Ana\\Documents"], "PC-01"), "Documents de PC-01");
  igual("dos carpetas", nombrePropuesto(["C:\\Users\\Ana\\Documents", "D:\\Fotos\\"], "PC-01"), "Documents y Fotos de PC-01");
  igual("tres o más", nombrePropuesto(["/home/ana/docs", "/srv/a", "/srv/b"], "SRV-01"), "docs y 2 más de SRV-01");
  igual("una unidad entera", nombrePropuesto(["D:\\"], "PC-01"), "D: de PC-01");
  igual("sin carpetas, la de una copia nueva", nombrePropuesto([], "PC-01"), "Copia nueva");
  igual("nunca más de 60", [...nombrePropuesto(["C:\\" + "x".repeat(90)], "PC-01")].length <= 60, true);
  const k = copiaNueva("r1", "k1");
  ponerCarpetas(k, ["C:\\Users\\Ana\\Documents"], "PC-01");
  igual("la nueva toma la propuesta", k.nombre, "Documents de PC-01");
  ponerCarpetas(k, ["C:\\Users\\Ana\\Documents", "C:\\Users\\Ana\\Desktop"], "PC-01");
  igual("y la sigue mientras sea la propuesta", k.nombre, "Documents y Desktop de PC-01");
  k.nombre = "Contabilidad";
  ponerCarpetas(k, ["C:\\Conta"], "PC-01");
  igual("una puesta a mano no se toca", [k.nombre, k.carpetas], ["Contabilidad", ["C:\\Conta"]]);
}

// --- 15v: las etiquetas de las versiones ---
{
  igual("espacios a guiones", limpiarEtiquetaVersion(" bases  sql "), { etiqueta: "bases-sql" });
  igual("sin comas", limpiarEtiquetaVersion("a,b"), { etiqueta: "ab" });
  igual("vacía, no", "error" in limpiarEtiquetaVersion("  "), true);
  igual("de más de 40, no", "error" in limpiarEtiquetaVersion("x".repeat(41)), true);
  const eq = (ets: string[][]) => ({ resumen: { copias: ets.map((e) => ({ etiquetas: e })) } });
  igual("en uso, la más usada primero", etiquetasEnUso([eq([["bases"], ["binarios", "bases"]]), eq([["binarios"], ["bases"]])]), ["bases", "binarios"]);
  const a: Admite = { reglas: true, ganchos: true, soloCambios: true, verif: false, verifHorario: false, escritorio: false, etiquetas: true };
  const cfg = (et?: string[]) => ({ v: 1, copias: [{ ...copiaNueva("r1", "k1"), carpetas: ["C:\\D"], ...(et ? { etiquetas: et } : {}) }], repositorios: [], destinos: [] }) as unknown as Configuracion;
  igual("se mandan si el agente las entiende", (configParaEnviar(cfg(["bases"]), a).copias[0] as { etiquetas?: string[] }).etiquetas, ["bases"]);
  igual("sin ninguna, no va el campo", "etiquetas" in configParaEnviar(cfg([]), a).copias[0], false);
  igual("a un agente anterior, no", "etiquetas" in configParaEnviar(cfg(["bases"]), { ...a, etiquetas: false }).copias[0], false);
}

// --- 15z: «Personalizar» desde cero, «Vaciar» y la «×» de cada fila ---
{
  igual("vacía no guarda nada", reglaSinNada(reglaVacia()), true);
  const r = { diarias: 7, semanales: 0, mensuales: 0, anuales: 0, plazos: { horarias: "15d", diarias: "1y" } };
  igual("quitar las diarias deja el plazo de las horarias", sinPeriodo(r, "diarias"), { diarias: 0, semanales: 0, mensuales: 0, anuales: 0, plazos: { horarias: "15d" } });
  igual("quitar todas deja la regla vacía", reglaSinNada(sinPeriodo(sinPeriodo(r, "diarias"), "horarias")), true);
  igual("sin tocar la original", r.plazos, { horarias: "15d", diarias: "1y" });
}

console.log(`\n${total - fallos}/${total} ${fallos ? "con fallos" : "bien"}`);
if (fallos) process.exit(1);
