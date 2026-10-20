// Pruebas del bloque D del plan 0.8.1 («Las copias de cada día, más fáciles»; docs/plan-0.8.1.md):
// el nombre que se propone con las carpetas (15t), las etiquetas de las versiones (15v) y
// la retención «Personalizada» desde cero (15z). `npm run test:vectores`.
import type { Configuracion, CopiaConfig, Equipo } from "../src/lib/tipos";
import { alternarGrupo, copiasQueUsan, cuerpoVista, errorTamano, leerVista, ponerAlDia, ponerSueltas, sueltas, tamanoEnFrase, usaGrupo } from "../src/lib/exclusionesCopia";
import { copiaNueva, etiquetasEnUso, limpiarEtiquetaVersion, nombrePropuesto, ponerCarpetas } from "../src/lib/copiaGuiada";
import { configParaEnviar, type Admite } from "../src/lib/configEnvio";
import { reglaSinNada, reglaVacia, sinPeriodo } from "../src/lib/retencion";

let fallos = 0;
let total = 0;
function igual(nombre: string, obtenido: unknown, esperado: unknown) {
  total++;
  const ok = JSON.stringify(obtenido) === JSON.stringify(esperado);
  if (!ok) fallos++;
  console.log(`${ok ? "ok  " : "MAL "} ${nombre}${ok ? "" : `\n       obtenido: ${JSON.stringify(obtenido)}\n       esperado: ${JSON.stringify(esperado)}`}`);
}

// --- 15t: el nombre se propone con las carpetas mientras nadie lo cambie a mano ---
{
  igual("una carpeta", nombrePropuesto(["C:\\Users\\Ana\\Documents"], "PC-01"), "Documents de PC-01");
  igual("dos carpetas", nombrePropuesto(["C:\\Users\\Ana\\Documents", "D:\\Fotos\\"], "PC-01"), "Documents y Fotos de PC-01");
  igual("tres o más", nombrePropuesto(["/home/ana/docs", "/srv/a", "/srv/b"], "SRV-01"), "docs y 2 más de SRV-01");
  igual("una unidad entera", nombrePropuesto(["D:\\"], "PC-01"), "D: de PC-01");
  igual("sin carpetas, la de una copia nueva", nombrePropuesto([], "PC-01"), "Copia nueva");
  igual("nunca más de 60", [...nombrePropuesto(["C:\\" + "x".repeat(90)], "PC-01")].length <= 60, true);
  const k = copiaNueva("r1", "k1");
  ponerCarpetas(k, ["C:\\Users\\Ana\\Documents"], "PC-01");
  igual("la nueva toma la propuesta", k.nombre, "Documents de PC-01");
  ponerCarpetas(k, ["C:\\Users\\Ana\\Documents", "C:\\Users\\Ana\\Desktop"], "PC-01");
  igual("y la sigue mientras sea la propuesta", k.nombre, "Documents y Desktop de PC-01");
  k.nombre = "Contabilidad";
  ponerCarpetas(k, ["C:\\Conta"], "PC-01");
  igual("una puesta a mano no se toca", [k.nombre, k.carpetas], ["Contabilidad", ["C:\\Conta"]]);
}

// --- 15v: las etiquetas de las versiones ---
{
  igual("espacios a guiones", limpiarEtiquetaVersion(" bases  sql "), { etiqueta: "bases-sql" });
  igual("sin comas", limpiarEtiquetaVersion("a,b"), { etiqueta: "ab" });
  igual("vacía, no", "error" in limpiarEtiquetaVersion("  "), true);
  igual("de más de 40, no", "error" in limpiarEtiquetaVersion("x".repeat(41)), true);
  const eq = (ets: string[][]) => ({ resumen: { copias: ets.map((e) => ({ etiquetas: e })) } });
  igual("en uso, la más usada primero", etiquetasEnUso([eq([["bases"], ["binarios", "bases"]]), eq([["binarios"], ["bases"]])]), ["bases", "binarios"]);
  const a: Admite = { reglas: true, ganchos: true, soloCambios: true, verif: false, verifHorario: false, escritorio: false, etiquetas: true };
  const cfg = (et?: string[]) => ({ v: 1, copias: [{ ...copiaNueva("r1", "k1"), carpetas: ["C:\\D"], ...(et ? { etiquetas: et } : {}) }], repositorios: [], destinos: [] }) as unknown as Configuracion;
  igual("se mandan si el agente las entiende", (configParaEnviar(cfg(["bases"]), a).copias[0] as { etiquetas?: string[] }).etiquetas, ["bases"]);
  igual("sin ninguna, no va el campo", "etiquetas" in configParaEnviar(cfg([]), a).copias[0], false);
  igual("a un agente anterior, no", "etiquetas" in configParaEnviar(cfg(["bases"]), { ...a, etiquetas: false }).copias[0], false);
}

// --- 15z: «Personalizar» desde cero, «Vaciar» y la «×» de cada fila ---
{
  igual("vacía no guarda nada", reglaSinNada(reglaVacia()), true);
  const r = { diarias: 7, semanales: 0, mensuales: 0, anuales: 0, plazos: { horarias: "15d", diarias: "1y" } };
  igual("quitar las diarias deja el plazo de las horarias", sinPeriodo(r, "diarias"), { diarias: 0, semanales: 0, mensuales: 0, anuales: 0, plazos: { horarias: "15d" } });
  igual("quitar todas deja la regla vacía", reglaSinNada(sinPeriodo(sinPeriodo(r, "diarias"), "horarias")), true);
  igual("sin tocar la original", r.plazos, { horarias: "15d", diarias: "1y" });
}

// --- 15y: grupos de exclusiones en una copia, tamaño máximo y vista previa ---
{
  const musica = { id: "musica", nombre: "Música", reglas: ["*.mp3", "*.flac"] };
  const temp = { id: "temporales", nombre: "Archivos temporales", reglas: ["*.tmp", "~$*"] };
  const k = { ...copiaNueva("r1", "k1"), exclusiones: ["*.tmp", "C:\\Datos\\cache"] } as CopiaConfig;
  alternarGrupo(k, temp, true);
  igual("activar un grupo añade sus reglas sin repetir", k.exclusiones, ["*.tmp", "C:\\Datos\\cache", "~$*"]);
  alternarGrupo(k, musica, true);
  igual("y queda anotado", [usaGrupo(k, "temporales"), usaGrupo(k, "musica")], [true, true]);
  igual("las sueltas son las que no pone ningún grupo", sueltas(k), ["C:\\Datos\\cache"]);
  ponerSueltas(k, ["C:\\Datos\\cache", "*.iso", " "]);
  igual("cambiar las sueltas deja las de los grupos", k.exclusiones, ["*.tmp", "~$*", "*.mp3", "*.flac", "C:\\Datos\\cache", "*.iso"]);
  alternarGrupo(k, temp, false);
  igual("quitar un grupo quita sus reglas", k.exclusiones, ["*.mp3", "*.flac", "C:\\Datos\\cache", "*.iso"]);
  const musica2 = { ...musica, reglas: ["*.mp3", "*.m4a"] };
  igual("poner al día cambia las reglas del grupo", [ponerAlDia(k, musica2), k.exclusiones], [true, ["C:\\Datos\\cache", "*.iso", "*.mp3", "*.m4a"]]);
  igual("uno que no usa, no", ponerAlDia(k, temp), false);
  const eq = (copias: unknown[]) => ({ id: "e", nombre: "PC-01", resumen: { copias } }) as unknown as Equipo;
  const usos = copiasQueUsan([eq([{ id: "a", nombre: "A", grupos_exclusiones: [{ ...musica }] }, { id: "b", nombre: "B" }]), eq([{ id: "c", nombre: "C", grupos_exclusiones: [{ ...musica2 }] }])], musica2);
  igual("qué copias usan un grupo y si están al día", usos.map((x) => [x.copia.id, x.alDia]), [["a", false], ["c", true]]);
  igual("tamaño en frase", tamanoEnFrase({ mayores_mb: 2048, excepto_tamano: ["*.pst", "*.bak"] }), "nada de más de 2 GB, salvo *.pst, *.bak");
  igual("sin límite, nada", tamanoEnFrase({ mayores_mb: null }), null);
  igual("límite no válido", errorTamano({ nombre: "K", mayores_mb: 0 }) !== null, true);
  igual("el cuerpo de la vista previa", cuerpoVista({ ...k, carpetas: ["C:\\Datos"], mayores_mb: 500, excepto_tamano: ["*.pst"] }, [musica2, temp]), {
    carpetas: ["C:\\Datos"],
    grupos: [{ id: "musica", reglas: ["*.mp3", "*.m4a"] }, { id: "temporales", reglas: ["*.tmp", "~$*"] }],
    sueltas: ["C:\\Datos\\cache", "*.iso"],
    mayores_mb: 500,
    excepto: ["*.pst"],
  });
  igual("una respuesta rara, a cero", leerVista({ grupos: [{ id: "x" }], completo: "sí" }), { total: { archivos: 0, bytes: 0 }, grupos: [{ id: "x", archivos: 0, bytes: 0 }], sueltas: { archivos: 0, bytes: 0 }, tamano: null, quedan: { archivos: 0, bytes: 0 }, completo: false });
  const a: Admite = { reglas: true, ganchos: true, soloCambios: true, verif: false, verifHorario: false, escritorio: false, grupos: true };
  const c1 = { v: 1, copias: [{ ...k, carpetas: ["C:\\D"], mayores_mb: 500, excepto_tamano: ["*.pst"] }], repositorios: [], destinos: [] } as unknown as Configuracion;
  const x = configParaEnviar(c1, a).copias[0] as Record<string, unknown>;
  igual("con el agente que lo entiende, van los grupos y el tamaño", [!!x.grupos_exclusiones, x.mayores_mb, x.excepto_tamano], [true, 500, ["*.pst"]]);
  const y = configParaEnviar(c1, { ...a, grupos: false }).copias[0] as Record<string, unknown>;
  igual("a uno anterior, solo las reglas", ["grupos_exclusiones" in y, "mayores_mb" in y, (y.exclusiones as string[]).includes("*.m4a")], [false, false, true]);
}

console.log(`\n${total - fallos}/${total} ${fallos ? "con fallos" : "bien"}`);
if (fallos) process.exit(1);
