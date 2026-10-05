// Pruebas de «Todos los clientes» (src/lib/global.ts): salud de cada cliente,
// lo que necesita atención, las cifras, «¿Cuándo se llena?» y el mapa con los
// clientes delante (plegar, filtrar, buscar y lo que está en marcha).
// `npm run test:vectores` (con las demás).
import type { Equipo, Informe } from "../src/lib/tipos";
import { atencion, construirMapaGlobal, llenadoGlobal, plegadosPorDefecto, saludCliente, totales, type PanelCliente } from "../src/lib/global";

let fallos = 0;
let total = 0;
function igual(nombre: string, obtenido: unknown, esperado: unknown) {
  total++;
  const ok = JSON.stringify(obtenido) === JSON.stringify(esperado);
  if (!ok) fallos++;
  console.log(`${ok ? "ok  " : "MAL "} ${nombre}${ok ? "" : `\n       obtenido: ${JSON.stringify(obtenido)}\n       esperado: ${JSON.stringify(esperado)}`}`);
}
const cierto = (nombre: string, v: boolean) => igual(nombre, v, true);

console.log("\n· Todos los clientes (lib/global.ts)");
const ahora = Date.parse("2026-10-04T12:00:00Z");
const h = (horas: number) => new Date(ahora - horas * 3600_000).toISOString();
const base = { so: "Windows 11", version_agente: "0.7.14", box_pub: "", sign_pub: "", sal_equipo: "", etiqueta: null, modo: "gestionado" as const, confirmado: true, conectado: true, ultimo_contacto: h(0.1), estado_servicio: "en_marcha" as const, siguiente_seq: 1, rol: "agente" as const };
const pc = (id: string, nombre: string, ok = true, almacen?: string): Equipo => ({
  ...base,
  id,
  nombre,
  resumen: {
    destinos: [almacen ? { id: "d", nombre: "Almacén", tipo: "rest", equipo_almacen: almacen } : { id: "d", nombre: "Nube", tipo: "b2", donde: `${id}-copias` }],
    repositorios: [{ id: "r", nombre: `Docs ${nombre}`, destino: "d", versiones: 3, bytes: 2e9, ultima_version: h(3) }],
    copias: [{ id: "k", nombre: "Docs", repo: "r", activa: true, ultima: { cuando: h(3), estado: ok ? "ok" : "fallo", mensaje: ok ? null : "Sin red" }, proxima: h(-5) }],
  },
});
const almacen = (id: string): Equipo => ({ ...base, id, nombre: "ALMACEN", rol: "almacenamiento", resumen: { guarda_copias: { activo: true, espacio: { libre: 30e9, total: 1e12 } } } });
const versiones = Array.from({ length: 20 }, (_, i) => ({ id: `v${i}`, hora: h(i * 24 + 1), copia: "k", total_bytes: 2e9, anadido: 1e9, anadido_empaquetado: 1e9, archivos_nuevos: 1, archivos_cambiados: 0, archivos_sin_cambios: 0, duracion_s: 60, etiquetas: [] }));
const informe = (fallo = false): Informe => ({
  recibido: h(0.1),
  datos: { repos: [{ id: "r", nombre: "Docs", versiones, versiones_leidas: null, ejecuciones: [{ hora: h(2), copia: "k", resultado: fallo ? "fallo" : "ok", mensaje_corto: null }], espacio: null, verificacion: null, prueba_restauracion: null, externa: null, proteccion: null }] },
});
const cliente = (id: string, nombre: string, equipos: Equipo[], extra: Partial<PanelCliente> = {}): PanelCliente => ({
  id,
  nombre,
  rol: "propietario",
  marca: null,
  equipos,
  avisos_abiertos: 0,
  pendientes: 0,
  informes: equipos.filter((e) => e.resumen?.repositorios?.length).map((e) => ({ equipo: e.id, recibido: h(0.1), datos: informe(e.resumen?.copias?.[0]?.ultima?.estado === "fallo").datos })),
  informes_completos: true,
  ...extra,
});

const a = cliente("A", "Altamar", [pc("a1", "CAJA"), pc("a2", "CONTABILIDAD", false, "a3"), almacen("a3")]);
const b = cliente("B", "Bodega", [pc("b1", "OFICINA")], { avisos_abiertos: 2, rol: "lectura" });
const c = cliente("C", "Clínica", [pc("c1", "RECEPCION"), pc("c2", "HISTORIAS")]);
const vacio = cliente("D", "Despacho", []);
const todos = [a, b, c, vacio];

igual("salud de cada cliente", todos.map((x) => [x.nombre, saludCliente(x, ahora).tono, saludCliente(x, ahora).texto]), [
  ["Altamar", "bad", "1 con problemas"],
  ["Bodega", "warn", "Avisos sin revisar"],
  ["Clínica", "ok", "Al día"],
  ["Despacho", "neutral", "Sin equipos"],
]);
const sa = saludCliente(a, ahora);
igual("al día, protegido y 14 días de todos sus equipos", [sa.alDia, sa.equipos, sa.protegido, sa.dias.length, sa.dias.at(-1)?.estado], [2, 3, 4e9, 14, "aviso"]);

const at = atencion(todos, ahora);
igual("lo que necesita atención: lo grave arriba, con su cliente", at.map((x) => [x.cliente.nombre, x.tono, x.titulo]), [
  ["Altamar", "bad", "CONTABILIDAD · copia fallida"],
  ["Bodega", "warn", "2 avisos sin revisar"],
]);
igual("con su acción («Copiar ahora» si su papel lo permite)", at.map((x) => x.accion.texto), ["Copiar ahora", "Ver avisos"]);
igual("solo lectura: «Ver equipo» en vez de «Copiar ahora»", atencion([{ ...a, rol: "lectura" }], ahora)[0].accion, { texto: "Ver equipo", href: "/c/A/equipos/a2" });

const t = totales(todos, ahora);
igual("cifras de todos", [t.equipos, t.alDia, t.porTono.bad, t.repos, t.protegido, t.versiones24, t.fallos24], [6, 5, 1, 5, 10e9, 5, 1]);
igual("la próxima copia, con su cliente", [t.proxima?.cliente.nombre, t.proxima?.equipo.nombre], ["Altamar", "CAJA"]);

const ll = llenadoGlobal(todos, ahora);
igual("¿Cuándo se llena?: el almacén que se llena antes, arriba y con su cliente", [ll[0].cliente.nombre, ll[0].nombre, ll[0].tono], ["Altamar", "ALMACEN", "bad"]);
cierto("las claves de dos clientes no se juntan", new Set(ll.map((x) => x.clave)).size === ll.length);

// El mapa con los clientes delante.
const m = construirMapaGlobal(todos, { ahora });
igual("columna 0: los clientes, lo urgente arriba (y lo neutro antes que lo que va bien)", m.nodos.filter((n) => n.col === 0).map((n) => n.nombre), ["Altamar", "Bodega", "Despacho", "Clínica"]);
igual("cada cliente lleva a sus equipos (el almacén sale como destino)", m.aristas.filter((x) => x.tipo === "cliente" && x.de === "cl:A").map((x) => x.a).sort(), ["A/eq:a1", "A/eq:a2"]);
igual("los equipos, una columna más a la derecha", [...new Set(m.nodos.filter((n) => n.tipo === "equipo").map((n) => n.col))], [1]);
cierto("ningún id se repite entre clientes (cada uno con su cliente delante)", new Set(m.nodos.map((n) => n.id)).size === m.nodos.length);
cierto("las frases dicen el cliente", m.frases[0].startsWith("Altamar: 1 con problemas, 3 equipos."));

const plegados = plegadosPorDefecto(todos, () => false, ahora);
igual("con 4 clientes, los que van bien empiezan plegados", [...plegados].sort(), ["C", "D"]);
const mp = construirMapaGlobal(todos, { ahora, plegados });
igual("un cliente plegado: solo su tarjeta", mp.nodos.filter((n) => n.id.startsWith("C/")).length, 0);
cierto("…y lo dice", mp.nodos.find((n) => n.id === "cl:C")?.plegado === true);
igual("filtro por cliente", construirMapaGlobal(todos, { ahora, cliente: "B" }).nodos.filter((n) => n.col === 0).map((n) => n.nombre), ["Bodega"]);
igual("filtro «con problemas»", construirMapaGlobal(todos, { ahora, estado: "problemas" }).nodos.filter((n) => n.col === 0).map((n) => n.nombre), ["Altamar", "Bodega"]);
const busca = construirMapaGlobal(todos, { ahora, buscar: "historias", plegados });
igual("buscar: solo la cadena de lo que casa (aunque su cliente esté plegado)", busca.nodos.map((n) => n.nombre), ["Clínica", "HISTORIAS", "Docs HISTORIAS", "Nube"]);
igual("buscar sin tildes ni mayúsculas", construirMapaGlobal(todos, { ahora, buscar: "CLINICA" }).nodos.filter((n) => n.col === 0).map((n) => n.nombre), ["Clínica"]);
const vivo = construirMapaGlobal(todos, { ahora, enVivo: (e, _r, tipo) => (e === "c1" && tipo === "copia" ? "Copiando 40 %" : null) });
cierto("una copia en marcha: el trazo del cliente a ese equipo se mueve", !!vivo.aristas.find((x) => x.id === "cl:C>C/eq:c1")?.vivo);
cierto("…y el de su repositorio", !!vivo.aristas.find((x) => x.de === "C/eq:c1")?.vivo);
const vivoPlegado = construirMapaGlobal(todos, { ahora, plegados: new Set(["C"]), enVivo: (e, _r, tipo) => (e === "c1" && tipo === "copia" ? "Copiando 40 %" : null) });
igual("plegado y con algo en marcha: lo dice su tarjeta", vivoPlegado.nodos.find((n) => n.id === "cl:C")?.vivo, "Copiando 40 %");

console.log(`\n${total - fallos} de ${total} comprobaciones correctas.`);
if (fallos) process.exit(1);
