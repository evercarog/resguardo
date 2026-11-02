// Pruebas de la rama `ia/083-servidor` (plan 0.8.3): 16k (sesiones de la consola) y lo
// que venga después en esta rama. `npm run test:vectores`.
import type { SesionCuenta } from "../src/lib/tipos";
import { conTopes, esMovil, frasePlazos, lineaSesion, motivoNoCerrar, nuevasSinVer, ordenarSesiones, textoSesionNueva } from "../src/lib/sesionesCuenta";

let fallos = 0;
let total = 0;
function igual(nombre: string, obtenido: unknown, esperado: unknown) {
  total++;
  const ok = JSON.stringify(obtenido) === JSON.stringify(esperado);
  if (!ok) fallos++;
  console.log(`${ok ? "ok  " : "MAL "} ${nombre}${ok ? "" : `\n       obtenido: ${JSON.stringify(obtenido)}\n       esperado: ${JSON.stringify(esperado)}`}`);
}
function cierto(nombre: string, v: boolean) {
  igual(nombre, v, true);
}

console.log("\n· 16k: sesiones de la consola");
{
  const ahora = Date.parse("2026-11-02T12:00:00Z");
  const iso = (ms: number) => new Date(ahora + ms).toISOString();
  const s = (x: Partial<SesionCuenta>): SesionCuenta => ({
    id: "a",
    actual: false,
    navegador: "Chrome · Android",
    ip: "192.0.2.10",
    creada: iso(-3 * 86_400_000),
    usada: iso(-2 * 3_600_000),
    caduca: iso(6 * 3_600_000),
    maximo: iso(20 * 3_600_000),
    recordar: false,
    nueva: false,
    ...x,
  });
  igual("móvil: Android", esMovil("Chrome · Android"), true);
  igual("móvil: iPhone", esMovil("Safari · iPhone"), true);
  igual("ordenador: Windows", esMovil("Edge · Windows"), false);

  const l = lineaSesion(s({}), ahora);
  cierto("línea: cuándo se usó", l.startsWith("Usada hace"));
  cierto("línea: desde dónde", l.includes("desde 192.0.2.10"));
  cierto("línea: sin recordar, se cierra si no se usa", l.includes("se cierra si no se usa"));
  cierto("línea: la actual se llama «Esta sesión»", lineaSesion(s({ actual: true }), ahora).startsWith("Esta sesión"));
  cierto("línea: recordada hasta…", lineaSesion(s({ recordar: true }), ahora).includes("recordada hasta el"));
  cierto("línea: sin IP conocida, no la pone", !lineaSesion(s({ ip: "?" }), ahora).includes("desde"));

  const orden = ordenarSesiones([s({ id: "vieja", usada: iso(-9 * 3_600_000) }), s({ id: "yo", actual: true, usada: iso(-1) }), s({ id: "reciente", usada: iso(-60_000) })]);
  igual("orden: la actual primero, luego la más usada", orden.map((x) => x.id), ["yo", "reciente", "vieja"]);

  igual("puede cerrar: sin motivo", motivoNoCerrar(true, null, ahora), null);
  const m = motivoNoCerrar(false, iso(5 * 3_600_000), ahora) ?? "";
  cierto("sesión nueva: dice que es nueva", m.startsWith("Esta sesión es nueva"));
  cierto("sesión nueva: dice cuánto falta", m.includes("Podrás dentro de 5"));
  cierto("sesión nueva: ofrece otro dispositivo", m.includes("dispositivo donde ya tenías la sesión abierta"));

  const nuevas = [{ id: "n1", navegador: "Firefox · Linux", creada: iso(-5 * 60_000) }, { id: "n2", navegador: "", creada: iso(-60_000) }];
  igual("aviso: sin ver", nuevasSinVer(nuevas, ["n1"]).map((n) => n.id), ["n2"]);
  igual("aviso: sin campo (servidor anterior)", nuevasSinVer(undefined, []), []);
  cierto("aviso: con el navegador", textoSesionNueva(nuevas[0], ahora).startsWith("Nueva sesión en Firefox · Linux, hace 5"));
  cierto("aviso: sin navegador conocido", textoSesionNueva(nuevas[1], ahora).startsWith("Nueva sesión en un navegador"));

  igual("topes: como el servidor", conTopes({ inactividad_horas: 0, maximo_horas: 9999, recordar_dias: 365 }), { inactividad_horas: 1, maximo_horas: 168, recordar_dias: 30 });
  igual("topes: el máximo nunca menor que la inactividad", conTopes({ inactividad_horas: 12, maximo_horas: 2, recordar_dias: 0 }), { inactividad_horas: 12, maximo_horas: 12, recordar_dias: 0 });
  igual("topes: lo que no es un número, al valor por defecto", conTopes({ inactividad_horas: Number.NaN, maximo_horas: Number.NaN, recordar_dias: Number.NaN }), { inactividad_horas: 8, maximo_horas: 24, recordar_dias: 14 });
  igual("plazos en una frase", frasePlazos({ inactividad_horas: 8, maximo_horas: 24, recordar_dias: 14 }), "Se cierra tras 8 h sin usarla y, aunque se use, a las 24 h; con «Recordar este dispositivo», hasta 14 días.");
  cierto("plazos sin recordar", frasePlazos({ inactividad_horas: 8, maximo_horas: 24, recordar_dias: 0 }).endsWith("sin «Recordar este dispositivo»."));
}

console.log(`\n${total - fallos} de ${total} bien`);
if (fallos) process.exit(1);
