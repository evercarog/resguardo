// Pruebas de los trabajos de espejo (src/lib/espejoTrabajos.ts; plan 0.7.26, bloque 4).
// 1. Los vectores compartidos con el agente (crates/protocolo/vectors/espejo-trabajos.json).
// 2. Lo que se manda, los textos, el orden y los espejos de una copia agrupados por almacén.
// `npm run test:vectores` (con las demás).
import { readFileSync } from "node:fs";
import {
  AVISO_IGUAL,
  conTrabajo,
  cuerpoAlmacen,
  cuerpoEquipo,
  errorTrabajo,
  errorTrabajos,
  espejosDeCopia,
  estadoTrabajo,
  hrefEspejos,
  mover,
  nombrePorDefecto,
  paraOrden,
  reduceTrabajos,
  sinTrabajo,
  textoCuando,
  textoQue,
  textoRetencion,
  trabajoIncluye,
  trabajoNuevo,
  type TrabajoEspejo,
} from "../src/lib/espejoTrabajos";
import { esDestructiva } from "../src/lib/cripto/ordenes";
import type { Equipo } from "../src/lib/tipos";

let fallos = 0;
let total = 0;
function igual(nombre: string, obtenido: unknown, esperado: unknown) {
  total++;
  const ok = JSON.stringify(obtenido) === JSON.stringify(esperado);
  if (!ok) fallos++;
  console.log(`${ok ? "ok  " : "MAL "} ${nombre}${ok ? "" : `\n       obtenido: ${JSON.stringify(obtenido)}\n       esperado: ${JSON.stringify(esperado)}`}`);
}

console.log("\n· Vectores compartidos con el agente (crates/protocolo/vectors/espejo-trabajos.json)");
const doc = JSON.parse(readFileSync(new URL("../../crates/protocolo/vectors/espejo-trabajos.json", import.meta.url), "utf8")) as {
  base: TrabajoEspejo;
  pedidos: { nombre: string; quien: "almacen" | "equipo"; cambios: Partial<TrabajoEspejo>[]; ok: boolean }[];
  reduce: { nombre: string; antes: Partial<TrabajoEspejo>[]; ahora: Partial<TrabajoEspejo>[]; reduce: boolean }[];
};
const con = (c: Partial<TrabajoEspejo>): TrabajoEspejo => ({ ...structuredClone(doc.base), ...structuredClone(c) }) as TrabajoEspejo;
for (const p of doc.pedidos) igual(`pedido: ${p.nombre}`, errorTrabajos(p.cambios.map(con), p.quien) === null, p.ok);
for (const c of doc.reduce) igual(`reduce: ${c.nombre}`, reduceTrabajos(c.antes.map(con), c.ahora.map(con)), c.reduce);
// La orden sellada pone la espera con lo mismo.
const base = con({});
igual("la orden al almacén espera si reduce", esDestructiva("guarda_copias", cuerpoAlmacen([con({ activo: false })]), undefined, { espejo: { trabajos: [base] } }), true);
igual("…y no si solo añade", esDestructiva("guarda_copias", cuerpoAlmacen([base, con({ id: "t2", adonde: { tipo: "carpeta", carpeta: "F:\\x" } })]), undefined, { espejo: { trabajos: [base] } }), false);
igual("quitarlos todos del almacén espera", esDestructiva("guarda_copias", cuerpoAlmacen([]), undefined, { espejo: { trabajos: [base] } }), true);
const delEquipo = con({ quien: "equipo" });
igual("los del equipo: quitarlos espera", esDestructiva("guarda_copias", cuerpoEquipo([]), undefined, { espejoEquipo: { trabajos: [delEquipo] } }), true);
igual("…cambiar la hora no", esDestructiva("guarda_copias", cuerpoEquipo([{ ...delEquipo, cuando: { tras_copia: true } }]), undefined, { espejoEquipo: { trabajos: [delEquipo] } }), false);
igual("…«igual que el origen», sí", esDestructiva("guarda_copias", cuerpoEquipo([{ ...delEquipo, retencion: { modo: "igual" } }]), undefined, { espejoEquipo: { trabajos: [delEquipo] } }), true);
igual("confirmar el freno, siempre", esDestructiva("guarda_copias", { espejo_freno: { trabajo: "t1" } }), true);

console.log("\n· Lo que se manda");
const resumen = { ...base, ultima: "2026-10-07T02:00:00+02:00", resultado: "ERROR: x", proxima: "y", freno_aviso: "z", por_borrar: { archivos: 1, bytes: 2 } };
igual("sin lo que recuerda el equipo", Object.keys(paraOrden(resumen as TrabajoEspejo)).sort(), ["activo", "adonde", "cuando", "freno", "id", "nombre", "orden", "que", "quien", "retencion"]);
igual("la zona principal no se manda", "zona" in paraOrden(con({ zona: "principal" })), false);
igual("la carpeta de una nube, sin barras", paraOrden(con({ adonde: { tipo: "nube", nube: " Dropbox ", carpeta: "/Resguardo/Sur/" } })).adonde, { tipo: "nube", nube: "Dropbox", carpeta: "Resguardo/Sur" });
igual("un horario vacío no se manda", paraOrden(con({ cuando: { horario: { dias: [], horas: [] }, tras_copia: true } })).cuando, { tras_copia: true });
igual("en cadena manda sobre «después de»", paraOrden(con({ cuando: { cadena: "a", despues: "b" } })).cuando, { cadena: "a" });
igual("sin trabajos: quitar el espejo", cuerpoAlmacen([]), { espejo: null });
igual("…y los del equipo", cuerpoEquipo([]), { espejo_equipo: null });
const nuevo = trabajoNuevo("almacen", 3);
igual("uno nuevo: después de cada copia y cada noche, nunca borra, freno al 10 %", [nuevo.cuando.tras_copia, nuevo.retencion.modo, nuevo.freno.pct, nuevo.orden, /^t[0-9a-f]{10}$/.test(nuevo.id)], [true, "nunca", 10, 3, true]);
igual("uno nuevo sin carpeta no vale", errorTrabajo(nuevo, "almacen"), "Falta la carpeta.");

console.log("\n· Orden y cadenas");
const a = con({ id: "a", orden: 0 });
const b = con({ id: "b", orden: 1, adonde: { tipo: "carpeta", carpeta: "F:\\b" }, cuando: { cadena: "a" } });
const c = con({ id: "c", orden: 2, adonde: { tipo: "carpeta", carpeta: "G:\\c" } });
igual("subir uno", mover([a, b, c], "c", -1).map((t) => `${t.id}${t.orden}`), ["a0", "c1", "b2"]);
igual("bajar el último no hace nada", mover([a, b, c], "c", 1).map((t) => t.id), ["a", "b", "c"]);
igual("añadir: al final", conTrabajo([a, b], c).map((t) => `${t.id}${t.orden}`), ["a0", "b1", "c2"]);
igual("cambiar uno: en su sitio", conTrabajo([a, b, c], { ...b, nombre: "Otro" }).map((t) => t.nombre), [a.nombre, "Otro", c.nombre]);
const sinA = sinTrabajo([a, b, c], "a");
igual("quitar uno: lo que iba en cadena tras él empieza después de cada copia", [sinA.map((t) => t.id), sinA[0].cuando.cadena, sinA[0].cuando.tras_copia], [["b", "c"], null, true]);
igual("…y sigue siendo válido", errorTrabajos(sinA, "almacen"), null);

console.log("\n· Textos");
igual("todos", textoQue({ que: { tipo: "todos" }, quien: "almacen" }), "Todos los repositorios");
igual("los del equipo", textoQue({ que: { tipo: "todos" }, quien: "equipo" }), "Todos sus repositorios");
igual("de un equipo", textoQue({ que: { tipo: "equipos", equipos: ["caja-1"] }, quien: "almacen" }, undefined, (u) => (u === "caja-1" ? "Caja" : u)), "Los de Caja");
igual("varios repositorios", textoQue({ que: { tipo: "repos", repos: ["a/x", "b/y"] }, quien: "almacen" }), "2 repositorios: a/x y b/y");
igual("en cadena con retraso", textoCuando({ cuando: { cadena: "a", retraso_min: 10 } }, () => "Disco E"), "En cadena tras «Disco E» (10 min después)");
igual("después de cada copia y cada noche", textoCuando({ cuando: { tras_copia: true, horario: { dias: [1, 2, 3, 4, 5, 6, 7], horas: ["02:00"] } } }).startsWith("Después de cada copia nueva · "), true);
igual("retenciones", [textoRetencion({ retencion: { modo: "nunca" } }), textoRetencion({ retencion: { modo: "retraso", dias: 30 } }), textoRetencion({ retencion: { modo: "igual" } }), textoRetencion({ retencion: { modo: "nunca" }, bloqueo: true })], [
  "Nunca borra",
  "Borra lo quitado a los 30 días",
  "Igual que el origen",
  "Nunca borra (bloqueo de objetos)",
]);
igual("el aviso de «igual que el origen»", AVISO_IGUAL, "Si algo borra en el original, aquí también.");
igual("nombre por defecto", [nombrePorDefecto({ tipo: "nube", nube: "Dropbox", carpeta: "x" }), nombrePorDefecto({ tipo: "zona", carpeta: "principal" })], ["Espejo a «Dropbox»", "Espejo a la zona principal"]);
igual("estados", [estadoTrabajo({ ...a, activo: false }), estadoTrabajo(a), estadoTrabajo({ ...a, resultado: "ERROR: no" }), estadoTrabajo({ ...a, resultado: "Espejo hecho" })], ["pausado", "nuevo", "error", "ok"]);

console.log("\n· Los espejos de una copia, por almacén");
igual("todos: entra", trabajoIncluye({ que: { tipo: "todos" }, quien: "almacen" }, "caja-1/caja"), true);
igual("de su equipo: entra", trabajoIncluye({ que: { tipo: "equipos", equipos: ["caja-1"] }, quien: "almacen" }, "caja-1/caja"), true);
igual("de otro equipo: no", trabajoIncluye({ que: { tipo: "equipos", equipos: ["caja-10"] }, quien: "almacen" }, "caja-1/caja"), false);
igual("de otra zona: no", trabajoIncluye({ que: { tipo: "todos" }, quien: "almacen", zona: "z0e0e0e" }, "caja-1/caja"), false);
const almacen = {
  id: "alm",
  nombre: "Servidor",
  resumen: {
    admite: ["espejo_trabajos"],
    guarda_copias: {
      activo: true,
      espejo: {
        hora: "02:00",
        trabajos: [
          { ...con({ id: "n", nombre: "Nube", orden: 1, adonde: { tipo: "nube", nube: "Dropbox", carpeta: "x" } }) },
          { ...con({ id: "e", nombre: "Disco E", orden: 0 }) },
          { ...con({ id: "o", nombre: "Otro equipo", orden: 2, que: { tipo: "equipos", equipos: ["srv"] }, adonde: { tipo: "carpeta", carpeta: "G:\\x" } }) },
        ],
      },
    },
  },
} as unknown as Equipo;
const caja = {
  id: "caja",
  nombre: "Caja",
  resumen: {
    destinos: [{ id: "d-alm", nombre: "Servidor", tipo: "rest", donde: "https://servidor:8000/caja-1/", equipo_almacen: "alm" }],
    repositorios: [{ id: "caja", nombre: "Caja", destino: "d-alm" }],
    espejo_equipo: { trabajos: [{ ...con({ id: "p", nombre: "USB", quien: "equipo", que: { tipo: "repos", repos: ["caja"] } }) }] },
  },
} as unknown as Equipo;
const repoCaja = caja.resumen!.repositorios![0];
const grupos = espejosDeCopia(caja, repoCaja, [almacen, caja]);
igual("agrupados: el almacén y el propio equipo", grupos.map((g) => [g.nombre, g.quien, g.nombreRepo, g.espejos.map((x) => x.trabajo.nombre)]), [
  ["Servidor", "almacen", "caja-1/caja", ["Disco E", "Nube"]],
  ["Caja", "equipo", "caja", ["USB"]],
]);
const fuera = espejosDeCopia({ ...caja, resumen: { ...caja.resumen!, espejo_equipo: null, consolas: [{ nombre: "Consola en línea", esta: false }] } } as unknown as Equipo, repoCaja, [caja]);
igual("un almacén que no está en esta consola", fuera.map((g) => [g.nombre, g.fuera, g.consolas, g.espejos.length]), [["Servidor", true, ["Consola en línea"], 0]]);
igual("el enlace para añadir uno desde la copia", hrefEspejos("c1", "alm", { repo: "caja-1/caja", zona: "principal", quien: "almacen" }), "/c/c1/equipos/alm/espejos?nuevo=1&repo=caja-1%2Fcaja&quien=almacen");

console.log(`\n${total - fallos}/${total} bien`);
if (fallos) process.exit(1);
