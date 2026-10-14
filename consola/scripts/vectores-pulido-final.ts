// Pruebas del pulido final de la 0.8.0: la contraseña de un repositorio nuevo
// (14ii, src/lib/contrasenaRepo.ts) y el explorador de carpetas del equipo
// (14zz, src/lib/arbolCarpetas.ts). `npm run test:vectores` (con las demás).
import { claveElegida, errorContrasena, generarContrasena, MIN_ESCRITA, nuevaContrasena, olvidarContrasena } from "../src/lib/contrasenaRepo";
import { admiteVerificarVolcado, agruparResultados, limpiar, paraConfig } from "../src/lib/ganchos";
import { admiteDe } from "../src/lib/configEnvio";
import { alternarNodo, dentroDe, dentroDeExcluida, esRutaExacta, estadoNodo, exclusionesFinales, quitarExclusion, seleccionInicial } from "../src/lib/arbolCarpetas";
import type { Equipo, Gancho, ResultadoGancho } from "../src/lib/tipos";

let fallos = 0;
let total = 0;
function igual(nombre: string, obtenido: unknown, esperado: unknown) {
  total++;
  const ok = JSON.stringify(obtenido) === JSON.stringify(esperado);
  if (!ok) fallos++;
  console.log(`${ok ? "ok  " : "MAL "} ${nombre}${ok ? "" : `\n       obtenido: ${JSON.stringify(obtenido)}\n       esperado: ${JSON.stringify(esperado)}`}`);
}

console.log("\n· 14ii: contraseña de un repositorio nuevo");
{
  const g = generarContrasena();
  igual("la generada tiene 43 caracteres base64url", /^[A-Za-z0-9_-]{43}$/.test(g), true);
  igual("dos generadas no se repiten", generarContrasena() !== g, true);
  const c = nuevaContrasena();
  igual("por defecto, generada", c.propia, false);
  igual("la generada vale", errorContrasena(c), null);
  igual("se usa la generada", claveElegida(c), c.generada);
  c.propia = true;
  igual("escrita vacía: falta", errorContrasena(c), `Al menos ${MIN_ESCRITA} caracteres.`);
  c.escrita = "corta-11-ch";
  igual("11 caracteres no bastan", errorContrasena(c), "Al menos 12 caracteres.");
  c.escrita = "caballo-grapa-42";
  c.repetida = "caballo-grapa-43";
  igual("tiene que repetirse igual", errorContrasena(c), "Las dos no coinciden.");
  c.repetida = c.escrita;
  igual("escrita y repetida: vale", errorContrasena(c), null);
  igual("se usa la escrita", claveElegida(c), "caballo-grapa-42");
  c.escrita = c.repetida = " caballo-grapa-42";
  igual("sin espacios al principio", errorContrasena(c), "Sin espacios al principio ni al final.");
  olvidarContrasena(c);
  igual("olvidar borra todo", [c.generada, c.escrita, c.repetida], ["", "", ""]);
  c.propia = false;
  igual("generada olvidada: falta", errorContrasena(c), "Falta la contraseña");
}

console.log("\n· 14ccc: verificar cada volcado de SQL Server");
{
  const g: Gancho = { tipo: "sqlserver", bases: ["Ventas"], carpeta: "C:\\ResguardoVolcados", verificar: false };
  igual("a un agente anterior no se manda «verificar»", "verificar" in limpiar(g), false);
  igual("apagado, a un agente que lo entiende", limpiar(g, true), { tipo: "sqlserver", bases: ["Ventas"], carpeta: "C:\\ResguardoVolcados", verificar: false });
  igual("encendido (lo normal): sin el campo", "verificar" in limpiar({ ...g, verificar: undefined }, true), false);
  igual("paraConfig lo pasa", paraConfig([g], true), { tipo: "sqlserver", bases: ["Ventas"], carpeta: "C:\\ResguardoVolcados", verificar: false });
  igual("admite: con verificar_volcado", admiteVerificarVolcado({ resumen: { admite: ["termina", "verificar_volcado"] } }), true);
  igual("admite: agente anterior", admiteVerificarVolcado({ resumen: { admite: ["termina"] } }), false);
  igual("Admite del equipo", admiteDe({ version_agente: "0.8.0", resumen: { admite: ["verificar_volcado"] } } as unknown as Equipo).verificarVolcado, true);
  const ok = (b: string, v?: boolean): ResultadoGancho => ({ tipo: "sqlserver", estado: "ok", mensaje: `Volcado de SQL Server de «${b}» hecho${v ? " y verificado" : ""}.`, verificado: v ?? null });
  igual("todas verificadas: lo dice", agruparResultados([ok("A", true), ok("B", true)]).map((r) => [r.mensaje, r.verificado]), [["2 bases volcadas y verificadas: A y B.", true]]);
  igual("alguna sin verificar: no lo dice", agruparResultados([ok("A", true), ok("B")]).map((r) => [r.mensaje, r.verificado]), [["2 bases volcadas: A y B.", null]]);
  const mal: ResultadoGancho = { tipo: "sqlserver", estado: "fallo", mensaje: "El volcado de «C» no se pudo verificar: dañado", verificado: false };
  igual("la que no pasó, suelta", agruparResultados([ok("A", true), ok("B", true), mal]).map((r) => r.estado), ["ok", "fallo"]);
}

console.log("\n· 14zz: árbol de carpetas con subcarpetas");
{
  const W = true;
  const U = "C:\\Users\\ana";
  const D = `${U}\\Documents`;
  const V = `${U}\\Videos`;
  let s = seleccionInicial([U], ["*.tmp", `${V}`, "C:\\Otra\\*.log"], W);
  igual("de «No copiar» solo entran las rutas exactas", s.excluidas, [V]);
  igual("la carpeta con una excluida dentro: a medias", estadoNodo(U, s, W), "medio");
  igual("su subcarpeta: marcada", estadoNodo(D, s, W), "si");
  igual("la de dentro de una marcada, también (aunque aparezca después)", estadoNodo(`${D}\\Nueva`, s, W), "si");
  igual("la excluida: sin marcar", estadoNodo(V, s, W), "no");
  igual("lo de dentro de la excluida no se puede marcar suelto", dentroDeExcluida(`${V}\\2026`, s, W), true);
  igual("la de arriba de una elegida: a medias", estadoNodo("C:\\Users", s, W), "medio");
  igual("sin distinguir mayúsculas en Windows", estadoNodo("c:\\users\\ANA\\documents", s, W), "si");
  s = alternarNodo(D, s, W);
  igual("desmarcar una subcarpeta la excluye con su ruta", s, { elegidas: [U], excluidas: [V, D] });
  s = alternarNodo(D, s, W);
  igual("volver a marcarla la quita de «No copiar»", s, { elegidas: [U], excluidas: [V] });
  s = alternarNodo(U, s, W);
  igual("marcar la que está a medias: entera", s, { elegidas: [U], excluidas: [] });
  s = alternarNodo(U, s, W);
  igual("desmarcar la elegida: fuera (sin exclusiones colgando)", s, { elegidas: [], excluidas: [] });
  s = alternarNodo(D, { elegidas: [D], excluidas: [] }, W);
  igual("desmarcar una elegida suelta no la excluye", s, { elegidas: [], excluidas: [] });
  s = alternarNodo(U, { elegidas: [D, "C:\\Users\\luis"], excluidas: [`${D}\\Viejo`] }, W);
  igual("marcar la de arriba recoge las de dentro", s, { elegidas: ["C:\\Users\\luis", U], excluidas: [] });
  s = quitarExclusion(V, { elegidas: [U], excluidas: [V] }, W);
  igual("quitar una exclusión desde la lista", s, { elegidas: [U], excluidas: [] });
  igual(
    "«No copiar» final: las reglas se quedan; las rutas sueltas fuera de lo elegido, no",
    exclusionesFinales(["*.tmp", "C:\\Vieja\\x", V], { elegidas: [U], excluidas: [V, "C:\\Vieja\\x", `${D}\\Borradores`] }, W),
    ["*.tmp", V, `${D}\\Borradores`],
  );
  igual("ruta exacta en Linux", [esRutaExacta("/home/ana/tmp", false), esRutaExacta("/home/*/cache", false), esRutaExacta("*.iso", false)], [true, false, false]);
  igual("Linux distingue mayúsculas", dentroDe("/home/Ana/x", "/home/ana", false), false);
  igual("la raíz de un disco contiene todo", dentroDe("C:\\Users", "C:\\", W), true);
}

console.log(`\n${total - fallos} de ${total} bien${fallos ? `, ${fallos} MAL` : ""}.`);
if (fallos) process.exit(1);
