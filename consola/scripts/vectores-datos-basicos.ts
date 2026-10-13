// Pruebas de los datos básicos del equipo (src/lib/datosBasicos.ts, plan-mejoras 14uu):
// lo que manda el agente se comprueba, el sistema en palabras y agrupado, el uso de
// los discos (con el criterio del aviso del servidor) y el reinicio antes de un fallo.
//
//   npm run test:vectores
import { casiLleno, datosBasicos, textoMemoria, grupoSistema, MAX_DISCOS, reinicioAntes, sistemaDe, textoBuscable, textoModelo, textoProcesador, textoSistema, usoDisco } from "../src/lib/datosBasicos";

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

const G = 1_000_000_000;
const eq = (datos_basicos: unknown, so = "Windows x86_64") => ({ so, resumen: { datos_basicos } }) as never;

console.log("\n— Agente anterior y datos raros —");
igual("sin resumen", datosBasicos({ resumen: null } as never), null);
igual("sin el campo", datosBasicos({ resumen: {} } as never), null);
igual("un texto en vez de objeto", datosBasicos(eq("hola")), null);
igual("una lista en vez de objeto", datosBasicos(eq([1, 2])), null);
igual("sistema exacto de un agente anterior: el de siempre", sistemaDe(eq(undefined)), "Windows x86_64");

const raro = datosBasicos(
  eq({
    sistema: { familia: "windows", nombre: "Windows 11 Pro\u0007" + "x".repeat(500), version: 3 },
    procesador: { nombre: "  Intel(R)   Core(TM) i5  ", nucleos: -2 },
    memoria: "16 GB",
    arranque: "no es una fecha",
    discos: [
      { punto: "C:", total: 100 * G, libre: 20 * G },
      { punto: "D:", total: 0, libre: 0 },
      { punto: "E:", total: 10, libre: 20 },
      { punto: "", total: 10, libre: 1 },
      { punto: "F:", total: Infinity, libre: 1 },
      "x",
      null,
    ],
  }),
);
igual("los textos se limpian y recortan", raro?.sistema?.nombre?.length, 120);
igual("sin caracteres de control", raro?.sistema?.nombre?.includes("\u0007"), false);
igual("versión que no es texto: fuera", raro?.sistema?.version, null);
igual("procesador con espacios juntados", raro?.procesador?.nombre, "Intel(R) Core(TM) i5");
igual("núcleos negativos: fuera", raro?.procesador?.nucleos, null);
igual("memoria que no es número: fuera", raro?.memoria, null);
igual("arranque que no es fecha: fuera", raro?.arranque, null);
igual("solo los discos que cuadran", raro?.discos?.map((d) => d.punto), ["C:"]);
igual("como mucho 32 discos", datosBasicos(eq({ discos: Array.from({ length: 80 }, (_, i) => ({ punto: `/m${i}`, total: 10, libre: 1 })) }))?.discos?.length, MAX_DISCOS);

console.log("\n— El sistema en palabras —");
const w11 = eq({ sistema: { familia: "windows", nombre: "Windows 11 Pro", variante: "Professional", version: "24H2", compilacion: "26100.2033" } });
igual("Windows con su versión", sistemaDe(w11), "Windows 11 Pro 24H2");
igual("grupo de Windows 11", grupoSistema(w11), "Windows 11");
const srv = eq({ sistema: { familia: "windows", nombre: "Windows Server 2012 R2 Standard", version: null } });
igual("Windows Server sin versión", textoSistema(datosBasicos(srv)), "Windows Server 2012 R2 Standard");
igual("grupo de Windows Server 2012 R2", grupoSistema(srv), "Windows Server 2012 R2");
igual("grupo de Windows Server 2019", grupoSistema(eq({ sistema: { familia: "windows", nombre: "Windows Server 2019 Datacenter", version: "1809" } })), "Windows Server 2019");
const ubu = eq({ sistema: { familia: "linux", nombre: "Ubuntu 22.04.4 LTS", id: "ubuntu", version: "22.04", kernel: "5.15.0-105-generic" } }, "Linux x86_64");
igual("Linux con su nombre bonito", sistemaDe(ubu), "Ubuntu 22.04.4 LTS");
igual("grupo de Ubuntu", grupoSistema(ubu), "Ubuntu 22.04");
igual("grupo de Debian", grupoSistema(eq({ sistema: { familia: "linux", nombre: "Debian GNU/Linux 12 (bookworm)", id: "debian", version: "12" } })), "Debian 12");
igual("se busca por sistema y edición", /windows 11/i.test(textoBuscable(w11)) && /professional/i.test(textoBuscable(w11)), true);

console.log("\n— Procesador y modelo —");
igual("procesador con núcleos", textoProcesador(datosBasicos(eq({ procesador: { nombre: "Intel Core i5-8500", nucleos: 6 } }))), "Intel Core i5-8500 · 6 núcleos");
igual("un núcleo", textoProcesador(datosBasicos(eq({ procesador: { nucleos: 1 } }))), "1 núcleo");
igual("modelo con fabricante", textoModelo(datosBasicos(eq({ fabricante: "Dell Inc.", modelo: "OptiPlex 7070" }))), "Dell Inc. OptiPlex 7070");
igual("el modelo ya lleva el fabricante", textoModelo(datosBasicos(eq({ fabricante: "HP", modelo: "HP ProLiant DL380 Gen10" }))), "HP ProLiant DL380 Gen10");
igual("memoria en potencias de 2", textoMemoria(datosBasicos(eq({ memoria: 16 * 1024 ** 3 }))), "16 GB");
igual("memoria pequeña", textoMemoria(datosBasicos(eq({ memoria: 3.5 * 1024 ** 3 }))), "3,5 GB");
igual("sin nada", textoModelo(datosBasicos(eq({}))), null);

console.log("\n— Uso de los discos (como el aviso del servidor) —");
igual("casi lleno: < 10 % y < 10 GiB", usoDisco({ punto: "C:", total: 120 * G, libre: 5 * G }), { usado: 96, tono: "bad" });
igual("poco libre pero más de 10 GiB: aviso suave", usoDisco({ punto: "D:", total: 2000 * G, libre: 150 * G }), { usado: 93, tono: "warn" });
igual("menos de 10 GiB pero más del 10 %: aviso suave", usoDisco({ punto: "E:", total: 50 * G, libre: 6 * G }), { usado: 88, tono: "warn" });
igual("con aire", usoDisco({ punto: "/", total: 100 * G, libre: 60 * G }), { usado: 40, tono: "ok" });
igual("casiLleno igual que el tono", casiLleno({ punto: "C:", total: 120 * G, libre: 5 * G }), true);

console.log("\n— Reinicio antes de un fallo —");
const d = datosBasicos(eq({ arranque: "2026-10-13T11:11:00+02:00" }));
const hora = new Intl.DateTimeFormat("es", { hour: "2-digit", minute: "2-digit" }).format(new Date("2026-10-13T11:11:00+02:00"));
igual("arrancó 40 min antes", reinicioAntes(d, "2026-10-13T11:51:00+02:00"), `El equipo se reinició a las ${hora}.`);
igual("arrancó 3 h antes: no se dice", reinicioAntes(d, "2026-10-13T14:30:00+02:00"), null);
igual("arrancó después del fallo: no se dice", reinicioAntes(d, "2026-10-13T10:00:00+02:00"), null);
igual("sin arranque", reinicioAntes(datosBasicos(eq({})), "2026-10-13T11:51:00+02:00"), null);
igual("otro día: con la fecha", reinicioAntes(datosBasicos(eq({ arranque: "2026-10-12T23:50:00Z" })), "2026-10-13T00:30:00Z")?.startsWith("El equipo se reinició"), true);

console.log(`\n${total - fallos} de ${total} comprobaciones correctas.`);
if (fallos) process.exit(1);
