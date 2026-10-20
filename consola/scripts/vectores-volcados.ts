// Pruebas de los volcados de SQL Server en la consola (plan 0.8.1, A3 y A4: 15f, 15u y 15j):
// «todas las bases» (validación, lo que se manda y la frase), los volcados agrupados con el
// mismo problema («sin verificar: falta permiso»), «volcadas y verificadas» o «sin verificar»
// y la línea corta del historial. `npm run test:vectores`.
import { agruparResultados, errorGancho, ganchosEnFrase, limpiar, paraConfig, resumenResultados } from "../src/lib/ganchos";
import { elegibles, estadoBase, leerLista } from "../src/lib/basesSql";
import type { GanchoSqlserver, ResultadoGancho } from "../src/lib/tipos";

let fallos = 0;
let total = 0;
function igual(nombre: string, obtenido: unknown, esperado: unknown) {
  total++;
  const ok = JSON.stringify(obtenido) === JSON.stringify(esperado);
  if (!ok) fallos++;
  console.log(`${ok ? "ok  " : "MAL "} ${nombre}${ok ? "" : `\n       obtenido: ${JSON.stringify(obtenido)}\n       esperado: ${JSON.stringify(esperado)}`}`);
}

console.log("\n· «Todas las bases» (15u)");
const todas: GanchoSqlserver = { tipo: "sqlserver", instancia: ".", bases: [], carpeta: "C:\\ResguardoVolcados", todas: true, excluir: ["Pruebas"], sistema: true };
igual("válido sin lista", errorGancho(todas), null);
igual("excluidas con las reglas de una base", errorGancho({ ...todas, excluir: ["x]; --"] }) !== null, true);
igual("sin «todas», la lista hace falta", errorGancho({ ...todas, todas: undefined }) !== null, true);
igual("a un agente que lo entiende", limpiar(todas, true, true), { tipo: "sqlserver", bases: [], carpeta: "C:\\ResguardoVolcados", todas: true, excluir: ["Pruebas"], sistema: true });
igual("a uno anterior, sin los campos nuevos", Object.keys(limpiar({ ...todas, bases: ["Ventas"] }, true, false)).sort(), ["bases", "carpeta", "tipo"]);
igual("paraConfig con todas", (paraConfig([todas], false, true) as GanchoSqlserver).todas, true);
igual("la frase de la copia", ganchosEnFrase([todas]), ["volcado de todas las bases de SQL Server salvo Pruebas"]);

console.log("\n· La lista de la instancia");
const lista = leerLista({ bases: [{ nombre: "master", estado: "ONLINE", sistema: true }, { nombre: "tempdb", estado: "ONLINE", sistema: true }, { nombre: "Ventas", estado: "ONLINE", sistema: false }, { nombre: 3 }] });
igual("lo que no tiene forma no cuenta", lista.length, 3);
igual("tempdb no se elige", elegibles(lista).map((b) => b.nombre), ["master", "Ventas"]);
igual("estado en palabras", estadoBase("RECOVERY_PENDING"), "pendiente de recuperación");

console.log("\n· Volcados agrupados (15f, 15j)");
const r = (estado: ResultadoGancho["estado"], mensaje: string, verificado: boolean | null = null): ResultadoGancho => ({ tipo: "sqlserver", estado, mensaje, verificado });
const permiso = (b: string) => r("aviso", `Volcado de SQL Server de «${b}» hecho, sin verificar: falta permiso. Para verificarlo, …`);
igual(
  "el mismo aviso en varias bases, una sola línea",
  agruparResultados([permiso("Ventas"), permiso("Nominas"), permiso("Compras")]).map((x) => x.mensaje),
  ["Volcado de SQL Server de 3 bases (Ventas, Nominas y Compras) hecho, sin verificar: falta permiso. Para verificarlo, …"],
);
igual(
  "verificadas",
  agruparResultados([r("ok", "Volcado de SQL Server de «A» hecho y verificado.", true), r("ok", "Volcado de SQL Server de «B» hecho y verificado.", true)])[0].mensaje,
  "2 bases volcadas y verificadas: A y B.",
);
igual(
  "sin verificar (apagado)",
  agruparResultados([r("ok", "Volcado de SQL Server de «A» hecho (sin verificar)."), r("ok", "Volcado de SQL Server de «B» hecho (sin verificar).")])[0].mensaje,
  "2 bases volcadas sin verificar: A y B.",
);
igual("fallos distintos, separados", agruparResultados([r("fallo", "No se pudo volcar «A» de SQL Server: x"), r("fallo", "No se pudo volcar «B» de SQL Server: y")]).length, 2);
igual("la línea del historial", resumenResultados([r("ok", "a"), r("ok", "b"), r("fallo", "c")]), "volcado de SQL Server: 2 bases bien, 1 falló");
igual("todas igual", resumenResultados([r("fallo", "a"), r("fallo", "b")]), "volcado de SQL Server falló (2 bases)");
igual("una sola", resumenResultados([r("ok", "a")]), "volcado de SQL Server correcto");

console.log(`\n${total - fallos} de ${total} bien`);
if (fallos) process.exit(1);
