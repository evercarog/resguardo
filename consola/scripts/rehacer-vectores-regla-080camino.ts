// Herramienta de esta rama (ia/080-camino): rehace «esperado» de los vectores de la
// regla 3-2-1-1-0 con la regla de la consola, para revisar el cambio a mano y
// comprobarlo después contra la de Rust (cargo test). No va en test:vectores.
import { readFileSync, writeFileSync } from "node:fs";
import { regla321, type EntradaRegla } from "../src/lib/regla321";
const ruta = new URL("../../crates/protocolo/vectors/regla-321.json", import.meta.url);
const doc = JSON.parse(readFileSync(ruta, "utf8"));
const ahora = Date.parse(doc.ahora);
let cambios = 0;
for (const v of doc.vectores) {
  const r = regla321(v.entrada as EntradaRegla, ahora);
  const esperado = { ...r, partes: r.partes.map(({ detalle: _d, ...p }) => p) };
  if (JSON.stringify(esperado) !== JSON.stringify(v.esperado)) {
    cambios++;
    console.log(`cambia: ${v.nombre}`);
    for (const p of esperado.partes) {
      const a = v.esperado.partes.find((x: { id: string }) => x.id === p.id);
      if (JSON.stringify(a) !== JSON.stringify(p)) console.log(`   ${p.id}: ${JSON.stringify(a)} -> ${JSON.stringify(p)}`);
    }
    if (JSON.stringify(v.esperado.avisos) !== JSON.stringify(esperado.avisos)) console.log(`   avisos: ${JSON.stringify(v.esperado.avisos)} -> ${JSON.stringify(esperado.avisos)}`);
    if (v.esperado.cumple !== esperado.cumple || v.esperado.cumple_config !== esperado.cumple_config) console.log(`   cumple ${v.esperado.cumple}/${v.esperado.cumple_config} -> ${esperado.cumple}/${esperado.cumple_config}`);
    v.esperado = esperado;
  }
}
if (process.argv.includes("--escribir")) writeFileSync(ruta, JSON.stringify(doc, null, 2) + "\n");
console.log(`${cambios} vectores cambian`);
