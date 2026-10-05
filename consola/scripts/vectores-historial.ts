// Pruebas de «Historial y versiones» (src/lib/historial.ts) y de la marca de
// fallos del calendario (src/lib/lineaTiempo.ts) y de la lista de Copias
// (src/lib/copiasCliente.ts). `npm run test:vectores`.
import type { CopiaResumen, EntradaHistorial, Equipo, RepoInforme, RepositorioResumen, VersionInforme } from "../src/lib/tipos";
import { filasCopias, filtrarCopias, ordenarCopias } from "../src/lib/copiasCliente";
import { esFallo, pasaFiltro, sucesosDe } from "../src/lib/historial";
import { altoCalendario, calendario, diaDeClave, nombreIntervalo } from "../src/lib/lineaTiempo";

let fallos = 0;
let total = 0;
function igual(nombre: string, obtenido: unknown, esperado: unknown) {
  total++;
  const ok = JSON.stringify(obtenido) === JSON.stringify(esperado);
  if (!ok) fallos++;
  console.log(`${ok ? "ok  " : "MAL "} ${nombre}${ok ? "" : `\n       obtenido: ${JSON.stringify(obtenido)}\n       esperado: ${JSON.stringify(esperado)}`}`);
}

console.log("\n· Historial y versiones (lib/historial.ts)");
const ahora = Date.parse("2026-10-04T12:00:00Z");
const h = (horas: number) => new Date(ahora - horas * 3600_000).toISOString();
const v = (id: string, horas: number, copia = "k1"): VersionInforme => ({ id, hora: h(horas), copia, total_bytes: 1e9, anadido: 1e6, anadido_empaquetado: 1e6, archivos_nuevos: 1, archivos_cambiados: 2, archivos_sin_cambios: 0, duracion_s: 60, etiquetas: [] });
const repo: RepositorioResumen = { id: "r1", nombre: "Documentos", destino: "d" };
const repo2: RepositorioResumen = { id: "r2", nombre: "Contabilidad", destino: "d" };
const copias: CopiaResumen[] = [
  { id: "k1", nombre: "Escritorio", repo: "r1" },
  { id: "k2", nombre: "Facturas", repo: "r1" },
  { id: "k3", nombre: "Siigo", repo: "r2" },
];
const inf: RepoInforme = {
  id: "r1",
  nombre: "Documentos",
  // La versión v1 empezó un minuto antes de que terminara su vuelta.
  versiones: [v("aaaa0001", 2.02), v("aaaa0002", 26.02, "k2")],
  versiones_leidas: null,
  ejecuciones: [
    { hora: h(2), copia: "k1", resultado: "aviso", mensaje_corto: "Un archivo estaba en uso." },
    { hora: h(14), copia: "k1", resultado: "fallo", mensaje_corto: "Sin red." },
    { hora: h(20), copia: "k2", resultado: "sin_cambios", mensaje_corto: null, duracion_s: 30 },
    { hora: h(26), copia: "k2", resultado: "ok", mensaje_corto: null },
  ],
  espacio: null,
  verificacion: { ultima: h(30), resultado: "ok", mensaje_corto: null },
  prueba_restauracion: null,
  externa: { ultima: h(8), resultado: "fallo", mensaje_corto: "Token caducado." },
  proteccion: null,
};
const inf2: RepoInforme = { ...inf, id: "r2", nombre: "Contabilidad", versiones: [v("bbbb0001", 5, "k3")], ejecuciones: [{ hora: h(4.98), copia: "k3", resultado: "ok", mensaje_corto: null }], verificacion: null, externa: null };
const historial: EntradaHistorial[] = [
  // La misma vuelta fallida que trae el informe: no se repite.
  { id: "h1", hora: h(14), tipo: "copia", repo: "r1", copia: "k1", resultado: "fallo", mensaje: "Sin red." },
  // Una vuelta de antes del informe, con un paso previo que falló.
  { id: "h2", hora: h(24 * 70), tipo: "copia", repo: "r1", copia: "k1", resultado: "ok", anadido: 5e6, ganchos: [{ tipo: "sqlserver", estado: "fallo", mensaje: "No respondió." }] },
  // La misma verificación que el informe: no se repite.
  { id: "h3", hora: h(30), tipo: "verificacion", repo: "r1", resultado: "ok" },
  { id: "h4", hora: h(24 * 40), tipo: "prueba_restauracion", repo: "r1", resultado: "ok" },
  { id: "h5", hora: h(3), tipo: "espejo", resultado: "ok" },
];
const r = sucesosDe({ fuentes: [{ repo, inf }], copias, historial });
igual(
  "los sucesos de un repositorio, sin repetir y del más reciente al más antiguo",
  r.sucesos.map((s) => `${s.tipo}:${s.tono}`),
  ["externa:bad", "fallo:bad", "sin_cambios:ok", "verificacion:ok", "prueba:ok", "copia:ok"],
);
igual("una vuelta que dejó versión no es un suceso: su aviso va en la versión", r.notas.get("aaaa0001"), { tono: "warn", texto: "Un archivo estaba en uso.", ganchos: [] });
igual("…y una correcta, sin aviso", r.notas.get("aaaa0002")?.tono, null);
igual("la que falló dice por qué y se abre en el cajón", [r.sucesos[1].detalle, r.sucesos[1].vuelta], ["Sin red.", h(14)]);
igual("el paso previo que falló va en su copia", r.sucesos.at(-1)?.detalle, "No respondió.");
igual("el espejo del almacén no es de un repositorio: solo en el equipo", r.sucesos.some((s) => s.tipo === "espejo"), false);

const k = sucesosDe({ fuentes: [{ repo, inf }], copias, historial, soloCopia: "k2" });
igual("una copia: lo suyo y lo de su repositorio", k.sucesos.map((s) => s.tipo), ["externa", "sin_cambios", "verificacion", "prueba"]);

const eq = sucesosDe({ fuentes: [{ repo, inf }, { repo: repo2, inf: inf2 }], copias, historial, conRepo: true, conEquipo: true });
igual("un equipo: también el espejo", eq.sucesos.filter((s) => s.tipo === "espejo").length, 1);
igual("…y los títulos dicen de qué repositorio", eq.sucesos.find((s) => s.tipo === "verificacion")?.titulo, "Verificación · «Documentos»");
igual("…y las versiones de cada repositorio casan con su vuelta", eq.notas.has("bbbb0001"), true);

igual("filtro «Fallos»: solo lo que falló", r.sucesos.filter((s) => pasaFiltro(s, "fallos")).map((s) => s.tipo), ["externa", "fallo"]);
{
  // Un aviso del equipo (otra consola, intentos fallidos…) es un aviso, nunca un fallo; una copia con avisos, también.
  const conAvisos = sucesosDe({
    fuentes: [{ repo, inf: { ...inf, versiones: [], ejecuciones: [{ hora: h(6), copia: "k1", resultado: "aviso", mensaje_corto: "Un archivo estaba en uso." }], externa: null, verificacion: null } }],
    copias,
    historial: [
      { id: "a1", hora: h(1), tipo: "aviso", mensaje: "Este equipo se conectó también a otra consola." },
      { id: "a2", hora: h(24 * 400), tipo: "resumen_dia", repo: "r1", copia: "k1", ok: 3, fallidas: 1 },
    ],
    conEquipo: true,
  }).sucesos;
  igual("clases: aviso del equipo, copia con avisos y día con una fallida", conAvisos.map((s) => `${s.tipo}:${s.clase}`), ["aviso:aviso", "copia:aviso", "resumen:fallo"]);
  igual("«Fallos» no enseña los avisos", conAvisos.filter((s) => pasaFiltro(s, "fallos")).map((s) => s.tipo), ["resumen"]);
  igual("«Avisos» los enseña", conAvisos.filter((s) => pasaFiltro(s, "avisos")).map((s) => s.tipo), ["aviso", "copia"]);
  igual("la marca roja del calendario, solo los fallos", conAvisos.filter(esFallo).map((s) => s.tipo), ["resumen"]);
}
igual("filtro «Comprobaciones»", r.sucesos.filter((s) => pasaFiltro(s, "comprobaciones")).map((s) => s.tipo), ["verificacion", "prueba"]);
igual("filtro «Subidas»", eq.sucesos.filter((s) => pasaFiltro(s, "subidas")).map((s) => s.tipo), ["espejo", "externa"]);
igual("filtro «Versiones»: ningún suceso", r.sucesos.filter((s) => pasaFiltro(s, "versiones")).length, 0);

console.log("\n· La marca de fallos del calendario (lib/lineaTiempo.ts)");
const vs = [{ id: "a", t: Date.parse(h(2)) }];
const cal = calendario(vs, null, ahora, 7, false, [Date.parse(h(14)), Date.parse(h(8))]);
igual("los fallos del rango, en sus casillas", cal.celdas.flat().reduce((n, x) => n + x.fallos, 0), 2);
igual("…y en la cabecera de su día", cal.columnas.reduce((n, c) => n + (c.dia?.fallos ?? 0), 0), 2);
igual("…sin cambiar la intensidad (las versiones)", cal.total, 1);
const tira = calendario(vs, null, ahora, 30, true, [Date.parse(h(14))]);
igual("en la tira del móvil, también", tira.celdas[0].reduce((n, x) => n + x.fallos, 0), 1);

console.log("\n· Un marco estable y las fechas (lib/lineaTiempo.ts)");
const fijas = { desde: 8, paso: 1, n: 12 };
igual("con las filas fijas, el mismo número de filas en 7, 30 y 60 días", [7, 30, 60].map((d) => calendario(vs, null, ahora, d as 7 | 30 | 60, false, [], fijas).filas.length), [12, 12, 12]);
igual("el marco: el más alto de sus vistas (aquí, por horas)", altoCalendario(12, false), 52 + 15 * 12);
igual("…con pocas filas, el del año", altoCalendario(2, false), altoCalendario(0, true));
igual("un día de la URL", diaDeClave("2026-09-29") === new Date(2026, 8, 29).getTime(), true);
igual("un día mal formado", diaDeClave("29/09/2026"), null);
const hoyL = new Date(2026, 9, 5, 12).getTime();
igual("intervalo del mismo mes", nombreIntervalo(new Date(2026, 8, 21).getTime(), new Date(2026, 8, 27).getTime(), hoyL), "Del 21 al 27 sept");
igual("intervalo entre meses", nombreIntervalo(new Date(2026, 8, 28).getTime(), new Date(2026, 9, 4).getTime(), hoyL), "Del 28 sept al 4 oct");
igual("un solo día: su nombre", nombreIntervalo(new Date(2026, 9, 4).getTime(), new Date(2026, 9, 4).getTime(), hoyL), "Ayer");

console.log("\n· Copias del cliente (lib/copiasCliente.ts)");
const base = { so: "Windows 11", version_agente: "0.7.17", box_pub: "", sign_pub: "", sal_equipo: "", etiqueta: null, modo: "gestionado" as const, confirmado: true, conectado: true, ultimo_contacto: h(0.1), estado_servicio: "en_marcha" as const, siguiente_seq: 1, rol: "agente" as const };
const e1: Equipo = { ...base, id: "e1", nombre: "CONTABILIDAD", resumen: { destinos: [{ id: "d", nombre: "Almacén", tipo: "rest" }], repositorios: [{ ...repo2 }], copias: [{ id: "k3", nombre: "Siigo", repo: "r2", activa: true, ultima: { cuando: h(3), estado: "fallo", mensaje: "Sin red" } }] } };
const e2: Equipo = {
  ...base,
  id: "e2",
  nombre: "RECEPCIÓN",
  modo: "trasladado",
  resumen: { destinos: [{ id: "d", nombre: "Nube", tipo: "b2" }], repositorios: [{ ...repo }], copias: [{ id: "k1", nombre: "Escritorio", repo: "r1", activa: true }, { id: "k2", nombre: "Facturas", repo: "r1", activa: false }] },
};
const filas = filasCopias([e1, e2], { e2: { recibido: h(1), datos: { repos: [inf] } } }, ahora);
igual("una fila por copia de cada equipo", filas.map((x) => x.clave), ["e1|k3", "e2|k1", "e2|k2"]);
igual("su estado, de la última copia", filas.map((x) => x.estado.texto), ["Con error", "Con avisos", "Desactivada"]);
igual("lo que protege: su última versión", filas[1].protegido, 1e9);
igual("se le pueden mandar órdenes (no a un equipo trasladado ni a una copia desactivada)", filas.map((x) => x.ordenable), [true, false, false]);
igual("buscar sin tildes y por todas las palabras", filtrarCopias(filas, { buscar: "recepcion escri" }).map((x) => x.clave), ["e2|k1"]);
igual("filtro «Necesitan atención»", filtrarCopias(filas, { estado: "atencion" }).map((x) => x.clave), ["e1|k3", "e2|k1"]);
igual("filtro «En pausa o desactivadas»", filtrarCopias(filas, { estado: "paradas" }).map((x) => x.clave), ["e2|k2"]);
igual("filtro por equipo y por repositorio", [filtrarCopias(filas, { equipo: "e1" }).length, filtrarCopias(filas, { repo: "e2|r1" }).length], [1, 2]);
igual("orden por estado: primero lo que falla", ordenarCopias(filas, "estado").map((x) => x.clave), ["e1|k3", "e2|k1", "e2|k2"]);
igual("orden por lo que protege: lo que no se sabe, al final", ordenarCopias(filas, "protegido").map((x) => x.clave)[0], "e2|k1");

console.log(`\n${total - fallos} de ${total} comprobaciones correctas.`);
if (fallos) process.exit(1);
