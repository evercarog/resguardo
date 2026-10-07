// Pruebas de los textos del equipo sin «[ruta]» a la vista (src/lib/textosEquipo.ts).
//
//   npm run test:vectores
import { porQueNoSeAplico } from "../src/lib/espera";
import { mensajeOrden, sinMarcadores, textoRestaurar, UNIDAD_ENTERA } from "../src/lib/textosEquipo";

let total = 0;
let fallos = 0;
function igual(nombre: string, real: unknown, esperado: unknown) {
  total++;
  const ok = JSON.stringify(real) === JSON.stringify(esperado);
  if (!ok) {
    fallos++;
    console.error(`✗ ${nombre}\n   real:     ${JSON.stringify(real)}\n   esperado: ${JSON.stringify(esperado)}`);
  } else console.log(`✓ ${nombre}`);
}

console.log("\n— Resultado de restaurar —");
igual("agente anterior, con la carpeta abajo", textoRestaurar("Restaurado (2 elementos) en [ruta] .", true), "Restaurado (2 elementos) en la carpeta de abajo.");
igual("agente anterior, sin carpeta abajo", textoRestaurar("Restaurado (2 elementos) en [ruta] ."), "Restaurado (2 elementos).");
igual("agente anterior, entre comillas", textoRestaurar("Restaurado (3 elementos) en «[ruta]», «[ruta]»."), "Restaurado (3 elementos).");
igual("agente nuevo", textoRestaurar("Restaurado (2 elementos).", true), "Restaurado (2 elementos) en la carpeta de abajo.");
igual("uno solo", textoRestaurar("Restaurado (1 elementos).", false), "Restaurado (1 elemento).");
igual("unidad entera, agente anterior", textoRestaurar("No se puede restaurar una unidad o la raíz entera: [ruta]"), UNIDAD_ENTERA);
igual("unidad entera, agente nuevo", textoRestaurar("No se puede restaurar una unidad entera de una vez. Elige las carpetas de dentro."), UNIDAD_ENTERA);
igual("otro error, sin cambios", textoRestaurar("Versión no válida."), "Versión no válida.");
igual("otro error con ruta en medio", textoRestaurar("«[ruta]» no está en las carpetas que copia este equipo: restáuralo junto al original."), "«…» no está en las carpetas que copia este equipo: restáuralo junto al original.");

console.log("\n— Cualquier mensaje del equipo —");
igual("sin marcador, igual", sinMarcadores("Copia hecha."), "Copia hecha.");
igual("dos puntos al final", sinMarcadores("No se encuentra la carpeta: [ruta]"), "No se encuentra la carpeta");
igual("dos puntos y punto", sinMarcadores("No se encuentra la carpeta: [ruta]."), "No se encuentra la carpeta.");
igual("en medio", sinMarcadores("open [ruta]: acceso denegado"), "open «…»: acceso denegado");
igual("entre comillas en medio", sinMarcadores("No se pudo leer «[ruta]» (acceso denegado)"), "No se pudo leer «…» (acceso denegado)");
igual("«en» que no es de ruta se queda", sinMarcadores("Falló en la copia: [ruta]"), "Falló en la copia");
igual(
  "en «Órdenes», la fallida de un agente anterior",
  porQueNoSeAplico({ tipo: "restaurar", estado: "fallida", mensaje: "No se puede restaurar una unidad o la raíz entera: [ruta]" }),
  `No se pudo aplicar: ${UNIDAD_ENTERA}`,
);
igual("mensajeOrden de otra orden", mensajeOrden("crear_repositorio", "No se encuentra la carpeta: [ruta]"), "No se encuentra la carpeta");
igual("nunca queda el marcador", sinMarcadores("a [ruta] b [ruta]: c en [ruta]").includes("[ruta]"), false);

console.log(`\n${total - fallos} de ${total} comprobaciones correctas.`);
if (fallos) process.exit(1);
