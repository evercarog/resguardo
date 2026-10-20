// Pruebas de los errores de las copias en palabras (src/lib/errorCopia.ts, plan-mejoras 15oo):
// las mismas pruebas que pasa el agente (crates/protocolo/vectors/errores-copia.json), que
// ninguna frase diga «restic», que una frase ya traducida no se vuelva a traducir y que el
// detalle que manda el agente aparte gane al del mensaje. `npm run test:vectores`.
import { readFileSync } from "node:fs";
import { DESCONOCIDO, MARCA_DETALLE, enPalabras, fraseError, textoTecnico } from "../src/lib/errorCopia";
import { explicarError } from "../src/lib/copia";

let fallos = 0;
let total = 0;
function igual(nombre: string, obtenido: unknown, esperado: unknown) {
  total++;
  const ok = JSON.stringify(obtenido) === JSON.stringify(esperado);
  if (!ok) fallos++;
  console.log(`${ok ? "ok  " : "MAL "} ${nombre}${ok ? "" : `\n       obtenido: ${JSON.stringify(obtenido)}\n       esperado: ${JSON.stringify(esperado)}`}`);
}

const doc = JSON.parse(readFileSync(new URL("../../crates/protocolo/vectors/errores-copia.json", import.meta.url), "utf8")) as {
  reglas: { caso: string; frase: string; frase_linux?: string }[];
  pruebas: { que: string; entrada: string; caso: string; frase: string; detalle?: string }[];
};

console.log("\n· Vectores compartidos con el agente (crates/protocolo/vectors/errores-copia.json)");
for (const p of doc.pruebas) {
  const e = enPalabras(p.entrada);
  igual(`${p.que} (caso)`, e.caso, p.caso);
  igual(`${p.que} (frase)`, e.frase, p.frase);
  if (p.detalle !== undefined) igual(`${p.que} (detalle)`, e.detalle, p.detalle);
}

console.log("\n· Las frases");
for (const r of doc.reglas) {
  for (const f0 of [r.frase, r.frase_linux].filter((x): x is string => !!x)) {
    const f = f0.replace("{destino}", "");
    igual(`${r.caso}: sin «restic»`, /restic/i.test(f), false);
    igual(`${r.caso}: estable`, [enPalabras(f).frase, enPalabras(f).detalle], [f, null]);
  }
}
igual("desconocido sin «restic»", /restic/i.test(DESCONOCIDO), false);

console.log("\n· El detalle");
igual("el que manda el agente aparte gana", enPalabras("Se cortó.", "Fatal: dial tcp").detalle, "Fatal: dial tcp");
igual("una frase en una línea con su detalle", enPalabras(`${DESCONOCIDO} ${MARCA_DETALLE} algo`).detalle, "algo");
igual("las líneas de progreso no cuentan", textoTecnico('{"message_type":"status","percent_done":0.5}\nFatal: x'), "Fatal: x");
igual("sin repetir líneas", textoTecnico("Fatal: x\nFatal: x"), "Fatal: x");
igual("vacío", enPalabras(null), { caso: "en_palabras", frase: "", detalle: null });
igual("solo la frase", fraseError('Falló: {"message_type":"exit_error","code":1,"message":"Fatal: wrong password or no key found"}').startsWith("Contraseña incorrecta"), true);

console.log("\n· Con las explicaciones de siempre (explicarError)");
igual("sin conexión, desde el JSON crudo", explicarError('Falló: {"message_type":"exit_error","code":1,"message":"Fatal: unable to save snapshot: dial tcp 192.0.2.10:8002: connectex: A socket"').titulo, "No se pudo llegar al destino");

console.log(`\n${total - fallos} de ${total} bien`);
if (fallos) process.exit(1);
