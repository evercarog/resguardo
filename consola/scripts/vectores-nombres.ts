// Pruebas de los nombres legibles en el almacén (src/lib/nombres.ts, docs/nombres-legibles.md):
// la misma regla que el agente (vectores compartidos), la carpeta de un repositorio
// nuevo, qué se ordena, las órdenes para el almacén y el equipo, las rutas que el
// equipo aún no usa y los nombres que se ven. `npm run test:vectores` (con las demás).
import { readFileSync } from "node:fs";
import type { Equipo, TrabajoEspejoResumen } from "../src/lib/tipos";
import {
  carpetaLegible,
  carpetaNueva,
  carpetaValida,
  cuerpoMover,
  cuerpoRutaNueva,
  dondeConUsuario,
  equiposEnAlmacen,
  espejosQueLoCopian,
  hayQueOrdenar,
  legibleEnTexto,
  mapaNombres,
  normalizar,
  planOrden,
  rutasPendientes,
  textoCarpetaVieja,
  usuarioDeAntes,
} from "../src/lib/nombres";
import { NIVEL, SOLO_ADMIN_ROL } from "../src/lib/cripto/ordenes";

let fallos = 0;
let total = 0;
function igual(nombre: string, obtenido: unknown, esperado: unknown) {
  total++;
  const ok = JSON.stringify(obtenido) === JSON.stringify(esperado);
  if (!ok) fallos++;
  console.log(`${ok ? "ok  " : "MAL "} ${nombre}${ok ? "" : `\n       obtenido: ${JSON.stringify(obtenido)}\n       esperado: ${JSON.stringify(esperado)}`}`);
}

console.log("\n· Vectores compartidos con el agente (crates/protocolo/vectors/nombres-legibles.json)");
{
  const doc = JSON.parse(readFileSync(new URL("../../crates/protocolo/vectors/nombres-legibles.json", import.meta.url), "utf8")) as {
    normalizar: { entrada: string; max: number; defecto: string; salida: string }[];
    validas: { carpeta: string; valida: boolean }[];
  };
  for (const v of doc.normalizar) igual(`normalizar ${JSON.stringify(v.entrada)}`, normalizar(v.entrada, v.max, v.defecto), v.salida);
  for (const v of doc.validas) igual(`válida ${JSON.stringify(v.carpeta)}`, carpetaValida(v.carpeta), v.valida);
}

console.log("\n· La carpeta de un repositorio nuevo en un almacén");
igual("su nombre", carpetaNueva("Contabilidad", []), "contabilidad");
igual("sin chocar (sin distinguir mayúsculas)", carpetaNueva("Contabilidad", ["Contabilidad", "contabilidad-2"]), "contabilidad-3");
igual("sin nombre aprovechable", carpetaNueva("日本", []), "repositorio");
igual("largo: se recorta y el número cabe", carpetaNueva("x".repeat(60), ["x".repeat(40)]), `${"x".repeat(38)}-2`);
igual("legible: su nombre o con número", [carpetaLegible("contabilidad", "Contabilidad"), carpetaLegible("contabilidad-2", "Contabilidad"), carpetaLegible("almacen-01-3f2a", "Contabilidad")], [true, true, false]);
igual("usuario de antes (el id recortado)", [usuarioDeAntes("0b5c1f8e-1d2a-4c3b-9e8f-7a6b5c4d"), usuarioDeAntes("pc-01-3f2a"), usuarioDeAntes("recepcion")], [true, false, false]);

// Un almacén y dos equipos: PC-01 (usuario de antes, dos repositorios) y CAJA (ya legible).
const base = { so: "Windows 11", version_agente: "0.7.27", box_pub: "", sign_pub: "", sal_equipo: "", etiqueta: null, modo: "gestionado" as const, confirmado: true, conectado: true, ultimo_contacto: null, estado_servicio: "en_marcha" as const, siguiente_seq: 1 };
const U = "0b5c1f8e-1d2a-4c3b-9e8f-7a6b5c4d";
const trabajo = (id: string, que: TrabajoEspejoResumen["que"], retencion: TrabajoEspejoResumen["retencion"] = { modo: "nunca" }): TrabajoEspejoResumen => ({
  id,
  nombre: `Espejo ${id}`,
  activo: true,
  quien: "almacen",
  que,
  adonde: { tipo: "nube", carpeta: "copias", nube: "Backblaze" },
  cuando: {},
  retencion,
  freno: { pct: 10, min_archivos: 100, min_faltan: 20, accion: "confirmar" },
  orden: 0,
});
const alm: Equipo = {
  ...base,
  id: "a1b2c3d4-0000-4000-8000-000000000000",
  nombre: "ALMACEN-01",
  rol: "almacenamiento",
  resumen: {
    admite: ["nombres_legibles"],
    guarda_copias: {
      activo: true,
      repositorios: [{ usuario: U, repos: ["almacen-01-3f2a", "ventas"] }, { usuario: "caja-9a8b", repos: ["caja"] }],
      espejo: { hora: "02:00", trabajos: [trabajo("todo", { tipo: "todos" }, { modo: "retraso", dias: 30 }), trabajo("sel", { tipo: "repos", repos: [`${U}/almacen-01-3f2a`] }), trabajo("caja", { tipo: "equipos", equipos: ["caja-9a8b"] })] },
    },
  },
};
const destino = (u: string) => ({ id: "almacen-a1b2c3d4", nombre: "ALMACEN-01", tipo: "rest" as const, donde: `https://192.0.2.10:8000/${u}/`, equipo_almacen: alm.id });
const pc: Equipo = {
  ...base,
  id: "pc",
  nombre: "PC-01",
  rol: "agente",
  resumen: {
    admite: ["nombres_legibles"],
    destinos: [destino(U)],
    repositorios: [
      { id: "almacen-01-3f2a", nombre: "Contabilidad", destino: "almacen-a1b2c3d4", bytes: 70e9 },
      { id: "ventas", nombre: "Ventas", destino: "almacen-a1b2c3d4" },
    ],
  },
};
const caja: Equipo = { ...base, id: "caja", nombre: "CAJA", rol: "agente", resumen: { destinos: [destino("caja-9a8b")], repositorios: [{ id: "caja", nombre: "Caja", destino: "almacen-a1b2c3d4" }] } };
const importador: Equipo = {
  ...base,
  id: "imp",
  nombre: "PORTATIL",
  rol: "agente",
  resumen: { destinos: [destino(U)], repositorios: [{ id: "contab-imp", nombre: "Contabilidad (importado)", destino: "almacen-a1b2c3d4", solo_lectura: true, ruta: "almacen-01-3f2a" }] },
};
const equipos = [alm, pc, caja, importador];

console.log("\n· Qué se ordena");
const ens = equiposEnAlmacen(alm, equipos);
igual("equipos en el almacén (por destino)", ens.map((x) => [x.equipo.nombre, x.usuario, x.repos.map((r) => r.carpeta), x.importado]), [
  ["PC-01", U, ["almacen-01-3f2a", "ventas"], false],
  ["CAJA", "caja-9a8b", ["caja"], false],
  ["PORTATIL", U, ["almacen-01-3f2a"], true],
]);
const plan = planOrden(ens[0]);
igual("PC-01: su carpeta y la de «Contabilidad» («Ventas» ya está bien)", [plan.usuario, plan.repos.map((r) => [r.de, r.propuesta])], [true, [["almacen-01-3f2a", "contabilidad"]]]);
igual("CAJA: nada", hayQueOrdenar(planOrden(ens[1])), false);
igual("un importado no se ordena (sigue al dueño)", planOrden(ens[2]).repos, []);
igual("solo un repositorio (desde su página)", [planOrden(ens[0], "almacen-01-3f2a").usuario, planOrden(ens[0], "almacen-01-3f2a").repos.length, planOrden(ens[0], "ventas").repos.length], [false, 1, 0]);
igual("la orden al almacén", cuerpoMover(ens[0], plan), { mover: { usuario: U, legible: "PC-01", repos: [{ de: "almacen-01-3f2a", nombre: "Contabilidad" }] } });
igual("sin la del equipo", cuerpoMover(ens[0], { usuario: false, repos: plan.repos }), { mover: { usuario: U, repos: [{ de: "almacen-01-3f2a", nombre: "Contabilidad" }] } });

console.log("\n· Lo que se vuelve a subir en los espejos");
const esp = espejosQueLoCopian(alm, U, ["almacen-01-3f2a"]);
igual("todos y el de su selección; no el de otro equipo", esp.map((t) => t.id), ["todo", "sel"]);
igual("la carpeta vieja: con retención", textoCarpetaVieja(esp[0]), "la carpeta vieja se quita pasados 30 días (su retención)");
igual("la carpeta vieja: nunca borra", textoCarpetaVieja(esp[1]).startsWith("la carpeta vieja se queda allí para siempre"), true);

console.log("\n· La ruta nueva en el equipo (y en el que lo tiene importado)");
const movido = { usuario: { de: U, a: "pc-01-3f2a" }, repos: [{ de: "almacen-01-3f2a", a: "contabilidad" }] };
igual("dueño: usuario, dirección y carpetas", cuerpoRutaNueva(ens[0], movido), {
  destino: "almacen-a1b2c3d4",
  donde: "https://192.0.2.10:8000/pc-01-3f2a/",
  usuario: "pc-01-3f2a",
  carpetas: { "almacen-01-3f2a": "contabilidad" },
});
igual("importado: también", cuerpoRutaNueva(ens[2], movido), { destino: "almacen-a1b2c3d4", donde: "https://192.0.2.10:8000/pc-01-3f2a/", usuario: "pc-01-3f2a", carpetas: { "contab-imp": "contabilidad" } });
igual("otro equipo: nada", cuerpoRutaNueva(ens[1], movido), null);
igual("solo la carpeta del equipo: sin carpetas", cuerpoRutaNueva(ens[0], { usuario: movido.usuario, repos: [] }), { destino: "almacen-a1b2c3d4", donde: "https://192.0.2.10:8000/pc-01-3f2a/", usuario: "pc-01-3f2a" });
igual("dirección con «rest:»", dondeConUsuario("rest:https://almacen:8002/x/", "y"), "rest:https://almacen:8002/y/");

console.log("\n· Rutas que el almacén movió y el equipo aún no usa");
{
  const conRen = (renombres: { de: string; a: string; cuando: string }[]): Equipo => ({ ...alm, resumen: { ...alm.resumen, guarda_copias: { ...alm.resumen!.guarda_copias!, renombres } } });
  const r1 = [
    { de: `${U}/almacen-01-3f2a`, a: "pc-01-3f2a/contabilidad", cuando: "2026-10-13T10:00:00-05:00" },
    { de: U, a: "pc-01-3f2a", cuando: "2026-10-13T10:00:00-05:00" },
  ];
  const p = rutasPendientes(conRen(r1), equipos);
  igual("PC-01 y PORTATIL siguen con la de antes", p.map((x) => [x.en.equipo.nombre, x.cuerpo]), [
    ["PC-01", { destino: "almacen-a1b2c3d4", donde: "https://192.0.2.10:8000/pc-01-3f2a/", usuario: "pc-01-3f2a", carpetas: { "almacen-01-3f2a": "contabilidad" } }],
    ["PORTATIL", { destino: "almacen-a1b2c3d4", donde: "https://192.0.2.10:8000/pc-01-3f2a/", usuario: "pc-01-3f2a", carpetas: { "contab-imp": "contabilidad" } }],
  ]);
  // Ya al día: nada.
  const pcAlDia: Equipo = { ...pc, resumen: { ...pc.resumen, destinos: [destino("pc-01-3f2a")], repositorios: [{ ...pc.resumen!.repositorios![0], ruta: "contabilidad" }, pc.resumen!.repositorios![1]] } };
  igual("al día: nada", rutasPendientes(conRen(r1), [alm, pcAlDia, caja]).length, 0);
  // Movido dos veces: hasta la última.
  const r2 = [...r1, { de: "pc-01-3f2a", a: "pc-01-77aa", cuando: "2026-10-13T11:00:00-05:00" }];
  igual("dos veces: la última", rutasPendientes(conRen(r2), [alm, pc])[0]?.cuerpo, { destino: "almacen-a1b2c3d4", donde: "https://192.0.2.10:8000/pc-01-77aa/", usuario: "pc-01-77aa", carpetas: { "almacen-01-3f2a": "contabilidad" } });
  igual("sin renombres: nada", rutasPendientes(alm, equipos).length, 0);
}

console.log("\n· Los nombres que se ven");
{
  const m = mapaNombres(alm, equipos);
  igual("mapa", [m.get(U), m.get(`${U}/almacen-01-3f2a`), m.get("caja-9a8b/caja")], ["PC-01", "Contabilidad, de PC-01", "Caja, de CAJA"]);
  igual("descripción de una orden", legibleEnTexto(`Cambiar la retención del almacén en ${U}/almacen-01-3f2a`, m), "Cambiar la retención del almacén en «Contabilidad, de PC-01»");
  igual("entre comillas", legibleEnTexto(`Ordenar los nombres de carpeta de «${U}» en el almacén`, m), "Ordenar los nombres de carpeta de «PC-01» en el almacén");
  igual("lo que no se conoce, tal cual", legibleEnTexto("Aplicar la retención del almacén en otro/repo", m), "Aplicar la retención del almacén en otro/repo");
  igual("palabras sueltas, tal cual", legibleEnTexto("Copias de la caja en el almacén", m), "Copias de la caja en el almacén");
}

console.log("\n· Las órdenes");
igual("renombrar un repositorio: sin clave, solo administradores", [NIVEL.nombre_repositorio, SOLO_ADMIN_ROL.has("nombre_repositorio")], ["sesion", true]);

console.log(`\n${total - fallos} de ${total} bien`);
if (fallos) process.exit(1);
