// Pruebas de las copias en cadena (tarea 7, parte B, src/lib/cadenas.ts;
// docs/copias-en-cadena.md). `npm run test:vectores` (con las demás).
import type { DestinoResumen, Equipo } from "../src/lib/tipos";
import {
  derivadasDe,
  errorCadenas,
  errorFiltro,
  filtroEnFrase,
  filtroParaOrden,
  idDerivadaNueva,
  lineaCadena,
  lineaEnTexto,
  mover,
  pasoDeEspejo,
  posiblesAnteriores,
  recomendarFueraRetencion,
} from "../src/lib/cadenas";
import { claveEspejo, esDestructiva } from "../src/lib/cripto/ordenes";
import { destinoParaOrden } from "../src/lib/espejo";
import { reglaDeCopia } from "../src/lib/regla321";

let fallos = 0;
let total = 0;
function igual(nombre: string, obtenido: unknown, esperado: unknown) {
  total++;
  const ok = JSON.stringify(obtenido) === JSON.stringify(esperado);
  if (!ok) fallos++;
  console.log(`${ok ? "ok  " : "MAL "} ${nombre}${ok ? "" : `\n       obtenido: ${JSON.stringify(obtenido)}\n       esperado: ${JSON.stringify(esperado)}`}`);
}
const cierto = (nombre: string, v: boolean) => igual(nombre, v, true);

console.log("\n· «Después de la anterior» (7c)");
const k = (id: string, tras?: string) => ({ id, nombre: id.toUpperCase(), tras: tras ?? null });
igual("una cadena que vale", errorCadenas([k("docs"), k("disco-e", "docs"), k("nube", "disco-e")]), null);
cierto("ella misma, no", !!errorCadenas([k("a", "a")]));
cierto("una que no está, no", !!errorCadenas([k("a", "zz")]));
cierto("una vuelta, no", errorCadenas([k("a", "c"), k("b", "a"), k("c", "b")])?.includes("círculo") === true);
igual(
  "antes de «docs» no puede ir ninguna que vaya detrás de ella",
  posiblesAnteriores([k("docs"), k("disco-e", "docs"), k("nube", "disco-e"), k("otra")], k("docs")).map((x) => x.id),
  ["otra"],
);
igual("ordenar: subir y bajar", [mover(["a", "b", "c"], 2, -1), mover(["a", "b", "c"], 0, -1), mover(["a", "b", "c"], 0, 1)], [["a", "c", "b"], ["a", "b", "c"], ["b", "a", "c"]]);

console.log("\n· Filtros de versiones (4c)");
igual("filtro para la orden", filtroParaOrden({ etiquetas: "diaria, semanal", carpetas: "C:\\Datos\n\n", ultimos_dias: "90", desde: "" }), { etiquetas: ["diaria", "semanal"], carpetas: ["C:\\Datos"], ultimos_dias: 90 });
igual("…vacío: todas", filtroParaOrden({ etiquetas: " ", ultimos_dias: "" }), null);
igual("en frase", filtroEnFrase({ equipos: ["PC-ANA"], etiquetas: ["diaria"], carpetas: 2, ultimos_dias: 30 }), "de PC-ANA, con la etiqueta diaria, de 2 carpetas, de los últimos 30 días");
cierto("errores del filtro", !!errorFiltro({ ultimos_dias: "0" }) && !!errorFiltro({ desde: "1 de enero" }) && errorFiltro({ ultimos_dias: 30, desde: "2026-01-01" }) === null);

console.log("\n· La línea de una copia (4d)");
const base = { so: "Windows 11", version_agente: "0.7.23", box_pub: "", sign_pub: "", sal_equipo: "", etiqueta: null, modo: "gestionado" as const, confirmado: true, conectado: true, ultimo_contacto: null, estado_servicio: "en_marcha" as const, siguiente_seq: 1 };
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
          { tipo: "nube", nube: "Dropbox Oficina", carpeta: "Resguardo", repos: ["recepcion/documentos"], retencion_dias: 30 },
          { tipo: "carpeta", carpeta: "F:\\Todo", retencion_dias: 30 },
          { tipo: "carpeta", carpeta: "F:\\Otra-zona", zona: "z1a2b3c" },
        ],
      },
    },
  },
};
const enD: DestinoResumen = { id: "almacen-0b5c1f8e", nombre: "ALMACEN-01", tipo: "rest", donde: "https://192.168.1.20:8000/recepcion/", equipo_almacen: almacen.id };
const b2: DestinoResumen = { id: "destino-1a2b3c4d", nombre: "B2 de la oficina", tipo: "b2", donde: "copias-sur" };
const dropbox: DestinoResumen = { id: "nube-1", nombre: "Dropbox de RECEPCION", tipo: "nube", nube: "Dropbox Recepcion" };
const recepcion: Equipo = {
  ...base,
  id: "e-recepcion",
  nombre: "RECEPCION",
  rol: "agente",
  resumen: {
    admite: ["cadenas", "derivadas"],
    nubes: [{ nombre: "Dropbox Recepcion", tipo: "dropbox" }],
    destinos: [enD, b2, dropbox],
    copias: [
      { id: "docs", nombre: "Documentos", repo: "documentos" },
      { id: "fotos", nombre: "Fotos", repo: "documentos", tras: "docs" },
    ],
    repositorios: [
      {
        id: "documentos",
        nombre: "Documentos",
        destino: "ALMACEN-01",
        externa: { destino: "B2 de la oficina", destino_id: b2.id, hora: "21:00", bloqueo_dias: 30 },
        derivadas: [{ id: "d1", destino: "Dropbox de RECEPCION", destino_id: "nube-1", cuando: { tras_copia: true }, filtro: { etiquetas: ["diaria"] } }],
      },
    ],
  },
};
const equipos = [almacen, recepcion];
const pasos = lineaCadena(recepcion, "docs", equipos);
igual(
  "los pasos, en orden",
  pasos.map((p) => [p.clase, p.texto, p.despues, p.nivel]),
  [
    ["origen", "Documentos (RECEPCION)", false, 0],
    ["copia", "Almacén ALMACEN-01 · Disco D", false, 0],
    ["espejo", "Almacén ALMACEN-01 · Disco E", true, 1],
    ["espejo", "Dropbox Oficina", false, 1],
    ["espejo", "Almacén ALMACEN-01 · Disco F:", false, 1],
    ["derivada", "B2 de la oficina", false, 1],
    ["derivada", "Dropbox de RECEPCION", true, 1],
    ["copia", "Almacén ALMACEN-01 · Disco D", true, 1],
  ],
);
igual(
  "en una línea",
  lineaEnTexto(pasos.slice(0, 3)),
  "Documentos (RECEPCION) → Almacén ALMACEN-01 · Disco D → después → Almacén ALMACEN-01 · Disco E (espejo)",
);
igual("el espejo de otra zona no es de este repositorio", pasos.some((p) => p.texto.includes("Otra")), false);
cierto("Dropbox no es inmutable (se avisa)", !!pasos.find((p) => p.texto === "Dropbox Oficina")?.noInmutable && !!pasos.find((p) => p.texto === "Dropbox de RECEPCION")?.noInmutable);
cierto("la copia externa con bloqueo, inmutable", pasos.find((p) => p.texto === "B2 de la oficina")?.inmutable === true);
igual("para la tarea 8: la zona E lleva su id", pasos[2].destinoId, `zona:${almacen.id}:z1a2b3c`);
igual("la derivada dice su filtro", pasos[6].detalle, "copia derivada · después de cada copia · solo las versiones con la etiqueta diaria");
cierto("con la copia externa ya hay un destino fuera de la retención", !recomendarFueraRetencion(pasos));
const soloEspejos = lineaCadena({ ...recepcion, resumen: { ...recepcion.resumen, repositorios: [{ id: "documentos", nombre: "Documentos", destino: "ALMACEN-01" }] } }, "docs", [
  { ...almacen, resumen: { ...almacen.resumen, guarda_copias: { ...almacen.resumen!.guarda_copias!, espejo: { hora: "02:00", destinos: [{ tipo: "zona", carpeta: "z1a2b3c", retencion_dias: 30 }] } } } },
]);
cierto("si todos los espejos siguen a la retención, se recomienda otro", recomendarFueraRetencion(soloEspejos));
igual("una derivada nueva toma un id libre", idDerivadaNueva(recepcion.resumen!.repositorios![0]), "d2");
igual("la externa de siempre va primero", derivadasDe(recepcion.resumen!.repositorios![0]).map((d) => d.id), ["externa", "d1"]);

console.log("\n· La regla 3-2-1-1-0 con los pasos de la cadena (tarea 8)");
{
  const rc = reglaDeCopia(recepcion, recepcion.resumen!.copias![0], equipos, null, [], Date.now());
  igual(
    "el espejo a la zona E, la nube del almacén, la externa y la derivada a la Dropbox del equipo",
    rc?.pasos.map((p) => [p.id, p.tipo, p.lugar, p.inmutable]),
    [
      ["destino", "copia", "oficina", "solo_anadir"],
      ["espejo-1", "espejo", "oficina", "no"],
      ["espejo-2", "espejo", "nube", "no"],
      ["espejo-3", "espejo", "oficina", "no"],
      ["externa", "externa", "nube", "object_lock"],
      ["derivada-d1", "derivada", "nube", "no"],
    ],
  );
  cierto("la zona E es otro soporte que la D", rc!.pasos[0].soporte !== rc!.pasos[1].soporte);
}

console.log("\n· Pasar un destino del espejo a un paso de la cadena (7e)");
igual("uno de un solo repositorio es de la copia de ese repositorio", pasoDeEspejo({ repos: ["recepcion/documentos"] }, almacen, equipos)?.repo.id, "documentos");
igual("…uno de «todos», no", pasoDeEspejo({ repos: null }, almacen, equipos), null);
igual("…de otra zona de origen, no", pasoDeEspejo({ repos: ["recepcion/documentos"], zona: "z1a2b3c" }, almacen, equipos), null);

console.log("\n· Órdenes (4b y 7d.2)");
const ctx = { derivadas: [{ repo: "documentos", id: "d1", destino_id: "nube-1" }] };
cierto("quitar una derivada espera", esDestructiva("quitar_derivada", { repo: "documentos", id: "d1" }, 24, ctx));
cierto("una derivada nueva, no", !esDestructiva("cambiar_derivada", { repo: "documentos", id: "d2", destino: { id: "x" }, retencion: { diarias: 7 } }, 24, ctx));
cierto("cambiar solo su horario, no", !esDestructiva("cambiar_derivada", { repo: "documentos", id: "d1", destino: { id: "nube-1" }, hora: "21:00" }, 24, ctx));
cierto("cambiarle la retención, sí", esDestructiva("cambiar_derivada", { repo: "documentos", id: "d1", destino: { id: "nube-1" }, retencion: { diarias: 7 } }, 24, ctx));
cierto("llevarla a otro destino, sí", esDestructiva("cambiar_derivada", { repo: "documentos", id: "d1", destino: { id: "b2" } }, 24, ctx));
cierto("probar, nunca", !esDestructiva("cambiar_derivada", { repo: "documentos", id: "d1", destino: { id: "b2" }, solo_probar: true }, 24, ctx));
igual("la zona de origen cuenta para el espejo", claveEspejo({ tipo: "carpeta", carpeta: "F:\\x", zona: "z1a2b3c" }) === claveEspejo({ tipo: "carpeta", carpeta: "F:\\x" }), false);
igual("…«principal» es lo mismo que nada", claveEspejo({ tipo: "carpeta", carpeta: "F:\\x", zona: "principal" }), claveEspejo({ tipo: "carpeta", carpeta: "F:\\x" }));
igual("reenviar un destino con zona la conserva", destinoParaOrden({ tipo: "zona", carpeta: "principal", zona: "z1a2b3c", repos: ["a/b"] }), { tipo: "zona", carpeta: "principal", zona: "z1a2b3c", repos: ["a/b"], vistos: [] });
cierto(
  "quitar del espejo un destino con zona espera",
  esDestructiva("guarda_copias", { espejo: { destinos: [] } }, 24, { espejo: { destinos: [{ tipo: "zona", carpeta: "principal", zona: "z1a2b3c" }] } }),
);

console.log(`\n${fallos ? "MAL" : "ok"}: ${total - fallos}/${total} (copias en cadena)`);
if (fallos) process.exit(1);
