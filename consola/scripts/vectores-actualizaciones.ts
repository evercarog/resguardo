// Pruebas de la actualización automática en la consola (docs/actualizaciones.md):
// 1. Las versiones, con los vectores compartidos con Rust (crates/protocolo/vectors/publicacion.json).
// 2. Las firmas de minisign en JavaScript (scripts/lib/minisign.mjs) con los mismos vectores
//    que comprueba el protocolo en Rust, y una firma de minisign de verdad.
// 3. El estado de cada equipo en palabras (lib/actualizaciones.ts) y la política.
// `npm run test:vectores` (con las demás).
import { readFileSync } from "node:fs";
import { compararVersiones, frasePolitica, horaValida, leerVersion, vistaActualizacion } from "../src/lib/actualizaciones";
// @ts-expect-error: módulo de Node en JavaScript, sin tipos.
import { comprobarFirma, leerLlaves } from "../../scripts/lib/minisign.mjs";

let fallos = 0;
let total = 0;
function igual(nombre: string, obtenido: unknown, esperado: unknown) {
  total++;
  const ok = JSON.stringify(obtenido) === JSON.stringify(esperado);
  if (!ok) fallos++;
  console.log(`${ok ? "ok  " : "MAL "} ${nombre}${ok ? "" : `\n       obtenido: ${JSON.stringify(obtenido)}\n       esperado: ${JSON.stringify(esperado)}`}`);
}

const doc = JSON.parse(readFileSync(new URL("../../crates/protocolo/vectors/publicacion.json", import.meta.url), "utf8")) as {
  versiones: [string, string, number][];
  versiones_no_validas: string[];
  firmas: { nombre: string; manifiesto: string; firma: string; valida: boolean; llave?: string; revocadas?: string[] }[];
  firma_minisign_real: { llave_pub: string; datos: string; firma: string; llave: string };
};

console.log("\n· Versiones (las mismas que en Rust)");
for (const [a, b, esperado] of doc.versiones) igual(`${a} frente a ${b}`, compararVersiones(a, b), esperado);
for (const v of doc.versiones_no_validas) igual(`«${v}» no es una versión`, leerVersion(v), null);
igual("con una que no es versión, no se compara", compararVersiones("0.7.1", "x"), null);

console.log("\n· Firmas (las mismas que comprueba el protocolo en Rust)");
const llaves = leerLlaves(readFileSync(new URL("../../crates/protocolo/tests/fixtures/llave-pruebas-a.pub", import.meta.url), "utf8"));
igual("la llave de pruebas A", llaves.map((k: { id: string }) => k.id), ["C57E2BA1129956D5"]);
for (const c of doc.firmas) {
  const r = comprobarFirma(Buffer.from(c.manifiesto), c.firma, llaves, c.revocadas ?? []);
  igual(`${c.nombre}`, r.error === null, c.valida);
  if (c.valida && c.llave) igual(`${c.nombre}: la llave`, r.id, c.llave);
}
const real = doc.firma_minisign_real;
const r = comprobarFirma(Buffer.from(real.datos), real.firma, leerLlaves(real.llave_pub));
igual("una firma de minisign de verdad", [r.error, r.id], [null, real.llave]);
igual("la misma, con otros datos, no", comprobarFirma(Buffer.from("Test"), real.firma, leerLlaves(real.llave_pub)).error !== null, true);

console.log("\n· El estado de cada equipo");
const v = (info: unknown, version: string, disponible: string | null) => {
  const x = vistaActualizacion(info as never, version, disponible);
  return [x.clave, x.texto, x.tono];
};
igual("agente anterior, versión vieja: a mano", v(undefined, "0.7.24", "0.7.25"), ["anterior", "Versión anterior", "warn"]);
igual("agente anterior al día", v(undefined, "0.7.25", "0.7.25"), ["sin_datos", "Sin datos", "neutral"]);
igual("al día", v({ estado: "al_dia" }, "0.7.25", "0.7.25"), ["al_dia", "Al día", "ok"]);
igual("al día pero esta consola tiene otra más nueva: pendiente", v({ estado: "al_dia" }, "0.7.24", "0.7.25"), ["pendiente", "Pendiente", "info"]);
igual("actualizada sola", v({ estado: "actualizada", version_objetivo: "0.7.25" }, "0.7.25", "0.7.25"), ["al_dia", "Al día", "ok"]);
igual("espera su anillo", v({ estado: "pendiente", motivo: "espera_anillo", hasta: "2026-10-22T10:00:00-05:00" }, "0.7.24", "0.7.25"), ["pendiente", "Pendiente", "info"]);
igual("retenida: atención", v({ estado: "pendiente", motivo: "retenida" }, "0.7.24", "0.7.25"), ["pendiente", "Pendiente", "warn"]);
igual("almacén sin ventana: atención", v({ estado: "pendiente", motivo: "almacen_sin_ventana" }, "0.7.24", "0.7.25"), ["pendiente", "Pendiente", "warn"]);
igual("actualizando", v({ estado: "actualizando", version_objetivo: "0.7.25" }, "0.7.24", "0.7.25"), ["actualizando", "Actualizando", "info"]);
igual("bajándola", v({ estado: "descargando", version_objetivo: "0.7.25" }, "0.7.24", "0.7.25"), ["actualizando", "Actualizando", "info"]);
igual("volvió atrás", v({ estado: "vuelta_atras", version_fallida: "0.7.25", mensaje: "No estuvo sana." }, "0.7.24", "0.7.25"), ["vuelta_atras", "Volvió a la anterior", "bad"]);
igual("falló", v({ estado: "fallida", mensaje: "Sin espacio." }, "0.7.24", "0.7.25"), ["fallo", "Falló", "bad"]);
igual("en pausa", v({ estado: "pausada" }, "0.7.24", "0.7.25"), ["pausada", "En pausa", "paused"]);
igual("compilado sin llave", v({ estado: "desactivada", motivo: "sin_actualizaciones" }, "0.7.24", "0.7.25"), ["desactivada", "Sin actualización automática", "neutral"]);
const detalle = vistaActualizacion({ estado: "vuelta_atras", version_fallida: "0.7.25", mensaje: "No estuvo sana en 10 minutos." }, "0.7.24", null).detalle ?? "";
igual("el detalle dice la versión y el motivo", detalle.includes("0.7.25") && detalle.includes("10 minutos"), true);
const espera = vistaActualizacion({ estado: "pendiente", motivo: "espera_anillo", hasta: "2026-10-22T10:00:00-05:00" }, "0.7.24", null).detalle ?? "";
igual("el detalle de la espera dice hasta cuándo", espera.includes("anillo general") && espera.includes("Hasta el 22/10"), true);

console.log("\n· La política");
igual("automática", frasePolitica({ modo: "auto", dias_general: 2, ventana: null }), "Automática: los equipos de prueba en cuanto sale; los demás, 2 días después.");
igual("automática con ventana", frasePolitica({ modo: "auto", dias_general: 1, ventana: { desde: "22:00", hasta: "06:00" } }), "Automática: los equipos de prueba en cuanto sale; los demás, 1 día después, entre las 22:00 y las 06:00.");
igual("solo cuando apruebe", frasePolitica({ modo: "manual", dias_general: 2, ventana: null }), "Solo cuando la apruebes con «Actualizar ahora».");
igual("en pausa", frasePolitica({ modo: "pausada", dias_general: 2, ventana: null }), "En pausa: ningún equipo se actualiza.");
igual("horas de la ventana", ["22:00", "06:00", "24:00", "7:00", "22:60"].map(horaValida), [true, true, false, false, false]);

console.log(`\n${fallos ? "MAL" : "bien"}  ${total - fallos}/${total} vectores de las actualizaciones`);
if (fallos) process.exit(1);
