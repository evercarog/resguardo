// Pruebas de «Equipos que no están en todas las consolas» (src/lib/consolasCliente.ts,
// docs/plan-mejoras.md tarea 2). `npm run test:vectores` (con las demás).
import type { ConsolaDelEquipo, Equipo } from "../src/lib/tipos";
import { equiposQueFaltan, faltaEn, fraseEquipo, nombreConsola, otrasConsolas } from "../src/lib/consolasCliente";

let fallos = 0;
let total = 0;
function igual(nombre: string, obtenido: unknown, esperado: unknown) {
  total++;
  const ok = JSON.stringify(obtenido) === JSON.stringify(esperado);
  if (!ok) fallos++;
  console.log(`${ok ? "ok  " : "MAL "} ${nombre}${ok ? "" : `\n       obtenido: ${JSON.stringify(obtenido)}\n       esperado: ${JSON.stringify(esperado)}`}`);
}

const AHORA = Date.parse("2026-10-06T12:00:00Z");
const hace = (dias: number) => new Date(AHORA - dias * 24 * 3600_000).toISOString();
const ESTA = "ZXN0YS0xMjM=";
const LINEA = "ZW4tbGluZWEtMQ==";
const VPN = "b2ZpY2luYS12cG4=";
const esta: ConsolaDelEquipo = { id: "principal", nombre: "", url: "https://consola.local.ejemplo:8443", identidad: ESTA, sal_cliente: null, ultimo_contacto: hace(0), desde: null, esta: true };
const consola = (identidad: string, nombre: string, url: string, ultimo: string | null, desde: string | null = hace(20)): ConsolaDelEquipo => ({ id: identidad, nombre, url, identidad, sal_cliente: "c2Fs", ultimo_contacto: ultimo, desde, esta: false });
const linea = (ultimo = hace(0)) => consola(LINEA, "Consola en línea", "https://consola.ejemplo.com", ultimo);
const base = { so: "Windows 11", version_agente: "0.7.21", box_pub: "", sign_pub: "", sal_equipo: "", etiqueta: null, modo: "gestionado" as const, confirmado: true, conectado: true, ultimo_contacto: null, estado_servicio: "en_marcha" as const, siguiente_seq: 1, rol: "agente" as const };
const ADMITE = ["consolas_multiples"];
const equipo = (id: string, consolas: ConsolaDelEquipo[] | undefined, extra: Partial<Equipo> = {}): Equipo => ({
  ...base,
  id,
  nombre: id.toUpperCase(),
  resumen: { admite: ADMITE, ...(consolas ? { consolas } : {}) },
  ...extra,
});
const ids = (es: Equipo[]) => es.map((e) => e.id);

console.log("\n· Las otras consolas del cliente (lib/consolasCliente.ts)");
{
  const a = equipo("a", [esta, linea()]);
  const b = equipo("b", [esta, linea(hace(1))]);
  const nuevo = equipo("nuevo", [esta]);
  const r = otrasConsolas([a, b, nuevo], AHORA);
  igual("una consola: la en línea, con A y B dentro", r.map((c) => [c.nombre, ids(c.con), ids(c.sin)]), [["Consola en línea", ["a", "b"], ["nuevo"]]]);
  igual("falta el nuevo", ids(equiposQueFaltan(r)), ["nuevo"]);
  igual("…en la en línea", faltaEn(nuevo, r).map((c) => c.identidad), [LINEA]);
  igual("A no falta en ninguna", faltaEn(a, r).length, 0);
  igual("la frase del alta", fraseEquipo(nuevo, r[0], [a, b, nuevo]), "Este equipo solo está en esta consola; los demás también están en «Consola en línea».");
  igual("nombre con su dirección", nombreConsola(r[0]), "Consola en línea (consola.ejemplo.com)");
  igual("sin nombre, el host", nombreConsola({ nombre: "", url: "https://otra.ejemplo.com:8443" }), "otra.ejemplo.com:8443");
}
{
  // Sin otras consolas, ningún aviso.
  igual("todos solo aquí: nada", otrasConsolas([equipo("a", [esta]), equipo("b", undefined)], AHORA).length, 0);
  igual("sin resumen: nada", otrasConsolas([equipo("a", undefined, { resumen: null })], AHORA).length, 0);
}
{
  // Quién cuenta y quién no.
  const a = equipo("a", [esta, linea()]);
  const sinResumen = equipo("recien", undefined, { resumen: null });
  const sinConfirmar = equipo("pend", [esta], { confirmado: false });
  const local = equipo("local", [esta], { modo: "local" });
  const trasladado = equipo("tras", [esta], { modo: "trasladado" });
  const viejo = equipo("viejo", undefined, { resumen: { admite: [] } });
  const r = otrasConsolas([a, sinResumen, sinConfirmar, local, trasladado, viejo], AHORA);
  igual("sin resumen aún, sin confirmar, local o trasladado: no cuentan; un agente anterior sí", ids(r[0].sin), ["viejo"]);
  igual("recién dado de alta (nuevo): cuenta aunque no haya informado", ids(otrasConsolas([a, sinResumen], AHORA, ["recien"])[0].sin), ["recien"]);
  igual("un equipo gestionado desde otra consola y aquí en modo local no cuenta para ella", ids(otrasConsolas([equipo("x", [esta, linea()], { modo: "local" }), a], AHORA)[0].con), ["a"]);
}
{
  // Consolas abandonadas: no se sugieren.
  const viejaContacto = equipo("a", [esta, linea(hace(45))]);
  const nueva = equipo("n", [esta]);
  igual("más de 30 días sin contacto de nadie: no se sugiere", otrasConsolas([viejaContacto, nueva], AHORA).length, 0);
  const otroReciente = equipo("b", [esta, linea(hace(2))]);
  igual("…pero si otro equipo la ve hace poco, sí", ids(otrasConsolas([viejaContacto, otroReciente, nueva], AHORA)[0].sin), ["n"]);
  const recienAnadida = equipo("c", [esta, consola(VPN, "Oficina norte", "https://norte.ejemplo.com", null, hace(1))]);
  igual("añadida hace poco y aún sin contacto: se espera", otrasConsolas([recienAnadida, nueva], AHORA).map((c) => c.nombre), ["Oficina norte"]);
  const nunca = equipo("d", [esta, consola(VPN, "Oficina norte", "https://norte.ejemplo.com", null, hace(10))]);
  igual("añadida hace más de 7 días y nunca contactada: no", otrasConsolas([nunca, nueva], AHORA).length, 0);
}
{
  // Dos consolas: cada equipo falta donde falta; la frase se adapta.
  const a = equipo("a", [esta, linea(), consola(VPN, "Oficina norte", "https://norte.ejemplo.com", hace(0))]);
  const b = equipo("b", [esta, linea()]);
  const c = equipo("c", [esta]);
  const r = otrasConsolas([a, b, c], AHORA);
  igual("primero la que tiene más equipos", r.map((x) => [x.nombre, ids(x.sin)]), [["Consola en línea", ["c"]], ["Oficina norte", ["b", "c"]]]);
  igual("sin repetir: B y C", ids(equiposQueFaltan(r)), ["c", "b"]);
  igual("B ya está en otra: «no está en…» y solo A está allí", fraseEquipo(b, r[1], [a, b, c]), "Este equipo no está en «Oficina norte»; A también está en «Oficina norte».");
  igual("C: «solo está en esta consola»", fraseEquipo(c, r[1], [a, b, c]), "Este equipo solo está en esta consola; A también está en «Oficina norte».");
  // El nombre: el del contacto más reciente (por si la renombraron).
  const d = equipo("d", [esta, { ...linea(hace(3)), nombre: "Nombre viejo" }]);
  igual("nombre del contacto más reciente", otrasConsolas([d, a], AHORA)[0].nombre, "Consola en línea");
  const varios = [equipo("e1", [esta, linea()]), equipo("e2", [esta, linea()]), equipo("e3", [esta, linea()]), equipo("e4", [esta]), equipo("e5", [esta])];
  const r2 = otrasConsolas(varios, AHORA);
  igual("varios: «otros N equipos de este cliente»", fraseEquipo(varios[3], r2[0], varios), "Este equipo solo está en esta consola; otros 3 equipos de este cliente también están en «Consola en línea».");
}
{
  // La identidad manda (no la dirección): misma consola con otra dirección es la misma.
  const a = equipo("a", [esta, linea()]);
  const b = equipo("b", [esta, { ...linea(), url: "https://agentes.consola.ejemplo.com" }]);
  igual("misma identidad, otra dirección: una sola consola", otrasConsolas([a, b], AHORA).map((c) => ids(c.con)), [["a", "b"]]);
}

console.log(`\n${total - fallos} de ${total} comprobaciones correctas.`);
if (fallos) process.exit(1);
