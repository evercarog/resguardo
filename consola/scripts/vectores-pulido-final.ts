// Pruebas del pulido final de la 0.8.0: la contraseña de un repositorio nuevo
// (14ii, src/lib/contrasenaRepo.ts) y el explorador de carpetas del equipo
// (14zz, src/lib/arbolCarpetas.ts). `npm run test:vectores` (con las demás).
import { claveElegida, errorContrasena, generarContrasena, MIN_ESCRITA, nuevaContrasena, olvidarContrasena } from "../src/lib/contrasenaRepo";

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

console.log(`\n${total - fallos} de ${total} bien${fallos ? `, ${fallos} MAL` : ""}.`);
if (fallos) process.exit(1);
