// Pruebas del editor de copias guiado (plan 0.7.26, bloque 3; src/lib/copiaGuiada.ts
// y src/lib/espejosCopia.ts; docs/editor-de-copias.md). `npm run test:vectores`.
//
// Lo más importante: el modo guiado y el «Avanzado» producen **la misma
// configuración**. Cada escenario se hace por los dos caminos (los atajos del
// guiado y lo que escriben los controles del editor completo, en otro orden) y
// se compara lo que se enviaría al equipo.
import type { Configuracion, CopiaConfig, DestinoResumen, Equipo, ReglaHorario } from "../src/lib/tipos";
import {
  aplicarPlantilla,
  copiaNueva,
  cuandoEnFrase,
  duplicarCopia,
  errorInicio,
  inicioDe,
  limpiarInicio,
  pasoHecho,
  plantillaDe,
  ponerInicio,
  ponerRetraso,
  primerPaso,
  queEnFrase,
  reglasDePlantilla,
  retrasoEnFrase,
  sugerencias,
  alternarExclusion,
} from "../src/lib/copiaGuiada";
import { espejosDeCopia } from "../src/lib/espejosCopia";
import { horarioParaEnviar } from "../src/lib/horario";
import { configParaEnviar, type Admite } from "../src/lib/configEnvio";
import { moverA, reenlazar } from "../src/lib/cadenas";

let fallos = 0;
let total = 0;
function igual(nombre: string, obtenido: unknown, esperado: unknown) {
  total++;
  const ok = JSON.stringify(obtenido) === JSON.stringify(esperado);
  if (!ok) fallos++;
  console.log(`${ok ? "ok  " : "MAL "} ${nombre}${ok ? "" : `\n       obtenido: ${JSON.stringify(obtenido)}\n       esperado: ${JSON.stringify(esperado)}`}`);
}
const cierto = (nombre: string, v: boolean) => igual(nombre, v, true);

const NUEVO: Admite = { reglas: true, ganchos: true, soloCambios: true, verif: true, verifHorario: true, escritorio: true, cadenas: true, pruebas: true, despues: true };
const VIEJO: Admite = { ...NUEVO, despues: false };
const ANTIGUO: Admite = { ...VIEJO, reglas: false, cadenas: false };
const cfg = (copias: CopiaConfig[]): Configuracion => ({ v: 1, copias, repositorios: [{ id: "r1", nombre: "Documentos", destino: "d1" }], destinos: [] });
const enviar = (copias: CopiaConfig[], a: Admite = NUEVO) => configParaEnviar(JSON.parse(JSON.stringify(cfg(copias))) as Configuracion, a);
/** Lo que escribe el editor completo (`EditorHorario`) con unas reglas. */
const editorCompleto = (k: CopiaConfig, reglas: ReglaHorario[], admiteReglas: boolean) => (k.horario = horarioParaEnviar(reglas, admiteReglas));
/** Lo que escribe la caja de texto de las carpetas (una por línea). */
const lineas = (t: string) => t.split(/\r?\n/).map((x) => x.trim()).filter(Boolean);

console.log("\n· Guiado = Avanzado (la misma configuración)");
{
  // Escenario 1: «Laborables a las 21:00», dos carpetas, repositorio r1, sin los bloqueos de Office.
  const g = copiaNueva("", "k1");
  aplicarPlantilla(g, "laborables", "21:00", true); // Cuándo
  g.carpetas = ["C:\\Datos", "C:\\Contabilidad"]; // Qué
  alternarExclusion(g, "~$*"); // Qué → Más opciones
  g.repo = "r1"; // Dónde
  const a = copiaNueva("", "k1");
  a.repo = "r1"; // el selector, primero
  a.carpetas = lineas("C:\\Datos\n\nC:\\Contabilidad\n");
  a.exclusiones = lineas("*.tmp\nThumbs.db");
  editorCompleto(a, [{ tipo: "horas", dias: [1, 2, 3, 4, 5], horas: ["21:00"] }], true);
  igual("laborables a las 21:00", enviar([g]), enviar([a]));

  // Escenario 2: «Cada hora», con un agente nuevo y con uno anterior a la 0.7.9 (sin reglas).
  for (const [admite, nombre] of [
    [NUEVO, "agente nuevo"],
    [ANTIGUO, "agente anterior a la 0.7.9"],
  ] as const) {
    const g2 = copiaNueva("r1", "k1");
    aplicarPlantilla(g2, "cada_hora", "", admite.reglas);
    const a2 = copiaNueva("r1", "k1");
    editorCompleto(a2, [{ tipo: "intervalo", dias: [1, 2, 3, 4, 5, 6, 7], cada_min: 60, desde: "00:00", hasta: "23:00" }], admite.reglas);
    igual(`cada hora (${nombre})`, enviar([g2], admite), enviar([a2], admite));
    igual(`cada hora: 24 horas cualquier día, sin reglas (${nombre})`, [enviar([g2], admite).copias[0].horario.horas.length, enviar([g2], admite).copias[0].horario.dias, "reglas" in enviar([g2], admite).copias[0].horario], [24, [1, 2, 3, 4, 5, 6, 7], false]);
  }

  // Escenario 3: una segunda copia «Después de la anterior», 15 min de retraso. El guiado elige
  // primero cuándo; el avanzado cambia antes el horario y después cómo empieza: da igual.
  const g1 = copiaNueva("r1", "docs");
  const g3 = copiaNueva("r1", "fotos");
  const lg = [g1, g3];
  ponerInicio(lg, 1, "despues");
  ponerRetraso(g3, 15);
  g3.carpetas = ["D:\\Fotos"];
  const a1 = copiaNueva("r1", "docs");
  const a3 = copiaNueva("r1", "fotos");
  const la = [a1, a3];
  a3.carpetas = ["D:\\Fotos"];
  editorCompleto(a3, [{ tipo: "horas", dias: [1, 2, 3, 4, 5, 6, 7], horas: ["22:00"] }], true);
  ponerInicio(la, 1, "cadena");
  ponerInicio(la, 1, "despues");
  ponerRetraso(a3, 15);
  igual("después de la anterior con 15 min", enviar(lg), enviar(la));
  igual("…lo que va al equipo", enviar(lg).copias[1], {
    id: "fotos",
    nombre: "Nueva copia",
    repo: "r1",
    carpetas: ["D:\\Fotos"],
    exclusiones: ["*.tmp", "~$*", "Thumbs.db"],
    horario: { dias: [], horas: [] },
    activa: true,
    gancho: null,
    solo_si_cambios: true,
    tras: "docs",
    inicio: "despues",
    retraso_min: 15,
  });

  // Escenario 4: en cadena e inmediatamente: lo de siempre (sin `inicio` ni `retraso_min`).
  const c1 = copiaNueva("r1", "docs");
  const c2 = copiaNueva("r1", "disco-e");
  ponerInicio([c1, c2], 1, "cadena");
  ponerRetraso(c2, 0);
  const env = enviar([c1, c2]).copias[1] as Record<string, unknown>;
  igual("en cadena: solo `tras`", [env.tras, "inicio" in env, "retraso_min" in env], ["docs", false, false]);
}

console.log("\n· Cuándo empieza");
{
  const l = [copiaNueva("r1", "a"), copiaNueva("r1", "b"), copiaNueva("r1", "c")];
  l[0].nombre = "Documentos";
  ponerInicio(l, 1, "despues");
  igual("después: sin horario propio", [inicioDe(l[1]), l[1].horario], ["despues", { dias: [], horas: [] }]);
  ponerInicio(l, 0, "cadena");
  igual("la primera no puede ir en cadena", [inicioDe(l[0]), l[0].tras], ["horario", null]);
  ponerInicio(l, 1, "horario", { dias: [6], horas: ["10:00"] });
  igual("vuelve a «Con horario» con el que tenía", [inicioDe(l[1]), l[1].horario, "inicio" in l[1]], ["horario", { dias: [6], horas: ["10:00"] }, false]);
  ponerInicio(l, 2, "cadena");
  ponerRetraso(l[2], 30);
  igual("en frase (en cadena)", cuandoEnFrase(l, 2), "En cadena tras «Nueva copia» · con 30 min de retraso");
  ponerInicio(l, 2, "despues");
  ponerRetraso(l[2], 0);
  igual("en frase (después)", cuandoEnFrase(l, 2), "Después de «Nueva copia» · inmediatamente");
  igual("en frase (horario)", cuandoEnFrase(l, 0), "Cada día laborable a las 13:00");
  igual("retrasos", [retrasoEnFrase(0), retrasoEnFrase(15), retrasoEnFrase(120)], ["Inmediatamente", "Con 15 min de retraso", "Con 2 h de retraso"]);
  ponerRetraso(l[0], 15);
  igual("sin `tras`, no hay retraso", "retraso_min" in l[0], false);
  ponerRetraso(l[2], 99999);
  igual("el retraso, como mucho un día", l[2].retraso_min, 1440);

  const a = { cadenas: true, despues: true };
  igual("errores: bien", errorInicio(l, 2, a), null);
  igual("errores: agente sin «después»", errorInicio(l, 2, { cadenas: true, despues: false }), "«Nueva copia»: actualiza el agente para «Después de la anterior» o un retraso.");
  igual("errores: agente sin cadenas", errorInicio(l, 2, { cadenas: false, despues: false }), "«Nueva copia» va después de otra copia y el agente de este equipo aún no lo admite.");
  igual("errores: la primera en cadena", errorInicio([{ id: "x", nombre: "X", tras: "y" }], 0, a), "«X» es la primera: empieza con su horario.");

  // Al ordenar: la que llega arriba pierde el «después» y toma horario.
  const antes = $snapshot(l);
  const r = reenlazar(antes, moverA(antes, 2, 0));
  limpiarInicio(r.copias);
  igual("ordenar: la de arriba, con horario y sin `inicio`", [r.aHorario, inicioDe(r.copias[0]), "inicio" in r.copias[0], r.copias[0].horario.horas.length > 0], [["c"], "horario", false, true]);

  // Un agente anterior: ni `inicio` ni `retraso_min` (la consola no los ofrece; si llegan, no se mandan).
  const env = enviar(l, VIEJO).copias[2] as Record<string, unknown>;
  igual("agente sin «inicio_despues»: solo `tras`", [env.tras, "inicio" in env, "retraso_min" in env], ["b", false, false]);
  const env2 = enviar(l, ANTIGUO).copias[2] as Record<string, unknown>;
  igual("agente sin cadenas: ni `tras`", "tras" in env2, false);
}

console.log("\n· Plantillas del horario");
for (const [p, h] of [
  ["cada_hora", ""],
  ["cada_dia", "21:00"],
  ["laborables", "08:30"],
] as const) {
  const k = copiaNueva("r1");
  aplicarPlantilla(k, p, h, true);
  igual(`se reconoce «${p}»`, plantillaDe(k.horario), p === "cada_hora" ? { plantilla: p, hora: "" } : { plantilla: p, hora: h });
}
igual("otra cosa: personalizado", plantillaDe({ dias: [1, 3], horas: ["10:00"] }), { plantilla: "personalizado" });
igual("dos horas: personalizado", plantillaDe({ dias: [1, 2, 3, 4, 5], horas: ["10:00", "18:00"] }), { plantilla: "personalizado" });
igual("la de siempre (lunes a viernes a las 13:00) es «laborables»", plantillaDe(copiaNueva("r1").horario), { plantilla: "laborables", hora: "13:00" });
igual("las reglas de «cada día»", reglasDePlantilla("cada_dia", "22:00"), [{ tipo: "horas", dias: [1, 2, 3, 4, 5, 6, 7], horas: ["22:00"] }]);

console.log("\n· Los pasos");
{
  const k = copiaNueva("");
  igual("una copia nueva empieza en «Qué» (ya tiene horario)", primerPaso(k), "que");
  k.carpetas = ["C:\\Datos"];
  igual("…después, «Dónde»", primerPaso(k), "donde");
  k.repo = "r1";
  igual("…y completa, el resumen", primerPaso(k), "resumen");
  igual("hechos", [pasoHecho(k, "cuando"), pasoHecho(k, "que"), pasoHecho(k, "donde")], [true, true, true]);
  igual("qué, en frase", queEnFrase(k), "1 carpeta · 3 exclusiones");
  igual("sin carpetas", queEnFrase(copiaNueva("")), "Sin carpetas");
  const d = duplicarCopia({ ...k, tras: "otra", inicio: "despues", retraso_min: 5, horario: { dias: [], horas: [] } }, "k2");
  igual("duplicar: con horario y sin cadena", [d.id, d.nombre, d.tras, "inicio" in d, d.horario.horas.length > 0, d.carpetas], ["k2", "Nueva copia (copia)", null, false, true, ["C:\\Datos"]]);
}

console.log("\n· Sugerencias de un clic (regla 3-2-1-1-0)");
const todo = { verificacion: true, prueba: true, espejo: true, derivada: true };
igual("verificación y prueba", sugerencias(["programar_verificacion", "programar_prueba"], todo), ["verificacion", "prueba"]);
igual("fuera del sitio: primero la derivada", sugerencias(["anadir_fuera", "anadir_inmutable"], todo), ["derivada", "espejo"]);
igual("otro destino: espejo y derivada", sugerencias(["anadir_destino"], todo), ["espejo", "derivada"]);
igual("solo lo que se puede", sugerencias(["anadir_fuera", "programar_prueba"], { ...todo, espejo: false, prueba: false }), ["derivada"]);
igual("nada que hacer", sugerencias(["", "poner_al_dia"], todo), []);

console.log("\n· Espejos agrupados por almacén");
{
  const base = { so: "Windows 11", version_agente: "0.7.26", box_pub: "", sign_pub: "", sal_equipo: "", etiqueta: null, modo: "gestionado" as const, confirmado: true, conectado: true, ultimo_contacto: null, estado_servicio: "en_marcha" as const, siguiente_seq: 1 };
  const almacen: Equipo = {
    ...base,
    id: "0b5c1f8e-1d2a-4c3b-9e8f-7a6b5c4d3e2f",
    nombre: "ALMACEN-01",
    rol: "almacenamiento",
    resumen: {
      admite: ["zonas_almacen", "espejo_zonas"],
      guarda_copias: {
        activo: true,
        puerto: 8000,
        carpeta: "D:\\Resguardo",
        zonas: [{ id: "z1a2b3c", nombre: "Disco E", carpeta: "E:\\Resguardo", puerto: 8002, usuarios: 1 }],
        nubes: [{ nombre: "Dropbox Oficina", tipo: "dropbox" }],
        espejo: {
          hora: "02:00",
          destinos: [
            { tipo: "zona", carpeta: "z1a2b3c", repos: ["recepcion/documentos"], tras_copia: true, retencion_dias: 30 },
            { tipo: "nube", nube: "Dropbox Oficina", carpeta: "Resguardo", repos: ["recepcion/documentos"] },
          ],
        },
      },
    },
  };
  const enD: DestinoResumen = { id: "almacen-0b5c1f8e", nombre: "ALMACEN-01", tipo: "rest", donde: "https://192.168.1.20:8000/recepcion/", equipo_almacen: almacen.id };
  const b2: DestinoResumen = { id: "destino-1a2b3c4d", nombre: "B2 de la oficina", tipo: "b2", donde: "copias-sur" };
  const recepcion: Equipo = {
    ...base,
    id: "e-recepcion",
    nombre: "RECEPCION",
    rol: "agente",
    resumen: {
      admite: ["cadenas", "derivadas"],
      destinos: [enD, b2],
      copias: [{ id: "docs", nombre: "Documentos", repo: "documentos" }],
      repositorios: [
        { id: "documentos", nombre: "Documentos", destino: "ALMACEN-01", derivadas: [{ id: "d1", destino: "B2 de la oficina", destino_id: b2.id, cuando: { tras_copia: true } }] },
        { id: "solo", nombre: "Solo", destino: "ALMACEN-01" },
      ],
    },
  };
  const g = espejosDeCopia(recepcion, "documentos", [almacen, recepcion]);
  igual(
    "dos grupos: el almacén (sus espejos) y el equipo (la derivada)",
    g.map((x) => [x.clave, x.quien.nombre, x.quien.almacen, x.pasos.map((p) => p.texto)]),
    [
      [`almacen:${almacen.id}`, "ALMACEN-01", true, ["Almacén ALMACEN-01 · Disco E", "Dropbox Oficina"]],
      ["equipo:e-recepcion", "RECEPCION", false, ["B2 de la oficina"]],
    ],
  );
  igual("sin espejos ni derivadas, nada", espejosDeCopia(recepcion, "solo", [almacen, recepcion]), []);
  igual("un repositorio que no está, nada", espejosDeCopia(recepcion, "otro", [almacen, recepcion]), []);
}

console.log(`\n${total - fallos}/${total} ${fallos ? "con fallos" : "bien"}`);
if (fallos) process.exit(1);

/** Una copia profunda (como `$state.snapshot`). */
function $snapshot<T>(x: T): T {
  return JSON.parse(JSON.stringify(x)) as T;
}
