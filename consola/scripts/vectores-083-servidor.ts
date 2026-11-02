// Pruebas de la rama `ia/083-servidor` (plan 0.8.3): 16k (sesiones de la consola) y lo
// que venga después en esta rama. `npm run test:vectores`.
import type { Equipo, SesionCuenta } from "../src/lib/tipos";
import { detalleDisco, infraValida, leerInfra, marcaDeModelo, marcaYModelo } from "../src/lib/infra";
import { adaptarPara } from "../src/lib/datosComunes";
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

console.log("\n· 16a y 16f: marca de los discos, número de serie e inventario del servidor");
{
  igual("marca: Samsung por el modelo", marcaDeModelo("Samsung SSD 870 EVO 1TB"), "Samsung");
  igual("marca: WD por WDC", marcaDeModelo("WDC WD40EFRX-68N32N0"), "WD");
  igual("marca: Seagate por ST…", marcaDeModelo("ST4000VN008-2DR166"), "Seagate");
  igual("marca: Crucial por CT…", marcaDeModelo("CT1000MX500SSD1"), "Crucial");
  igual("marca: Toshiba", marcaDeModelo("TOSHIBA MG04ACA400E"), "Toshiba");
  igual("marca: HPE (no HP)", marcaDeModelo("HPE MB4000GVYZK"), "HPE");
  igual("marca: si no es evidente, nada", marcaDeModelo("Disco virtual 2"), null);
  igual("marca: sin modelo, nada", marcaDeModelo(null), null);
  igual("marca y modelo sin repetir", marcaYModelo("Samsung", "Samsung SSD 870 EVO"), "Samsung SSD 870 EVO");
  igual("marca y modelo juntos", marcaYModelo("WD", "Red Plus 4TB"), "WD Red Plus 4TB");
  igual("solo la marca", marcaYModelo("Dell", null), "Dell");
  igual("detalle del disco con su marca", detalleDisco({ tipo: "hdd", tamano: null, marca: "WD", modelo: "Red Plus", bahia: "2" }), "HDD · WD Red Plus · bahía 2");

  const valor = { nombre: "hv-01", serie: "ABC1234", inventario: "INV-0042", discos: [{ id: "b1", nombre: "Bahía 1", marca: "WD", serie: "SN-1" }] };
  igual("infra válida con serie, inventario y marca", infraValida("infra:servidor:hv-01", valor), { ok: true, limpio: valor });
  igual("serie de más de 64: no", infraValida("infra:servidor:hv-01", { nombre: "hv-01", serie: "x".repeat(65) }).ok, false);
  igual("marca de más de 80: no", infraValida("infra:servidor:hv-01", { nombre: "hv-01", discos: [{ id: "b1", nombre: "B", marca: "m".repeat(81) }] }).ok, false);

  const nuevo = { resumen: { admite: ["infra_comun", "infra_ficha"] } } as unknown as Equipo;
  const viejo = { resumen: { admite: ["infra_comun"] } } as unknown as Equipo;
  const x = { clave: "infra:servidor:hv-01", valor, cambiado: "2026-11-02T10:00:00Z" };
  igual("a un agente que lo admite: tal cual", adaptarPara(nuevo, x), x);
  igual("a uno anterior: sin serie, inventario ni marca", adaptarPara(viejo, x).valor, { nombre: "hv-01", discos: [{ id: "b1", nombre: "Bahía 1", serie: "SN-1" }] });
  igual("otras claves: tal cual", adaptarPara(viejo, { clave: "infra:equipo:e1", valor: { servidor: "hv-01" } }).valor, { servidor: "hv-01" });
  igual("quitado (null): tal cual", adaptarPara(viejo, { clave: "infra:servidor:hv-01", valor: null }).valor, null);
  const leido = leerInfra({ servidores: [{ id: "hv-01", nombre: "hv-01", serie: "ABC1234", inventario: "INV-0042", discos: [{ id: "b1", nombre: "Bahía 1", marca: "WD" }], conjuntos: [] }] });
  igual("leer: la serie y el inventario del servidor", [leido.servidores[0].serie, leido.servidores[0].inventario], ["ABC1234", "INV-0042"]);
  igual("leer: la marca del disco", leido.servidores[0].discos[0].marca, "WD");
}

console.log(`\n${total - fallos} de ${total} bien`);
if (fallos) process.exit(1);
