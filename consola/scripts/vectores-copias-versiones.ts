// Pruebas del bloque F de la 0.8.0 (copias y versiones). `npm run test:vectores`.
//
// - F1 (14pp): «Solo a mano», «Una sola vez» y «Termina» en el editor de copias
//   (src/lib/copiaGuiada.ts), lo que se envía (configEnvio.ts), el estado
//   «Completada» (copia.ts) y la regla 3-2-1-1-0 sin plazo (regla321.ts).
// - F2 (14ww-2, 14cc): comentarios y «Conservar para siempre» (versionesNotas.ts),
//   con la misma detección de secretos que el agente.
// - F3 (14u): «N versiones por día» con los vectores que comparte con el agente
//   (crates/protocolo/vectors/retencion-por-dia.json).
// - F4 (14t): franjas de velocidad (franjas.ts), como el agente.
import { readFileSync } from "node:fs";
import type { CopiaConfig, CopiaResumen, Equipo, Regla } from "../src/lib/tipos";
import { copiaNueva, cuandoEnFrase, errorInicio, esAhora, inicioDe, leerComoSeguir, limpiarInicio, pasoHecho, ponerInicio, ponerTermina, ponerUnaVez, reanudarCopia, terminaEnFrase, unaVezAhora, comoSeguirParam } from "../src/lib/copiaGuiada";
import { configParaEnviar, type Admite } from "../src/lib/configEnvio";
import { atrasada, cuandoCopiaEnFrase, estadoCopia, proximaDe } from "../src/lib/copia";
import { copiaAtrasada } from "../src/lib/salud";
import { pasoAlDia } from "../src/lib/regla321";
import { comentariosDe, conservadaDe, destinosConservar, errorComentario, errorMotivo, esMio, mismaVersion, pareceSecreto } from "../src/lib/versionesNotas";
import { errorPorDia, errorRegla, porDiaQuedan, reglaParaOrden, seQuedan, textoPorDia, textoRegla } from "../src/lib/retencion";
import { explicar, prever, textoSeQueda } from "../src/lib/retencionDetalle";
const NB = " "; // el espacio fino de lib/formato.ts
import { errorFranjas, franjaNueva, franjasEnFrase, velocidadEnFrase } from "../src/lib/franjas";

let fallos = 0;
let total = 0;
function igual(nombre: string, obtenido: unknown, esperado: unknown) {
  total++;
  const ok = JSON.stringify(obtenido) === JSON.stringify(esperado);
  if (!ok) fallos++;
  console.log(`${ok ? "ok  " : "MAL "} ${nombre}${ok ? "" : `\n       obtenido: ${JSON.stringify(obtenido)}\n       esperado: ${JSON.stringify(esperado)}`}`);
}
const copiar = <T>(x: T): T => JSON.parse(JSON.stringify(x)) as T;

const NUEVO: Admite = { reglas: true, ganchos: true, soloCambios: true, verif: true, verifHorario: true, escritorio: true, cadenas: true, pruebas: true, despues: true, termina: true };
const ANTERIOR: Admite = { ...NUEVO, termina: false };
const AHORA = new Date(2026, 9, 13, 10, 0).getTime();

console.log("\n· F1: «Solo a mano» y «Una sola vez»");
{
  const copias: CopiaConfig[] = [copiaNueva("r1", "k1"), copiaNueva("r1", "k2")];
  copias[1].carpetas = ["C:\\Datos"];
  ponerInicio(copias, 1, "a_mano", null, AHORA);
  igual("solo a mano: sin horario ni anterior", [inicioDe(copias[1]), copias[1].a_mano, copias[1].tras, copias[1].horario], ["a_mano", true, null, { dias: [], horas: [] }]);
  igual("solo a mano en una frase", cuandoEnFrase(copias, 1), "Solo a mano");
  igual("el paso «Cuándo» está hecho", pasoHecho(copias[1], "cuando"), true);
  limpiarInicio(copias);
  igual("ordenar no le pone horario", copias[1].horario, { dias: [], horas: [] });
  ponerInicio(copias, 0, "una_vez", null, AHORA);
  igual("una sola vez, ahora: la víspera a las 00:00 y desde ahora", [inicioDe(copias[0]), copias[0].termina?.una_vez, copias[0].termina?.desde], ["una_vez", "2026-10-12T00:00", new Date(AHORA).toISOString()]);
  igual("«ahora» se reconoce", [esAhora(copias[0].termina?.una_vez, AHORA), cuandoEnFrase(copias, 0, null, AHORA)], [true, "Una sola vez, ahora"]);
  ponerUnaVez(copias[0], "2026-10-20T21:30", AHORA);
  igual("en una fecha y hora", cuandoEnFrase(copias, 0, null, AHORA), "Una sola vez, el 20 oct 2026 a las 21:30");
  ponerInicio(copias, 0, "horario", null, AHORA);
  igual("volver a «Con horario» quita «una sola vez» y pone el de por defecto", [copias[0].termina, copias[0].horario.horas], [undefined, ["13:00"]]);
  igual("un agente anterior no entiende «Solo a mano»", errorInicio(copias, 1, { cadenas: true, despues: true, termina: false }), "«Copia nueva»: actualiza el agente para «Solo a mano», «Una sola vez» o «Termina».");
  igual("uno nuevo, sí", errorInicio(copias, 1, { cadenas: true, despues: true, termina: true }), null);
  // Lo que se envía: solo a un agente que lo entiende.
  const cfg = { v: 1, copias: copiar(copias), repositorios: [], destinos: [] };
  const nuevo = configParaEnviar(cfg, NUEVO).copias[1] as CopiaConfig;
  igual("se envía `a_mano` a un agente nuevo", [nuevo.a_mano, nuevo.horario], [true, { dias: [], horas: [] }]);
  igual("a uno anterior, no", "a_mano" in configParaEnviar(cfg, ANTERIOR).copias[1], false);
}

console.log("\n· F1: «Termina después de N copias o el día …»");
{
  const k = copiaNueva("r1", "k1");
  ponerTermina(k, "veces", 10, AHORA);
  igual("10 veces, desde ahora", k.termina, { veces: 10, desde: new Date(AHORA).toISOString() });
  igual("en una frase, con las que lleva", cuandoEnFrase([k], 0, 3), "Cada día laborable a las 13:00, 10 veces: van 3");
  ponerTermina(k, "veces", 50_000, AHORA);
  igual("como mucho 10 000", k.termina?.veces, 10_000);
  ponerTermina(k, "hasta", "2027-01-31", AHORA);
  igual("hasta un día", terminaEnFrase(k.termina), "hasta el 31 ene 2027");
  igual("el día mal escrito no vale", errorInicio([{ ...k, termina: { hasta: "31/01/2027" } }], 0, { cadenas: true, despues: true, termina: true }), "«Copia nueva»: el día en que termina no es válido.");
  ponerTermina(k, "nunca");
  igual("nunca: sin el campo", k.termina, undefined);
  const cfg = { v: 1, copias: [{ ...copiaNueva("r1", "k1"), termina: { veces: 3, desde: "2026-10-13T08:00:00.000Z", hasta: null } }], repositorios: [], destinos: [] };
  igual("se envía sin campos vacíos", (configParaEnviar(cfg, NUEVO).copias[0] as CopiaConfig).termina, { veces: 3, desde: "2026-10-13T08:00:00.000Z" });
}

console.log("\n· F1: completada (ni fallida ni atrasada) y «Reanudar»");
{
  const k: CopiaResumen = { id: "k1", nombre: "Cierre", repo: "r1", horario: { dias: [1, 2, 3, 4, 5], horas: ["13:00"] }, activa: true, termina: { veces: 10 }, hechas: 10, completada: new Date(AHORA - 86_400_000).toISOString(), proxima: new Date(AHORA - 5 * 86_400_000).toISOString() };
  const vuelta = { cuando: new Date(AHORA - 6 * 86_400_000).toISOString(), resultado: "ok" as const, mensaje: null };
  igual("estado: completada", estadoCopia(k, vuelta, false, AHORA), { tono: "ok", texto: "Completada" });
  igual("ni atrasada", [atrasada(k, vuelta, false, AHORA), copiaAtrasada(k, AHORA)], [false, false]);
  igual("sin próxima", proximaDe(k, null, AHORA), null);
  igual("en una frase", cuandoCopiaEnFrase(k, AHORA), "Cada día laborable a las 13:00, 10 veces: van 10");
  igual("solo a mano tampoco va atrasada", copiaAtrasada({ ...k, completada: null, a_mano: true }, AHORA), false);
  // Reanudar: la cuenta vuelve a empezar (otro `desde`).
  const c: CopiaConfig = { ...copiaNueva("r1", "k1"), termina: { veces: 10, desde: "2026-01-01T00:00:00.000Z" } };
  reanudarCopia(c, { modo: "veces", veces: 5 }, AHORA);
  igual("5 veces más, desde ahora", c.termina, { veces: 5, desde: new Date(AHORA).toISOString() });
  const u: CopiaConfig = { ...copiaNueva("r1", "k2"), horario: { dias: [], horas: [] }, termina: { una_vez: "2026-10-01T10:00", desde: "2026-10-01T08:00:00.000Z" } };
  reanudarCopia(u, { modo: "nunca" }, AHORA);
  igual("una de «una sola vez» que sigue sin fin recibe horario", [u.termina, u.horario.horas], [undefined, ["13:00"]]);
  reanudarCopia(u, { modo: "una_vez" }, AHORA);
  igual("o una vez más, ahora", u.termina, { una_vez: unaVezAhora(AHORA), desde: new Date(AHORA).toISOString() });
  for (const como of [{ modo: "nunca" }, { modo: "veces", veces: 7 }, { modo: "hasta", hasta: "2027-03-01" }, { modo: "una_vez" }] as const) igual(`«${comoSeguirParam(como)}» en la dirección`, leerComoSeguir(comoSeguirParam(como)), como);
  igual("una dirección rara no reanuda", [leerComoSeguir("veces:0"), leerComoSeguir("hasta:mañana"), leerComoSeguir(null)], [null, null, null]);
}

console.log("\n· F1: la regla 3-2-1-1-0 no exige al día una copia completada o solo a mano");
{
  const hace = (d: number) => new Date(AHORA - d * 86_400_000).toISOString();
  igual("con plazo, atrasada", pasoAlDia({ id: "d", soporte: "s", ultima_ok: hace(200) }, AHORA), false);
  igual("sin plazo, al día si tiene alguna versión", pasoAlDia({ id: "d", soporte: "s", ultima_ok: hace(200), sin_plazo: true }, AHORA), true);
  igual("sin ninguna, no", pasoAlDia({ id: "d", soporte: "s", ultima_ok: null, sin_plazo: true }, AHORA), false);
}

console.log("\n· F2: comentarios y «Conservar para siempre»");
{
  // Lo mismo que el agente (versiones_v2::tests::textos_y_secretos).
  for (const s of ["contraseña: Verano2026", "la clave es 1234", "token=abc", "Xk9vQ2mZ7pL4rT8wY1nB5cD3", "-----BEGIN PRIVATE KEY-----"]) igual(`parece un secreto: ${s}`, pareceSecreto(s), true);
  for (const s of ["Versión buena antes de migrar el programa", "Ver https://ejemplo.com/una/ruta/bastante/larga/123", "Cierre 2025"]) igual(`no lo parece: ${s}`, pareceSecreto(s), false);
  igual("largo máximo", errorComentario("a".repeat(281)), "Como mucho 280 caracteres.");
  igual("con un salto de línea, bien", errorComentario("Antes del cierre\nde mes"), null);
  igual("con un carácter de control, no", errorComentario("con\u0007campana"), "El texto lleva caracteres no válidos.");
  igual("motivo obligatorio", [errorMotivo("ab"), errorMotivo("Cierre contable 2025")], ["Escribe el motivo (de 3 a 200 caracteres).", null]);
  igual("misma versión (corta o completa)", [mismaVersion("3f2a9c1b", "3f2a9c1bdeadbeef"), mismaVersion("3f2a9c1b", "3f2a9c1c"), mismaVersion("3f2a", "3f2a")], [true, false, false]);
  const e = {
    id: "e1",
    nombre: "CAJA-1",
    resumen: {
      admite: ["comentarios_version", "conservar_version"],
      repositorios: [{ id: "r1", nombre: "Ventas", destino: "d1" }],
      destinos: [{ id: "d1", nombre: "Disco D", tipo: "local" }],
      comentarios: [
        { id: "c2", repo: "r1", version: "3f2a9c1bdead", texto: "Segundo", cuando: "2026-10-12T10:00:00Z", autor: "ana", esta: true },
        { id: "c1", repo: "r1", version: "3f2a9c1b", texto: "Primero", cuando: "2026-10-11T10:00:00Z", autor: "luis", esta: false },
        { id: "c3", repo: "r2", version: "3f2a9c1b", texto: "De otro repositorio", cuando: "2026-10-12T10:00:00Z" },
      ],
      conservadas: [{ repo: "r1", version: "3f2a9c1b", motivo: "Cierre contable 2025", cuando: "2026-10-12T10:00:00Z" }],
    },
  } as unknown as Equipo;
  igual("los de la versión, del más antiguo al más reciente", comentariosDe(e, "r1", "3f2a9c1b").map((c) => c.id), ["c1", "c2"]);
  igual("solo los suyos se cambian", comentariosDe(e, "r1", "3f2a9c1b").map((c) => esMio(c, "ana")), [false, true]);
  igual("se conserva, con su motivo", conservadaDe(e, "r1", "3f2a9c1bffff")?.motivo, "Cierre contable 2025");
  igual("sin almacén, solo al equipo", destinosConservar(e, e.resumen!.repositorios![0], [e], "3f2a9c1b", "  Cierre   2025 ", true).map((d) => [d.quien, d.cuerpo]), [["equipo", { repo: "r1", version: "3f2a9c1b", motivo: "Cierre 2025", conservar: true }]]);
}

console.log("\n· F3: «N versiones por día» (vectores compartidos con el agente)");
{
  const doc = JSON.parse(readFileSync(new URL("../../crates/protocolo/vectors/retencion-por-dia.json", import.meta.url), "utf8")) as { casos: { nombre: string; regla: Regla; horas: string[]; quedan: boolean[] }[] };
  for (const c of doc.casos) {
    const horas = c.horas.map((h) => new Date(`${h}:00`));
    igual(c.nombre, seQuedan(horas, c.regla), c.quedan);
    igual(`${c.nombre} (explicar dice lo mismo)`, explicar(horas, c.regla).map((x) => !!x.queda), c.quedan);
  }
  igual("sin regla «por día», nada", porDiaQuedan([new Date()], undefined), [false]);
  igual("en palabras (como el agente)", textoPorDia({ horas: ["18:00", "09:00"], plazo: "30d" }), "2 por día (las más cercanas a las 09:00 y 18:00) durante 30 días");
  igual("la primera y la última", textoPorDia({}), "2 por día (la primera y la última)");
  igual("sola vale", errorRegla({ diarias: 0, semanales: 0, mensuales: 0, anuales: 0, por_dia: {} }), null);
  igual("con un agente que no la aplica, no", errorRegla({ diarias: 7, semanales: 0, mensuales: 0, anuales: 0, por_dia: {} }, true, false), "Este agente aún no aplica «Versiones por día»: actualízalo.");
  igual("horas mal", [errorPorDia({ horas: ["9:00"] }), errorPorDia({ horas: ["09:00", "09:00"] }), errorPorDia({ plazo: "treinta" })], ["Las horas de «Versiones por día» son HH:MM, sin repetir.", "Las horas de «Versiones por día» son HH:MM, sin repetir.", "El plazo de «Versiones por día» no vale (por ejemplo 30d o 6m)."]);
  igual("para la orden, ordenadas", reglaParaOrden({ diarias: 7, semanales: 0, mensuales: 0, anuales: 0, por_dia: { horas: ["18:00", "09:00"], plazo: null } }).por_dia, { horas: ["09:00", "18:00"] });
  igual("en el texto de la regla", textoRegla({ diarias: 7, semanales: 0, mensuales: 0, anuales: 0, por_dia: {} }), "7 diarias · 2 por día (la primera y la última)");
}

console.log("\n· F2: la simulación de la retención respeta lo conservado");
{
  const vs = [0, 1, 2, 3, 4].map((d) => ({ id: `0000000${d}aa`, hora: new Date(AHORA - d * 86_400_000).toISOString(), copia: "k1" }));
  const sin = prever({ versiones: vs, regla: { diarias: 2, semanales: 0, mensuales: 0, anuales: 0 }, ahora: AHORA, dias: 7 });
  const con = prever({ versiones: vs, regla: { diarias: 2, semanales: 0, mensuales: 0, anuales: 0 }, ahora: AHORA, dias: 7, conservadas: ["00000004"] });
  igual("sin marcar, la quitaría", sin.proxima.ids.includes("00000004aa"), true);
  igual("conservada, no", [con.proxima.ids.includes("00000004aa"), con.porVersion.get("00000004aa")?.ahora.queda], [false, "conservada"]);
  igual("y lo dice", textoSeQueda("conservada", new Date(AHORA)), "Se queda: se conserva para siempre.");
}

console.log("\n· F4: franjas de velocidad (como el agente)");
{
  const of = franjaNueva();
  igual("la de por defecto vale", errorFranjas([of]), null);
  igual("al revés, no", errorFranjas([{ ...of, desde: "18:00", hasta: "08:00" }]), "La franja de 18:00 a 08:00 tiene que empezar antes de terminar (el mismo día).");
  igual("se pisan, no", errorFranjas([of, { dias: [5], desde: "17:00", hasta: "20:00", kib: 512 }]), "Dos franjas de velocidad se pisan el mismo día.");
  igual("un límite ridículo, no", errorFranjas([{ ...of, kib: 10 }]), `El límite de cada franja va de 66${NB}KB/s a 10${NB}GB/s (o sin límite).`);
  igual("sin días, no", errorFranjas([{ ...of, dias: [] }]), "Cada franja de velocidad necesita sus días.");
  igual("en palabras", franjasEnFrase([of], null), `2,1${NB}MB/s de 08:00 a 18:00 (lun–vie); el resto, sin límite`);
  igual("velocidades", [velocidadEnFrase(0), velocidadEnFrase(512), velocidadEnFrase(4096)], ["sin límite", `524${NB}KB/s`, `4,2${NB}MB/s`]);
}

console.log(`\n${total - fallos}/${total} ${fallos ? "con fallos" : "bien"}`);
if (fallos) process.exit(1);
