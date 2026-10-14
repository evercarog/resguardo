// Pruebas de las pruebas funcionales (plan 0.8.0, E3, 14k) y de las pruebas desde un
// espejo (E4, 14j): textos y formulario (src/lib/pruebasFuncionales.ts) y cómo cuentan
// en el «0» de la regla 3-2-1-1-0 (src/lib/regla321.ts).
//
//   npm run test:vectores
import { aMinutos, cifrasPruebas, deMinutos, deRestauracion, errorPrueba, opcionesPruebaEspejo, ordenPruebaEspejo, pruebaNueva, textoAlcance, textoMinutos, textoPlazo, textoPruebaEspejo } from "../src/lib/pruebasFuncionales";
import { conOtraPrueba, conRestauracionReal, estadoCero, fraseCero, idPruebaEspejo, quePrueba, ultimaFuncional, type PasoVista } from "../src/lib/regla321";
import { bytes } from "../src/lib/formato";
import type { Equipo, Restauracion } from "../src/lib/tipos";
import type { PruebaFuncional } from "../src/lib/tiposPruebas";

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

console.log("\n— Tiempos (RTO y RPO) —");
igual("minutos", textoMinutos(45), "45 min");
igual("horas justas", textoMinutos(120), "2 h");
igual("horas y minutos", textoMinutos(150), "2 h 30 min");
igual("días", textoMinutos(3 * 1440), "3 días");
igual("sin dato", textoMinutos(null), "—");
igual("1,5 horas", aMinutos("1,5", "h"), 90);
igual("2 días", aMinutos("2", "dias"), 2880);
igual("vacío", aMinutos(" ", "min"), null);
igual("no es un número", Number.isNaN(aMinutos("dos", "h")), true);
igual("ida y vuelta", deMinutos(90), { texto: "1,5", unidad: "h" });
igual("ida y vuelta en días", deMinutos(2880), { texto: "2", unidad: "dias" });

console.log("\n— El formulario —");
const hoy = "2026-10-14";
const base = { ...pruebaNueva(hoy), que: "Facturas", donde: "Máquina de pruebas" };
igual("una correcta vale", errorPrueba(base, hoy), null);
igual("futura, no", errorPrueba({ ...base, fecha: "2026-10-15" }, hoy) !== null, true);
igual("unos repositorios sin ninguno, no", errorPrueba({ ...base, alcance: "repos", repos: [] }, hoy) !== null, true);
igual("con incidencias pide contarlas", errorPrueba({ ...base, resultado: "parcial" }, hoy), "Cuenta qué falló o qué incidencias hubo.");
igual("con incidencias contadas, vale", errorPrueba({ ...base, resultado: "parcial", incidencias: "Faltó un permiso" }, hoy), null);
igual("RTO de más de un año, no", errorPrueba({ ...base, rto_min: 600_000 }, hoy) !== null, true);

console.log("\n— De una restauración registrada —");
const rest: Restauracion = { equipo: "e1", id: "restauracion-9", hora: "2026-10-10T10:30:00Z", inicio: "2026-10-10T10:00:00Z", duracion_s: 1750, repo: "r1", version: "a1b2c3d4", archivos: 120, bytes: 52_000_000, destino: "nueva", resultado: "ok" };
const d = deRestauracion(rest, "2026-10-09T22:00:00Z");
igual("alcance: su repositorio", [d.alcance, d.repos], ["repos", [{ equipo: "e1", repo: "r1" }]]);
igual("RTO: lo que tardó (en minutos, hacia arriba)", d.rto_min, 30);
igual("RPO: desde la versión hasta que empezó", d.rpo_min, 720);
igual("enlazada", d.restauracion, { equipo: "e1", id: "restauracion-9" });
igual("a dónde", d.donde, "En otra carpeta");
igual("sin la hora de la versión, sin RPO", deRestauracion(rest).rpo_min, null);
igual("una fallida, fallida", deRestauracion({ ...rest, resultado: "fallo", mensaje: "Sin espacio" }).resultado, "fallo");

console.log("\n— Cifras y textos —");
const pf = (x: Partial<PruebaFuncional>): PruebaFuncional => ({ ...pruebaNueva(hoy), id: "p", creada: "", actualizada: "", por: "Ana", adjuntos: [], ...x });
const lista = [pf({ id: "a", rto_min: 60, rpo_min: 600, cumple_rto: true, cumple_rpo: true }), pf({ id: "b", resultado: "parcial", rto_min: 180, cumple_rto: false }), pf({ id: "c", resultado: "fallo" })];
const cf = cifrasPruebas(lista);
igual("cuenta por resultado", [cf.total, cf.correctas, cf.conIncidencias, cf.fallidas], [3, 1, 1, 1]);
igual("RTO medio y peor", [cf.rtoMedio, cf.rtoPeor], [120, 180]);
igual("objetivos", [cf.rtoCumplen, cf.rtoConObjetivo, cf.rpoCumplen, cf.rpoConObjetivo], [1, 2, 1, 1]);
igual("alcance con nombres", textoAlcance({ alcance: "repos", repos: [{ equipo: "e1", repo: "r1" }, { equipo: "e2", repo: "r2" }] }, (_e, r) => r.toUpperCase()), "R1 y R2");
igual("todo el cliente", textoAlcance({ alcance: "cliente", repos: [] }, () => "x"), "Todo el cliente");
igual("plazo vencido", textoPlazo({ ultima: null, vence: "2026-04-01T00:00:00Z", fase: "vencida", meses: 6 }).tono, "bad");
igual("plazo al día", textoPlazo({ ultima: null, vence: "2027-04-01T00:00:00Z", fase: "al_dia", meses: 6 }).texto.startsWith("Al día"), true);

console.log("\n— En el «0» de la regla (E3) —");
const ps = [pf({ fecha: "2026-10-01", alcance: "repos", repos: [{ equipo: "e1", repo: "r1" }] }), pf({ fecha: "2026-09-01", alcance: "cliente" }), pf({ fecha: "2026-10-05", alcance: "repos", repos: [{ equipo: "e2", repo: "r9" }], resultado: "fallo" })];
igual("la más reciente que incluye el repositorio", ultimaFuncional(ps, "e1", "r1")?.ok, true);
igual("…con su día", ultimaFuncional(ps, "e1", "r1")?.cuando.slice(0, 10), "2026-10-01");
igual("la de todo el cliente cuenta para otro", ultimaFuncional(ps, "e3", "x")?.cuando.slice(0, 10), "2026-09-01");
igual("una fallida de otro repositorio, no", ultimaFuncional(ps, "e2", "r9")?.ok, false);
igual("sin ninguna", ultimaFuncional([], "e1", "r1"), null);
const prog = { configurada: true, ultima_ok: "2026-09-20T04:00:00Z", fallo: false };
igual("funcional más reciente y correcta: manda ella", conOtraPrueba(prog, { cuando: "2026-10-01T10:00:00Z", ok: true, origen: "funcional" }), { configurada: true, ultima_ok: "2026-10-01T10:00:00Z", fallo: false, origen: "funcional", origenNombre: null });
igual("funcional más reciente que falló: el «0» por revisar", conOtraPrueba(prog, { cuando: "2026-10-01T10:00:00Z", ok: false, origen: "funcional" }).fallo, true);
igual("funcional anterior: manda la programada", conOtraPrueba(prog, { cuando: "2026-09-01T10:00:00Z", ok: false, origen: "funcional" }), prog);
igual("sin nada programado, la funcional basta", conOtraPrueba({ configurada: false }, { cuando: "2026-10-01T10:00:00Z", ok: true, origen: "funcional" }).configurada, true);
igual("con la real (13e) y una funcional después", conOtraPrueba(conRestauracionReal(prog, "2026-09-25T10:00:00Z"), { cuando: "2026-10-02T10:00:00Z", ok: true, origen: "funcional" }).origen, "funcional");
igual("lo dice en la frase", quePrueba({ configurada: true, origen: "funcional" }), " (la prueba: una prueba funcional)");
igual("y desde un espejo", quePrueba({ configurada: true, origen: "espejo", origenNombre: "Nube Sur" }), " (la prueba: desde el espejo «Nube Sur»)");
const cero = estadoCero({ verificacion: { configurada: true, ultima_ok: "2026-10-10T00:00:00Z" }, prueba_restauracion: { configurada: true, ultima_ok: "2026-10-01T10:00:00Z", fallo: true, origen: "funcional" }, pasos: [] }, Date.parse("2026-10-14T00:00:00Z"));
igual("el «0» dice que la funcional falló", fraseCero(cero), "Falta: una prueba de restauración correcta (la última, una prueba funcional, falló) (la verificación ya está bien).");

console.log("\n— Desde un espejo (E4) —");
const equipo = (x: Partial<Equipo>): Equipo => ({ id: "e1", nombre: "pc-01", so: "windows", ...x }) as Equipo;
const almacen = equipo({ id: "a1", nombre: "almacen-01" });
const paso = (x: Partial<PasoVista>): PasoVista => ({ id: "espejo-1", nombre: "Disco E", tipo: "espejo", soporte: "s", clave: "espejo:a1:0000", ...x }) as PasoVista;
const deEquipo = paso({ id: "espejo-equipo-t1", nombre: "Disco F de pc-01", clave: null, espejo: { quien: "equipo", trabajo: "t1", tipo: "carpeta", carpeta: "F:\\Espejo", ruta: "docs" } });
const carpetaAlmacen = paso({ espejo: { quien: "almacen", almacen: "a1", tipo: "carpeta", carpeta: "E:\\Espejo", ruta: "pc-01-3f2a/docs" } });
const nubeAlmacen = paso({ id: "espejo-2", nombre: "Nube Sur", clave: "nube:a1:nube-sur", espejo: { quien: "almacen", almacen: "a1", tipo: "nube", carpeta: "mi-bucket/Resguardo", nube: "Nube Sur", ruta: "pc-01-3f2a/docs" } });
igual("ids estables y válidos para el equipo", [idPruebaEspejo(deEquipo), idPruebaEspejo(deEquipo)].every((x) => /^e-[0-9a-f]{8}$/.test(x)) && idPruebaEspejo(deEquipo) === idPruebaEspejo({ ...deEquipo, id: "otro" }), true);
igual("ids distintos por espejo", idPruebaEspejo(carpetaAlmacen) !== idPruebaEspejo(nubeAlmacen), true);
const ops = opcionesPruebaEspejo([deEquipo, carpetaAlmacen, nubeAlmacen, paso({ tipo: "copia", espejo: undefined })], equipo({}), [almacen]);
igual("solo los espejos", ops.length, 3);
igual("el del propio equipo: listo, por su trabajo", [ops[0].modo, ops[0].orden.trabajo], ["listo", "t1"]);
igual("el disco del almacén desde otro equipo: hay que conectarlo", [ops[1].modo, ops[1].sugerida], ["carpeta", "E:\\Espejo\\pc-01-3f2a\\docs"]);
igual("la nube del almacén sin nubes en el equipo: no se puede (y dice por qué)", [ops[2].modo, /conéctala también en «pc-01»/.test(ops[2].nota)], ["no", true]);
const conNube = opcionesPruebaEspejo([nubeAlmacen], equipo({ resumen: { nubes: [{ nombre: "Nube Sur", tipo: "b2" }] } as Equipo["resumen"] }), [almacen]);
igual("con una nube en el equipo: se elige y se propone la carpeta", [conNube[0].modo, conNube[0].sugerida], ["nube", "mi-bucket/Resguardo/pc-01-3f2a/docs"]);
igual("orden con la nube", ordenPruebaEspejo(conNube[0], { nube: "Nube Sur", carpeta: "/mi-bucket/Resguardo/pc-01-3f2a/docs/" }, null), { ...conNube[0].orden, nube: "Nube Sur", carpeta: "mi-bucket/Resguardo/pc-01-3f2a/docs" });
igual("programada", ordenPruebaEspejo(ops[0], {}, 30)?.cada_dias, 30);
igual("sin carpeta, nada", ordenPruebaEspejo(ops[1], { carpeta: " " }, null), null);
const mismo = opcionesPruebaEspejo([carpetaAlmacen], equipo({ id: "a1", nombre: "almacen-01" }), [almacen]);
igual("el almacén probando lo suyo: listo, en su disco", [mismo[0].modo, mismo[0].orden.carpeta], ["listo", "E:\\Espejo\\pc-01-3f2a\\docs"]);
igual("resultado en palabras", textoPruebaEspejo({ resultado: "ok", archivos: 20, bytes: 1_048_576 }), `Bien: 20 archivos, ${bytes(1_048_576)} restaurados y comprobados.`);
igual("fallo en palabras", textoPruebaEspejo({ resultado: "fallo", mensaje_corto: "Un archivo no coincide." }), "Falló: Un archivo no coincide.");

console.log(`\n${total - fallos} de ${total} comprobaciones correctas.`);
if (fallos) process.exit(1);
