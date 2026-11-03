// Pruebas de la rama `ia/083-rendimiento` (plan 0.8.3): 16x (los informes nuevos con
// `desde`) y 16v («Comprobar todo» de un cliente). `npm run test:vectores`.
import type { Equipo, Informe, Orden } from "../src/lib/tipos";
import type { ReglaCopia } from "../src/lib/regla321";
import { desdeInformes } from "../src/lib/informesDesde";
import { cuentas, informeCsv, kitsDe, planComprobar, resultadoDe } from "../src/lib/comprobarTodo";

let fallos = 0;
let total = 0;
function igual(nombre: string, obtenido: unknown, esperado: unknown) {
  total++;
  const ok = JSON.stringify(obtenido) === JSON.stringify(esperado);
  if (!ok) fallos++;
  console.log(`${ok ? "ok  " : "MAL "} ${nombre}${ok ? "" : `\n       obtenido: ${JSON.stringify(obtenido)}\n       esperado: ${JSON.stringify(esperado)}`}`);
}

console.log("\n· 16x: desde cuándo pedir los informes nuevos");
{
  const inf = (recibido: string) => ({ recibido, datos: {} }) as Informe;
  igual("sin informes: todos", desdeInformes({}), undefined);
  igual("el más reciente (con zonas distintas)", desdeInformes({ a: inf("2026-11-02T10:00:00Z"), b: inf("2026-11-02T06:30:00-05:00"), c: null }), "2026-11-02T06:30:00-05:00");
  igual("una fecha que no vale no cuenta", desdeInformes({ a: inf("ayer"), b: inf("2026-11-01T00:00:00Z") }), "2026-11-01T00:00:00Z");
}

console.log("\n· 16v: lo que se comprueba");
const ahora = Date.parse("2026-11-02T12:00:00Z");
const hace = (min: number) => new Date(ahora - min * 60_000).toISOString();
const equipo = (id: string, extra: Partial<Equipo> = {}) =>
  ({
    id,
    nombre: id.toUpperCase(),
    so: "Windows 11 Pro",
    modo: "gestionado",
    confirmado: true,
    conectado: true,
    siguiente_seq: 5,
    resumen: {
      admite: ["prueba_espejo", "comprobar_contrasena"],
      repositorios: [
        { id: "docs", nombre: "Documentos", destino: "x", versiones: 12, derivadas: [{ id: "d1", nombre: "Docs a la nube", destino: "Nube" }, { id: "d2", activa: false }] },
        { id: "vacio", nombre: "Vacío", destino: "x", versiones: 0 },
        { id: "ajeno", nombre: "De otro", destino: "x", versiones: 4, solo_lectura: true },
      ],
    },
    ...extra,
  }) as unknown as Equipo;
const pc = equipo("pc-01");
const viejo = equipo("pc-02", { resumen: { admite: [], repositorios: [{ id: "r", nombre: "Ventas", destino: "x", versiones: 3, contrasena_comprobada: { ultima: hace(60), resultado: "abre" } }] } } as unknown as Partial<Equipo>);
const fuera = equipo("pc-03", { modo: "trasladado" });
const sinConfirmar = equipo("pc-04", { confirmado: false });
// Un espejo que hace el propio equipo (listo) y otro en una nube conectada solo en el almacén (no).
const paso = (nombre: string, espejo: Record<string, unknown>) => ({ tipo: "espejo", nombre, clave: null, espejo }) as unknown;
const regla = { equipo: pc, repo: pc.resumen!.repositorios![0], pasos: [paso("USB de la oficina", { quien: "equipo", trabajo: "t1", tipo: "carpeta", carpeta: "F:\\Espejo", ruta: "docs" }), paso("Nube del almacén", { quien: "almacen", almacen: "alm", tipo: "nube", carpeta: "espejo", ruta: "u/docs" })] } as unknown as ReglaCopia;
const plan = planComprobar([pc, viejo, fuera, sinConfirmar], [regla, regla]);
igual(
  "por equipo: repositorio, derivadas activas y espejos (sin repetir); ni trasladados ni sin confirmar",
  plan.map((c) => `${c.equipo.id}:${c.clase}:${c.nombre}:${c.orden ? c.orden.tipo : "-"}`),
  [
    "pc-01:repo:Documentos:verificar_ahora",
    "pc-01:derivada:Docs a la nube:verificar_ahora",
    "pc-01:espejo:USB de la oficina:probar_restauracion",
    "pc-01:espejo:Nube del almacén:-",
    "pc-01:repo:Vacío:-",
    "pc-01:repo:De otro:-",
    "pc-02:repo:Ventas:verificar_ahora",
  ],
);
igual("la derivada lleva su id", plan[1].orden?.cuerpo, { repo: "docs", derivada: "d1" });
igual("el espejo listo lleva la orden de 14j", (plan[2].orden?.cuerpo as { espejo: { trabajo: string } }).espejo.trabajo, "t1");
igual("la nube del almacén dice por qué no", plan[3].nota.startsWith("Esa nube está conectada en «el almacén»"), true);
const sinPrueba = planComprobar([equipo("pc-05", { resumen: { ...pc.resumen!, admite: [] } } as Partial<Equipo>)], [{ ...regla, equipo: equipo("pc-05") } as ReglaCopia]);
igual("un agente sin prueba_espejo: el espejo no se manda, con el motivo", sinPrueba.filter((c) => c.clase === "espejo").map((c) => [c.orden, c.nota.startsWith("Actualiza Resguardo")]), [
  [null, true],
  [null, true],
]);

console.log("\n· 16v: el veredicto de cada una");
{
  const desde = ahora - 5 * 60_000;
  const [repo, deriv, esp] = plan;
  const informe = (repos: unknown[]) => ({ recibido: hace(1), datos: { repos } }) as unknown as Informe;
  const orden = (estado: string, mensaje: string | null = null) => ({ id: "o", estado, mensaje }) as Orden;
  igual("sin mandar: sin pedir", resultadoDe(repo, null, null, pc).veredicto, "esperando");
  igual("una que no se manda: su motivo", resultadoDe(plan[4], desde, null, pc), { veredicto: "no_se_manda", texto: "Aún no tiene versiones: no hay nada que verificar.", cuando: null });
  igual("pedida y sin respuesta", resultadoDe(repo, desde, null, pc, orden("entregada")).texto, "Pedida: esperando al equipo…");
  igual("rechazada: mal con su motivo", resultadoDe(repo, desde, null, pc, orden("rechazada", "No es de esta consola")), { veredicto: "mal", texto: "No es de esta consola", cuando: null });
  igual("sin conexión: lo hará al volver", resultadoDe(repo, desde, null, { ...pc, conectado: false }, orden("pendiente")).texto, "Sin conexión: lo hará cuando vuelva.");
  const vieja = informe([{ id: "docs", verificacion: { ultima: hace(60 * 24), resultado: "ok", mensaje_corto: "Sin errores" } }]);
  igual("una verificación de antes no cuenta", resultadoDe(repo, desde, vieja, pc, orden("hecha")).veredicto, "esperando");
  const bien = informe([{ id: "docs", verificacion: { ultima: hace(2), resultado: "ok", mensaje_corto: "Sin errores" }, derivadas: [{ id: "d1", verificacion: { ultima: hace(1), resultado: "fallo", mensaje_corto: "Datos dañados" } }], pruebas_espejo: [{ id: plan[2].id, nombre: "USB", ultima: hace(1), resultado: "ok", mensaje_corto: "20 archivos iguales" }] }]);
  igual("verificada después: bien", resultadoDe(repo, desde, bien, pc, orden("hecha")), { veredicto: "bien", texto: "Sin errores", cuando: hace(2) });
  igual("la derivada falló", resultadoDe(deriv, desde, bien, pc, orden("hecha")).veredicto, "mal");
  igual("la prueba desde el espejo", resultadoDe(esp, desde, bien, pc, orden("hecha")).texto, "20 archivos iguales");
  const adelantado = informe([{ id: "docs", verificacion: { ultima: hace(0.5), resultado: "ok", mensaje_corto: "Sin errores" } }]);
  igual("un minuto de margen de reloj (el del equipo va 30 s atrás)", resultadoDe(repo, ahora, adelantado, pc, orden("hecha")).veredicto, "bien");
  const resumen = { ...pc, resumen: { ...pc.resumen!, repositorios: [{ ...pc.resumen!.repositorios![0], verificado: hace(1) }] } } as Equipo;
  igual("sin informe nuevo, la fecha del resumen", resultadoDe(repo, desde, vieja, resumen, orden("hecha")).veredicto, "bien");
  igual("las cuentas", cuentas([resultadoDe(repo, desde, bien, pc), resultadoDe(deriv, desde, bien, pc), resultadoDe(plan[4], desde, null, pc), resultadoDe(repo, desde, null, pc)]), { bien: 1, aviso: 0, mal: 1, esperando: 1, no_se_manda: 1 });
}

console.log("\n· 16v: los kits y el informe");
{
  const ks = kitsDe([pc, viejo]);
  igual("los kits sin los de solo lectura, con su última comprobación", ks.map((k) => [k.equipo.id, k.repo.id, k.admite, k.resultado]), [
    ["pc-01", "docs", true, null],
    ["pc-01", "vacio", true, null],
    ["pc-02", "r", false, "abre"],
  ]);
  const raro = equipo("=HYPERLINK(1)");
  const texto = informeCsv([{ c: { ...plan[0], equipo: raro }, r: { veredicto: "bien", texto: "Sin errores; todo bien", cuando: null } }], ks.slice(2));
  const lineas = texto.replace(/^\uFEFF/, "").trim().split("\r\n");
  igual("CSV: cabecera", lineas[0], "Equipo;Qué;Nombre;Repositorio;Resultado;Detalle;Cuándo");
  igual("CSV: sin fórmulas y con «;» entre comillas", lineas[1], "'=HYPERLINK(1);Repositorio;Documentos;Documentos;Bien;\"Sin errores; todo bien\";");
  igual("CSV: el kit", lineas[2].split(";").slice(0, 5), ["PC-02", "Kit (contraseña)", "Ventas", "Ventas", "La abre"]);
}

console.log(`\n${total - fallos} de ${total} bien`);
if (fallos) process.exit(1);
