// Pruebas del registro de restauraciones y de «Restaurar mejor» (plan 0.8.0, E1 y E2):
// src/lib/restauraciones.ts, la restauración real en la regla (src/lib/regla321.ts),
// quién la pidió en el progreso (src/lib/textoProgreso.ts) y qué órdenes piden la
// clave de administración por su cuerpo (src/lib/cripto/ordenes.ts).
//
//   npm run test:vectores
import { claseRestauracion, soloReales, textoComprobacion, textoMuestra, errorCarpetaRestaurar, errorContrasenaZip, errorNombreZip, estadisticas, juntar, medidaEstimada, porMes, recomendar, textoDesde, textoDestino, textoMedida, textoQuien, textoTardo } from "../src/lib/restauraciones";
import { alternarNodo, estadoNodo, exclusionesVisibles } from "../src/lib/arbolCarpetas";
import { conRestauracionReal } from "../src/lib/regla321";
import { quienRestaura, textoFase } from "../src/lib/textoProgreso";
import { pideAdminPorCuerpo } from "../src/lib/cripto/ordenes";
import type { Restauracion, Restauraciones, TareaEnMarcha } from "../src/lib/tipos";

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

const r = (x: Partial<Restauracion>): Restauracion => ({ equipo: "e1", id: "restauracion-1", hora: "2026-10-10T10:00:00Z", repo: "r1", resultado: "ok", ...x });

console.log("\n— Carpeta donde restaurar (13a, 13b) —");
igual("Windows, carpeta normal", errorCarpetaRestaurar("D:\\Restaurado", true), null);
igual("Windows, una unidad entera", errorCarpetaRestaurar("E:\\", true), null);
igual("Windows, relativa", errorCarpetaRestaurar("Restaurado", true) !== null, true);
igual("Windows, de red", errorCarpetaRestaurar("\\\\nas\\copias", true) !== null, true);
igual("Windows, con ..", errorCarpetaRestaurar("D:\\a\\..\\Windows", true) !== null, true);
igual("Windows, la carpeta de Windows", errorCarpetaRestaurar("C:\\Windows\\Temp\\x", true) !== null, true);
igual("Windows, Program Files", errorCarpetaRestaurar("c:\\program files\\x", true) !== null, true);
igual("Windows, ProgramData", errorCarpetaRestaurar("C:\\ProgramData\\ResguardoAgente", true) !== null, true);
igual("Windows, punto al final", errorCarpetaRestaurar("D:\\Datos.", true) !== null, true);
igual("Windows, vacía", errorCarpetaRestaurar("  ", true) !== null, true);
igual("Linux, carpeta normal", errorCarpetaRestaurar("/srv/restaurado", false), null);
igual("Linux, /etc", errorCarpetaRestaurar("/etc/x", false) !== null, true);
igual("Linux, la del agente", errorCarpetaRestaurar("/var/lib/resguardo-agente/x", false) !== null, true);
igual("Linux, la raíz", errorCarpetaRestaurar("/", false) !== null, true);
igual("Linux, con ..", errorCarpetaRestaurar("/srv/../etc", false) !== null, true);

console.log("\n— Nombre y contraseña del .zip —");
igual("nombre vacío vale", errorNombreZip(""), null);
igual("nombre normal", errorNombreZip("Facturas de marzo"), null);
igual("con .zip", errorNombreZip("Facturas.zip"), null);
igual("con barra", errorNombreZip("a/b") !== null, true);
igual("dispositivo", errorNombreZip("CON") !== null, true);
igual("sin contraseña", errorContrasenaZip("", ""), null);
igual("corta", errorContrasenaZip("corta", "corta") !== null, true);
igual("distintas", errorContrasenaZip("una buena clave", "otra buena clave") !== null, true);
igual("bien", errorContrasenaZip("una buena clave", "una buena clave"), null);

console.log("\n— La clave de administración por el cuerpo —");
igual("restaurar junto, no", pideAdminPorCuerpo("restaurar", { destino: "junto" }), false);
igual("restaurar en otra carpeta, sí", pideAdminPorCuerpo("restaurar", { destino: "nueva" }), true);
igual("restaurar en .zip, sí", pideAdminPorCuerpo("restaurar", { destino: "zip" }), true);
igual("descargar, no", pideAdminPorCuerpo("descargar", { destino: "zip" }), false);

console.log("\n— Registro: motivo, cifras y textos (13d, 14l) —");
const datos: Restauraciones = {
  ajustes: { motivo_obligatorio: false },
  entradas: [
    r({ id: "restauracion-a", hora: "2026-10-01T10:00:00Z", duracion_s: 60, bytes: 1000, archivos: 3, motivo: { tipo: "prueba" } }),
    r({ id: "restauracion-b", hora: "2026-10-05T10:00:00Z", duracion_s: 180, bytes: 3000, archivos: 1 }),
    r({ id: "restauracion-c", hora: "2026-09-20T10:00:00Z", duracion_s: 30, resultado: "fallo" }),
    r({ id: "restauracion-d", hora: "2026-10-06T10:00:00Z", duracion_s: 120, bytes: 0, archivos: 0 }),
  ],
  notas: [{ equipo: "e1", id: "restauracion-b", motivo: "borrado", notas: "Lo borró un usuario", por: "Ana" }],
};
const todas = juntar(datos);
igual("de la más reciente a la más antigua", todas.map((x) => x.id), ["restauracion-d", "restauracion-b", "restauracion-a", "restauracion-c"]);
igual("el motivo de esta consola manda", todas.find((x) => x.id === "restauracion-b")?.motivoFinal?.tipo, "borrado");
igual("el de la orden, si no hay otro", todas.find((x) => x.id === "restauracion-a")?.motivoFinal?.tipo, "prueba");
const e = estadisticas(todas);
igual("cifras", [e.total, e.correctas, e.fallidas, e.exito, e.medio_s, e.mediana_s, e.maximo_s, e.bytes, e.archivos, e.reales, e.sinMotivo], [4, 3, 1, 75, 120, 120, 180, 4000, 4, 2, 2]);
igual("sin restauraciones", estadisticas([]).exito, null);
const meses = porMes(todas, Date.parse("2026-10-13T12:00:00Z"), 2);
igual("por mes", meses.map((m) => [m.mes, m.correctas, m.fallidas, m.medio_s]), [["2026-09", 0, 1, null], ["2026-10", 3, 0, 120]]);
igual("tardó, segundos", textoTardo(45), "45 s");
igual("tardó, minutos", textoTardo(180), "3 min");
igual("tardó, horas", textoTardo(4800), "1 h 20 min");
igual("tardó, sin dato", textoTardo(null), "—");
igual("destino, zip", textoDestino("zip"), "En un .zip en el equipo");
igual("destino, nueva", textoDestino("nueva"), "En otra carpeta");
igual("desde una derivada", textoDesde({ desde: "derivada", desde_nombre: "Nube" }), "Copia derivada en «Nube»");
igual("desde su repositorio", textoDesde({ desde: "principal" }), "Su repositorio");
igual("quién, desde una consola", textoQuien({ por: "Ana", consola: "Oficina" }), "Ana, desde «Oficina»");
igual("quién, desde la ventana", textoQuien({ en_el_equipo: true }), "Desde la ventana del equipo");

console.log("\n— Desde dónde (14m) —");
const os = recomendar([
  { id: "principal", tipo: "principal", nombre: "Su repositorio", donde: "Almacén", estado: "fallo", ultima: null },
  { id: "d1", tipo: "derivada", nombre: "Copia derivada", donde: "Nube", estado: "ok", ultima: "2026-10-12T10:00:00Z", noSe: "Actualiza el agente." },
  { id: "espejo:x", tipo: "espejo", nombre: "Espejo", donde: "Disco E", estado: "ok", ultima: "2026-10-12T08:00:00Z" },
]);
igual("el principal no responde: el siguiente disponible", os.filter((o) => o.recomendado).map((o) => o.id), ["espejo:x"]);
igual("todo bien: el principal", recomendar([{ id: "principal", tipo: "principal", nombre: "", donde: "", estado: "ok", ultima: null }, { id: "espejo:x", tipo: "espejo", nombre: "", donde: "", estado: "ok", ultima: null }]).find((o) => o.recomendado)?.id, "principal");

console.log("\n— Restauración real como prueba (13e) —");
const real = "2026-10-10T10:00:00Z";
igual("sin prueba programada", conRestauracionReal({ configurada: false }, real), { configurada: true, ultima_ok: real, fallo: false, real: true });
igual("prueba anterior correcta: manda la real", conRestauracionReal({ configurada: true, ultima_ok: "2026-09-01T00:00:00Z", fallo: false }, real).real, true);
igual("prueba posterior correcta: manda la prueba", conRestauracionReal({ configurada: true, ultima_ok: "2026-10-11T00:00:00Z", fallo: false }, real).real, undefined);
igual("prueba posterior que falló: manda el fallo", conRestauracionReal({ configurada: true, ultima_ok: "2026-10-11T00:00:00Z", fallo: true }, real).fallo, true);
igual("prueba anterior que falló: la real la arregla", conRestauracionReal({ configurada: true, ultima_ok: "2026-10-01T00:00:00Z", fallo: true }, real).fallo, false);
igual("sin restauración real, igual", conRestauracionReal({ configurada: true, ultima_ok: null }, null), { configurada: true, ultima_ok: null });

console.log("\n— Quién la empezó (13f) —");
const t = (x: Partial<TareaEnMarcha>): TareaEnMarcha => ({ tipo: "restauracion", repo: "r1", fase: "en_marcha", etapa: "Restaurando", ...x });
igual("desde esta consola", textoFase(t({ por: "Ana", otra_consola: false })), "Restaurando (pedida por Ana)");
igual("desde otra consola", textoFase(t({ por: "Ana", otra_consola: true, consola: "Oficina" })), "Restaurando (pedida por Ana desde «Oficina»)");
igual("desde la ventana del equipo", quienRestaura(t({ en_el_equipo: true })), "desde la ventana del equipo");
igual("un agente anterior (sin datos)", textoFase(t({})), "Restaurando");

console.log("\n— El árbol de Restaurar y lo que de verdad se restaura (15w) —");
{
  // Rutas de una versión: siempre con «/» y distinguiendo mayúsculas (win = false).
  let s = { elegidas: [] as string[], excluidas: [] as string[] };
  s = alternarNodo("/C/Docs", s, false);
  igual("marcar una carpeta marca lo de dentro", estadoNodo("/C/Docs/a.txt", s, false), "si");
  s = alternarNodo("/C/Docs/sub", s, false);
  igual("desmarcar un hijo lo quita…", exclusionesVisibles(s, false), ["/C/Docs/sub"]);
  igual("…y la de arriba queda a medias", estadoNodo("/C/Docs", s, false), "medio");
  igual("lo de dentro de lo quitado, sin marcar", estadoNodo("/C/Docs/sub/x", s, false), "no");
  s = alternarNodo("/C/Docs/sub", s, false);
  igual("volver a marcarlo deja la carpeta entera", [estadoNodo("/C/Docs", s, false), exclusionesVisibles(s, false)], ["si", []]);
  igual("mayúsculas distintas son otra ruta", estadoNodo("/C/docs/a.txt", s, false), "no");
}
const nbsp = (x: string) => x.replace(/\s/g, " ");
igual("medido por el equipo", nbsp(textoMedida({ carpetas: 1, archivos: 20, bytes: 5_400_000_000 }).linea), "1 carpeta · 20 archivos · 5,4 GB");
igual("con algo quitado", nbsp(textoMedida({ carpetas: 1, archivos: 20, bytes: 1000, fuera_archivos: 2 }).sin ?? ""), "sin 2 archivos");
igual("sin carpetas", textoMedida({ carpetas: 0, archivos: 1, bytes: 0 }).linea, "1 archivo");
const est = medidaEstimada([{ tipo: "dir" }, { tipo: "archivo", bytes: 2048 }], [{ tipo: "dir" }, { tipo: "archivo", bytes: 10 }]);
igual("lo que se sabe sin medir", nbsp(textoMedida(est).linea), "1 carpeta · 1 archivo · 2 KB");
igual("lo quitado sin medir", nbsp(textoMedida(est).sin ?? ""), "sin 1 carpeta y 1 archivo (10 B)");
igual("solo carpetas sin medir: no dice «0 archivos»", textoMedida(medidaEstimada([{ tipo: "dir" }], [])).linea, "1 carpeta");

console.log("\n— Las pruebas de restauración en «Restauraciones» (15aa) —");
{
  const p = (x: Partial<Restauracion>) => r({ tipo: "prueba_restauracion", ...x });
  igual("una real", claseRestauracion(r({})), "real");
  igual("una prueba de su horario", claseRestauracion(p({ origen: "horario" })), "prueba_auto");
  igual("una prueba de un agente anterior cuenta como automática", claseRestauracion(p({})), "prueba_auto");
  igual("una prueba con «Probar ahora»", claseRestauracion(p({ origen: "a_mano" })), "prueba_mano");
  igual("las reales, aparte", soloReales([r({}), p({}), p({ origen: "a_mano" })]).length, 1);
  igual("quién: su horario", textoQuien(p({ origen: "horario" })), "Su horario (automática)");
  igual("quién: a mano", textoQuien(p({ origen: "a_mano", por: "Ana", consola: "Oficina" })), "Ana, con «Probar ahora» desde «Oficina»");
  igual("quién: la persona del equipo (15mm)", textoQuien(r({ cuenta: "ana (PC-01)", en_el_equipo: true })), "ana (PC-01), en su equipo");
  igual("desde un espejo", textoDesde(p({ espejo: { id: "x", nombre: "Nube" } })), "Espejo «Nube»");
  igual("la muestra", textoMuestra({ archivos: 20, candidatos: 1200 }).replace(/\s/g, " "), "20 archivos al azar (de 1200 archivos de la versión)");
  igual("dónde", textoDestino("privada"), "En una carpeta privada del equipo, borrada al terminar");
  igual("cómo se comprobó (solo en pruebas)", [textoComprobacion(p({})) !== null, textoComprobacion(r({}))], [true, null]);
}

console.log(`\n${total - fallos} de ${total} comprobaciones correctas.`);
if (fallos) process.exit(1);
