// Pruebas de «lo que comparten las consolas de un equipo» (src/lib/datosEquipo.ts,
// docs/consolas-multiples.md §6), de «Quitar este destino» y del almacén de otra
// consola en el mapa (src/lib/mapa.ts) y en «Se guarda en» (src/lib/dondeGuarda.ts).
// `npm run test:vectores` (con las demás).
import type { ConsolaDelEquipo, Equipo } from "../src/lib/tipos";
import {
  admiteDatosEquipo,
  alcanceDatos,
  cambiadoDesde,
  conDatosDelEquipo,
  destinoQueQuedaVacio,
  destinoQuitable,
  problemaDestinoPaso,
  usosDestino,
} from "../src/lib/datosEquipo";
import { construirMapa } from "../src/lib/mapa";
import { lugarDe } from "../src/lib/dondeGuarda";
import { NIVEL, SOLO_ADMIN_ROL } from "../src/lib/cripto/ordenes";

let fallos = 0;
let total = 0;
function igual(nombre: string, obtenido: unknown, esperado: unknown) {
  total++;
  const ok = JSON.stringify(obtenido) === JSON.stringify(esperado);
  if (!ok) fallos++;
  console.log(`${ok ? "ok  " : "MAL "} ${nombre}${ok ? "" : `\n       obtenido: ${JSON.stringify(obtenido)}\n       esperado: ${JSON.stringify(esperado)}`}`);
}

const base = { so: "Windows 11", version_agente: "0.7.23", box_pub: "", sign_pub: "", sal_equipo: "", etiqueta: null, modo: "gestionado" as const, confirmado: true, conectado: true, ultimo_contacto: null, estado_servicio: "en_marcha" as const, siguiente_seq: 1, rol: "agente" as const };
const ADMITE = ["consolas_multiples", "datos_equipo", "quitar_destino"];
const esta: ConsolaDelEquipo = { id: "principal", nombre: "Consola en línea", url: "https://consola.ejemplo.com", identidad: "ZXN0YQ==", sal_cliente: null, ultimo_contacto: null, desde: null, esta: true };
const oficina: ConsolaDelEquipo = { id: "o1", nombre: "Oficina", url: "https://oficina.ejemplo:8443", identidad: "b2ZpY2luYQ==", sal_cliente: "c2Fs", ultimo_contacto: null, desde: null, esta: false };

console.log("\n· Nombre y etiquetas del equipo (lib/datosEquipo.ts)");
{
  const e: Equipo = { ...base, id: "e1", nombre: "Recepción (en línea)", etiquetas: ["Viejas"], resumen: { admite: ADMITE, consolas: [esta, oficina] } };
  igual("sin datos del equipo: el de esta consola", conDatosDelEquipo(e).nombre, "Recepción (en línea)");
  igual("y es el mismo objeto", conDatosDelEquipo(e) === e, true);
  const puesto: Equipo = { ...e, resumen: { ...e.resumen, datos_equipo: { nombre: { valor: "Recepción", cuando: "2026-10-06T10:00:00Z", consola: "Oficina", esta: false, por: "Ana" }, etiquetas: { valor: ["Contabilidad"], cuando: "2026-10-06T10:00:00Z" } } } };
  igual("con datos del equipo: manda el suyo (un servidor anterior no lo copia)", [conDatosDelEquipo(puesto).nombre, conDatosDelEquipo(puesto).etiquetas], ["Recepción", ["Contabilidad"]]);
  igual("quién lo cambió", cambiadoDesde(puesto.resumen?.datos_equipo?.nombre), "Lo cambió Ana desde la consola «Oficina»");
  igual("lo cambió esta: nada que decir", cambiadoDesde({ consola: "Consola en línea", esta: true }), null);
  igual("sin nombre de consola", cambiadoDesde({ esta: false }), "Cambiado desde otra consola");
  igual("alcance con varias consolas", alcanceDatos(e), "Se guarda en el equipo: lo verán igual todas sus consolas.");
  const viejo: Equipo = { ...e, resumen: { admite: ["consolas_multiples"], consolas: [esta, oficina] } };
  igual("agente anterior: solo aquí", [admiteDatosEquipo(viejo), alcanceDatos(viejo)], [false, "Solo en esta consola: su agente aún no guarda estos datos para todas (actualízalo)."]);
  igual("agente anterior con una consola: nada que decir", alcanceDatos({ resumen: { admite: [] } }), "");
  const raro: Equipo = { ...e, resumen: { ...e.resumen, datos_equipo: { nombre: { valor: "  ", cuando: "" }, etiquetas: { valor: [3 as unknown as string], cuando: "" } } } };
  igual("lo que no vale no se usa", [conDatosDelEquipo(raro).nombre, conDatosDelEquipo(raro).etiquetas], ["Recepción (en línea)", ["Viejas"]]);
}

console.log("\n· Quitar un destino sin uso");
{
  const e: Equipo = {
    ...base,
    id: "vm",
    nombre: "SRV-APP1",
    resumen: {
      admite: ADMITE,
      destinos: [
        { id: "d-local", nombre: "Disco o carpeta del equipo", tipo: "local", unidad: "D:", extraible: false, red: false },
        { id: "d-alm", nombre: "Servidor de copias (Disco 1)", tipo: "rest", donde: "https://almacen.ejemplo:8000/vm/" },
        { id: "d-ext", nombre: "Bucket", tipo: "b2", donde: "copias-ejemplo" },
        { id: "d-der", nombre: "Otra nube", tipo: "s3", donde: "otra" },
      ],
      repositorios: [
        { id: "r1", nombre: "Documentos", destino: "Servidor de copias (Disco 1)", externa: { destino: "Bucket", destino_id: "d-ext", hora: "23:00" }, derivadas: [{ id: "x", destino: "Otra nube", destino_id: "d-der" }] },
      ],
    },
  };
  igual("usos del almacén", usosDestino(e, "d-alm"), ["el repositorio «Documentos»"]);
  igual("usos de la nube de la externa", usosDestino(e, "d-ext"), ["la copia externa de «Documentos»"]);
  igual("usos de la derivada", usosDestino(e, "d-der"), ["una copia derivada de «Documentos»"]);
  igual("el local sin uso se puede quitar", destinoQuitable(e, "d-local"), true);
  igual("el que usa un repositorio, no", destinoQuitable(e, "d-alm"), false);
  igual("el de la copia externa, tampoco", destinoQuitable(e, "d-ext"), false);
  igual("agente anterior: no se ofrece", destinoQuitable({ resumen: { ...e.resumen, admite: [] } }, "d-local"), false);
  igual("al quitar Documentos, su destino se queda vacío", destinoQueQuedaVacio(e, e.resumen!.repositorios![0])?.id, "d-alm");
  const dos: Equipo = { ...e, resumen: { ...e.resumen, repositorios: [...e.resumen!.repositorios!, { id: "r2", nombre: "Fotos", destino: "d-alm" }] } };
  igual("con otro repositorio allí, no", destinoQueQuedaVacio(dos, dos.resumen!.repositorios![0]), null);
}

console.log("\n· Copias externas y derivadas con un destino que no protege");
{
  const e: Equipo = { ...base, id: "e", nombre: "PC", resumen: { destinos: [{ id: "d1", nombre: "Disco D", tipo: "local", extraible: false, red: false }, { id: "u", nombre: "USB", tipo: "local", extraible: true }, { id: "n", nombre: "NAS", tipo: "local", red: true }, { id: "b", nombre: "B2", tipo: "b2" }] } };
  igual("a una carpeta del mismo equipo", problemaDestinoPaso(e, "d1"), "mismo_equipo");
  igual("a un disco extraíble: bien", problemaDestinoPaso(e, "u"), null);
  igual("a la red: bien", problemaDestinoPaso(e, "n"), null);
  igual("a la nube: bien", problemaDestinoPaso(e, "b"), null);
  igual("a un destino que ya no está", problemaDestinoPaso(e, "quitado"), "sin_destino");
  igual("por nombre (agente anterior)", problemaDestinoPaso(e, null, "Disco D"), "mismo_equipo");
  igual("sin saber nada: nada que avisar", problemaDestinoPaso(e, null, null), null);
}

console.log("\n· El almacén de otra consola en el mapa y en «Se guarda en»");
{
  // El equipo copia en un almacén que se gestiona desde la otra consola (no está aquí).
  const vm: Equipo = {
    ...base,
    id: "vm",
    nombre: "SRV-APP1",
    resumen: {
      admite: ADMITE,
      consolas: [esta, oficina],
      destinos: [{ id: "almacen-alc00001", nombre: "ALMACEN-CENTRAL · Disco 1", tipo: "rest", donde: "https://192.0.2.9:8000/vm/", equipo_almacen: "alc00001-0000-4000-8000-000000000001" }],
      repositorios: [{ id: "r1", nombre: "Documentos", destino: "almacen-alc00001", ultima_version: "2026-10-06T09:00:00Z" }],
      copias: [{ id: "k1", nombre: "Documentos", repo: "r1", activa: true }],
    },
  };
  const m = construirMapa([vm], {}, { cliente: "c", ahora: Date.parse("2026-10-06T12:00:00Z") });
  const fuera = m.nodos.find((n) => n.tipo === "fuera");
  igual("una tarjeta para lo que no se ve", fuera && [fuera.nombre, fuera.estado, fuera.fuera], ["«ALMACEN-CENTRAL · Disco 1» no está en esta consola", "Se gestiona desde «Oficina»", { almacen: "ALMACEN-CENTRAL · Disco 1", consolas: ["Oficina"] }]);
  const dest = m.nodos.find((n) => n.tipo === "destino");
  igual("el destino, con su nombre y «de otra consola»", dest && [dest.nombre, dest.sub, dest.tono], ["ALMACEN-CENTRAL · Disco 1", "Almacén de otra consola", "ok"]);
  igual("unidos", m.aristas.some((a) => a.de === dest?.id && a.a === fuera?.id), true);
  igual("y la frase", m.frases.some((f) => f.includes("no está en esta consola: sus espejos no se ven aquí")), true);
  // Con el almacén aquí: sin tarjeta de «fuera».
  const central: Equipo = { ...base, id: "alc00001-0000-4000-8000-000000000001", nombre: "ALMACEN-CENTRAL", rol: "almacenamiento", resumen: { guarda_copias: { activo: true, puerto: 8000, espejo: { hora: "02:00", destinos: [{ tipo: "nube", nube: "Dropbox Oficina" }] } } } };
  const m2 = construirMapa([vm, central], {}, { cliente: "c", ahora: Date.parse("2026-10-06T12:00:00Z") });
  igual("con el almacén aquí, su espejo y nada «fuera»", [m2.nodos.some((n) => n.tipo === "fuera"), m2.nodos.some((n) => n.tipo === "espejo" && n.nombre === "Dropbox Oficina")], [false, true]);
  // «Se guarda en»: el nombre del destino (el mismo en todas las consolas), no «externo».
  const l = lugarDe(vm.resumen!.destinos![0], vm, [vm]);
  igual("se guarda en: almacén de otra consola", [l.clase, l.texto, l.detalle, l.deOtraConsola, l.clave], ["almacen", "Almacén ALMACEN-CENTRAL · Disco 1 (otro equipo)", "se gestiona desde otra consola", true, "almacen-alc00001"]);
  const l2 = lugarDe(vm.resumen!.destinos![0], vm, [vm, central]);
  igual("con el almacén aquí: su nombre y su clave de zona", [l2.texto, l2.clave], ["Almacén ALMACEN-CENTRAL (otro equipo)", `zona:${central.id}:principal`]);
}

console.log("\n· Las órdenes nuevas, como en protocolo/ordenes.rs");
igual("inofensivas", ["nombre_equipo", "etiquetas_equipo", "observacion_equipo", "quitar_destino"].map((t) => NIVEL[t]), ["sesion", "sesion", "sesion", "sesion"]);
igual("el nombre y quitar destinos: solo administradores", ["nombre_equipo", "etiquetas_equipo", "observacion_equipo", "quitar_destino"].map((t) => SOLO_ADMIN_ROL.has(t)), [true, false, false, true]);

console.log(`\n${total - fallos} de ${total} bien`);
if (fallos) process.exit(1);
