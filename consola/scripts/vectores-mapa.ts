// Pruebas de la geometría del «Mapa de la protección» (src/lib/mapaGeometria.ts):
// ningún trazo pasa por detrás de una tarjeta que no es uno de sus extremos,
// ninguna tarjeta pisa a otra y el orden de las columnas quita los cruces que
// se pueden quitar. Con mapas de verdad (lib/mapa.ts y lib/global.ts) y con
// grafos al azar (con semilla). `npm run test:vectores` (con las demás).
import type { Equipo, RepositorioResumen } from "../src/lib/tipos";
import { construirMapa, progresoCorto, type Mapa } from "../src/lib/mapa";
import { leerOrganizacion } from "../src/lib/sedes";
import { leerInfra } from "../src/lib/infra";
import { tareaDelTrazo, textoDelTrazo } from "../src/lib/mapaVivo";
import { alternarGrupo, clavePrefs, leerPrefs, PREFS_POR_DEFECTO } from "../src/lib/mapaPreferencias";
import type { TareaEnMarcha } from "../src/lib/tipos";
import { construirMapaGlobal, type PanelCliente } from "../src/lib/global";
import { cortaCaja, disponer, isotonica, problemas, type EntradaDisposicion } from "../src/lib/mapaGeometria";

let fallos = 0;
let total = 0;
function igual(nombre: string, obtenido: unknown, esperado: unknown) {
  total++;
  const ok = JSON.stringify(obtenido) === JSON.stringify(esperado);
  if (!ok) fallos++;
  console.log(`${ok ? "ok  " : "MAL "} ${nombre}${ok ? "" : `\n       obtenido: ${JSON.stringify(obtenido)}\n       esperado: ${JSON.stringify(esperado)}`}`);
}
const cierto = (nombre: string, v: boolean) => igual(nombre, v, true);

console.log("\n· Piezas: regresión isotónica y cortes con una caja");
igual("isotónica: ya en orden, igual", isotonica([1, 2, 3], [1, 1, 1]), [1, 2, 3]);
igual("isotónica: junta los que se cruzan (media)", isotonica([3, 1, 5], [1, 1, 1]), [2, 2, 5]);
igual("isotónica: con pesos", isotonica([4, 0], [3, 1]), [3, 3]);
const caja = { x: 10, y: 10, w: 20, h: 10 };
cierto("un segmento que la cruza, corta", cortaCaja({ x: 0, y: 15 }, { x: 40, y: 15 }, caja));
cierto("uno por encima, no", !cortaCaja({ x: 0, y: 5 }, { x: 40, y: 5 }, caja));
cierto("uno que acaba en su borde, no (es su puerto)", !cortaCaja({ x: 0, y: 15 }, { x: 10, y: 15 }, caja));
cierto("uno dentro del todo, sí", cortaCaja({ x: 12, y: 12 }, { x: 14, y: 14 }, caja));

// Tamaños parecidos a los de la consola (los mide el navegador).
const ALTO: Record<string, number> = { cliente: 92, equipo: 78, grupo: 78, repo: 38, destino: 78, espejo: 78, externa: 78, fuera: 128 };
const ANCHO: Record<number, number> = { 0: 210, 1: 200, 2: 220, 3: 220, 4: 220 };
function entradaDe(m: Mapa, extra: Partial<EntradaDisposicion> = {}): EntradaDisposicion {
  return {
    nodos: m.nodos.map((n) => ({ id: n.id, col: n.col })),
    aristas: m.aristas.map((a) => ({ id: a.id, de: a.de, a: a.a, marca: a.tono === "bad" || a.tono === "warn" })),
    altos: Object.fromEntries(m.nodos.map((n) => [n.id, ALTO[n.tipo] + (n.aviso ? 22 : 0)])),
    anchos: ANCHO,
    separacionCol: 64,
    separacionFila: { 1: 10, 2: 10 },
    ...extra,
  };
}
function comprobar(nombre: string, m: Mapa, extra: Partial<EntradaDisposicion> = {}) {
  const d = disponer(entradaDe(m, extra));
  igual(`${nombre}: ningún trazo por detrás de otra tarjeta, ninguna tarjeta pisa a otra`, problemas(d), []);
  igual(`${nombre}: todas las tarjetas y todos los trazos`, [Object.keys(d.cajas).length, d.rutas.length], [new Set(m.nodos.map((n) => n.id)).size, m.aristas.length]);
  cierto(`${nombre}: cada trazo sale del borde derecho de su tarjeta y llega al izquierdo de la otra`, d.rutas.every((r) => {
    const a = d.cajas[r.de];
    const b = d.cajas[r.a];
    const p = r.tramos[0].de;
    const q = r.tramos.at(-1)!.a;
    return Math.abs(p.x - (a.x + a.w)) < 0.01 && Math.abs(p.y - (a.y + a.h / 2)) < 0.01 && Math.abs(q.x - b.x) < 0.01 && Math.abs(q.y - (b.y + b.h / 2)) < 0.01;
  }));
  return d;
}

// --- Mapas de ejemplo (nombres inventados) ---------------------------------
const ahora = Date.parse("2026-10-04T12:00:00Z");
const h = (horas: number) => new Date(ahora - horas * 3600_000).toISOString();
const base = { so: "Windows 11", version_agente: "0.7.24", box_pub: "", sign_pub: "", sal_equipo: "", etiqueta: null, modo: "gestionado" as const, confirmado: true, conectado: true, ultimo_contacto: h(0.1), estado_servicio: "en_marcha" as const, siguiente_seq: 1 };
const repo = (id: string, nombre: string, destino: string, externa?: string): RepositorioResumen =>
  ({ id, nombre, destino, versiones: 5, bytes: 2e9, ultima_version: h(3), ...(externa ? { externa: { destino: externa, hora: "03:00" } } : {}) }) as RepositorioResumen;
const almacen = (id: string, nombre: string, espejos: number): Equipo => ({
  ...base,
  id,
  nombre,
  rol: "almacenamiento",
  resumen: {
    guarda_copias: {
      activo: true,
      zonas: [
        { id: "z0a0b0c", nombre: "Disco E", carpeta: "E:\\Copias", puerto: 8002, usuarios: 2 },
        { id: "z0d0e0f", nombre: "Disco F", carpeta: "F:\\Copias", puerto: 8003, usuarios: 1 },
      ],
      espejo: {
        hora: "02:00",
        destinos: [
          { tipo: "zona", carpeta: "z0a0b0c", ultima: h(10), resultado: "Espejo hecho." },
          { tipo: "nube", nube: "Dropbox Norte", ultima: h(30), resultado: "ERROR: la nube respondió 429." },
          { tipo: "carpeta", carpeta: "G:\\Espejo", ultima: h(70), resultado: "Espejo hecho." },
          { tipo: "nube", nube: "B2 Norte", ultima: h(5), resultado: "Espejo hecho." },
          { tipo: "zona", carpeta: "z0d0e0f", ultima: null },
        ].slice(0, espejos) as never,
      },
    },
  } as Equipo["resumen"],
});
let fallo = 0;
const pc = (id: string, nombre: string, repos: RepositorioResumen[], destinos: Equipo["resumen"] extends infer R ? (R extends { destinos?: infer D } ? D : never) : never): Equipo => ({
  ...base,
  id,
  nombre,
  rol: "agente",
  resumen: {
    destinos,
    repositorios: repos,
    copias: repos.map((r) => ({ id: `k-${r.id}`, nombre: r.nombre, repo: r.id, activa: true, ultima: { cuando: h(3), estado: fallo++ % 3 === 1 ? "fallo" : "ok" }, proxima: h(-5) })),
  } as Equipo["resumen"],
});

console.log("\n· Un equipo con su cadena (la ficha de un equipo)");
{
  const alm = almacen("alm-norte", "ALMACEN-NORTE", 3);
  const destinos = [
    { id: "d-alm", nombre: "Almacén Norte", tipo: "rest", equipo_almacen: "alm-norte" },
    { id: "d-usb", nombre: "Disco USB", tipo: "local", unidad: "E:", extraible: true },
    { id: "d-b2", nombre: "B2 Norte", tipo: "b2", donde: "norte-copias" },
  ] as never;
  const caja = pc("caja-9", "CAJA-9", [repo("r1", "Facturas", "d-alm", "B2 Norte"), repo("r2", "Correo", "d-alm", "Disco USB"), repo("r3", "Fotos", "d-usb"), repo("r4", "Planos", "d-b2", "Dropbox Caja")], destinos);
  const m = construirMapa([caja, alm], {}, { cliente: "c", ahora, raiz: { perspectiva: "equipos", id: "caja-9" } });
  cierto("hay trazos que saltan una columna (copia externa por encima de los destinos)", m.aristas.some((a) => a.tipo === "externa"));
  const d = comprobar("equipo con cadenas", m);
  igual("sin cruces", d.cruces, 0);
}

console.log("\n· Un almacén con zonas, espejos y nubes (lo que guardan en él los demás)");
{
  const alm = almacen("alm-sur", "ALMACEN-SUR-9", 5);
  const destinos = [{ id: "d-alm", nombre: "Almacén Sur", tipo: "rest", equipo_almacen: "alm-sur" }, { id: "d-nube", nombre: "Nube propia", tipo: "s3", donde: "propia" }] as never;
  const equipos = [
    alm,
    ...Array.from({ length: 6 }, (_, i) =>
      pc(`pc-${i}`, `PUESTO-${i + 1}`, [repo(`a${i}`, `Documentos ${i + 1}`, "d-alm", i % 2 ? "Nube externa" : undefined), ...(i % 3 === 0 ? [repo(`b${i}`, `Contabilidad ${i + 1}`, i === 3 ? "d-nube" : "d-alm", "Disco del puesto")] : [])], destinos),
    ),
  ];
  const m = construirMapa(equipos, {}, { cliente: "c", ahora, raiz: { perspectiva: "equipos", id: "alm-sur" }, agruparDesde: 99 });
  cierto("con cinco espejos y copias externas", m.nodos.filter((n) => n.tipo === "espejo").length === 5 && m.nodos.some((n) => n.tipo === "externa"));
  comprobar("almacén con zonas, espejos y nubes", m);
  // Todo el cliente (los equipos con sus otros destinos también).
  comprobar("el cliente entero", construirMapa(equipos, {}, { cliente: "c", ahora, agruparDesde: 99 }));
}

console.log("\n· 14mm: un solo nodo por servidor y los espejos desde sus repositorios");
{
  // El almacén dio acceso a PUESTO-A («Copiar en …»); PUESTO-B llega al mismo servidor como
  // «Servidor de copias» genérico (un repositorio adoptado), y el propio almacén por localhost.
  const alm = almacen("alm-este", "ALMACEN-ESTE", 0);
  alm.resumen!.guarda_copias!.espejo = {
    hora: "02:00",
    destinos: [
      { tipo: "nube", nube: "Nube Este", ultima: h(4), resultado: "Espejo hecho.", repos: ["puesto-b/ventas"] },
      { tipo: "carpeta", carpeta: "G:\\Espejo", ultima: h(40), resultado: "ERROR: disco lleno.", repos: ["puesto-a"] },
      { tipo: "nube", nube: "Nube de nadie", ultima: h(4), resultado: "Espejo hecho.", repos: ["otro/x"] },
    ],
  } as never;
  alm.resumen!.destinos = [{ id: "d-local", nombre: "Mi almacén", tipo: "rest", donde: "https://localhost:8000/almacen-este/" }] as never;
  alm.resumen!.repositorios = [repo("consola", "Consola", "d-local")];
  const a = pc("pc-a", "PUESTO-A", [repo("docs", "Documentos", "d-alm")], [{ id: "d-alm", nombre: "Almacén Este", tipo: "rest", donde: "https://almacen-este.lan:8000/puesto-a/", equipo_almacen: "alm-este" }] as never);
  const b = pc("pc-b", "PUESTO-B", [{ ...repo("ventas", "Ventas", "d-rest"), ruta: "puesto-b/ventas" }], [{ id: "d-rest", nombre: "Servidor de copias", tipo: "rest", donde: "https://almacen-este.lan:8000" }] as never);
  const m = construirMapa([a, b, alm], {}, { cliente: "c", ahora, agruparDesde: 99 });
  const destinos = m.nodos.filter((n) => n.tipo === "destino");
  igual("el almacén y el «Servidor de copias» con su dirección son un solo nodo", destinos.map((n) => n.id), ["al:alm-este"]);
  igual("los tres repositorios llegan a él", m.aristas.filter((x) => x.tipo === "guarda").map((x) => x.a), ["al:alm-este", "al:alm-este", "al:alm-este"]);
  const espejo = (nombre: string) => m.aristas.filter((x) => x.tipo === "espejo" && m.nodos.find((n) => n.id === x.a)?.nombre === nombre).map((x) => [x.de, x.tono]);
  igual("el espejo de Ventas sale de Ventas (al día)", espejo("Nube Este"), [["rp:pc-b:ventas", "ok"]]);
  igual("el de PUESTO-A sale de su repositorio (falló)", espejo("G:\\Espejo"), [["rp:pc-a:docs", "bad"]]);
  // 0.8.1 (15m, punto 5): el de un repositorio que no se reconoce ya no desaparece: sale del almacén con una marca.
  igual("el de un repositorio que no sale aquí, desde el almacén", espejo("Nube de nadie"), [["al:alm-este", "ok"]]);
  igual("…con la marca «no sé de qué repositorios es»", m.nodos.filter((n) => n.sinReconocer).map((n) => [n.nombre, n.aviso]), [["Nube de nadie", "No sé de qué repositorios es"]]);
  cierto("los demás espejos no cuelgan del almacén", m.aristas.filter((x) => x.tipo === "espejo" && x.de === "al:alm-este").length === 1);
  cierto("la frase lo cuenta desde el repositorio", m.frases.some((f) => f.includes("«Ventas»") && f.includes("espejo en Nube Este")));
  comprobar("un solo nodo por servidor", m);
}

console.log("\n· 14kk: las copias derivadas, «después» del repositorio a su destino");
{
  const alm = almacen("alm-oeste", "ALMACEN-OESTE", 0);
  const destinos = [
    { id: "d-local", nombre: "Disco D", tipo: "local", unidad: "D:" },
    { id: "d-alm", nombre: "Almacén Oeste", tipo: "rest", donde: "https://oeste.lan:8000/caja-3/", equipo_almacen: "alm-oeste" },
    { id: "d-b2", nombre: "B2 Oeste", tipo: "b2", donde: "oeste-copias" },
  ] as never;
  const r1 = { ...repo("caja", "Caja", "d-local"), derivadas: [{ id: "dv1", destino: "Almacén Oeste", destino_id: "d-alm", cuando: { tras_copia: true } }, { id: "dv2", destino: "B2 Oeste", destino_id: "d-b2", cuando: { hora: "23:00" } }] } as RepositorioResumen;
  const r2 = repo("docs", "Documentos", "d-alm");
  const caja = pc("caja-3", "CAJA-3", [r1, r2], destinos);
  const inf = { recibido: h(1), datos: { repos: [{ id: "caja", nombre: "Caja", versiones: [], versiones_leidas: null, ejecuciones: [], espacio: null, verificacion: null, prueba_restauracion: null, externa: null, proteccion: null, derivadas: [{ id: "dv1", resultado: "fallo", ultima: h(2) }] }] } } as never;
  const m = construirMapa([caja, alm], { "caja-3": inf }, { cliente: "c", ahora, agruparDesde: 99 });
  const der = m.aristas.filter((x) => x.tipo === "derivada");
  igual("dos flechas derivadas desde «Caja»", der.map((x) => [x.de, x.a]), [["rp:caja-3:caja", "al:alm-oeste"], ["rp:caja-3:caja", "de:b2|oeste-copias"]]);
  igual("con su estado y «después» si aún no se hizo", der.map((x) => [x.tono, x.etiqueta?.replace(/\s/g, " ")]), [["bad", "falló hace 2 h"], ["neutral", "después · cada día a las 23:00"]]);
  cierto("al almacén de la derivada llegan también los suyos (un solo nodo)", m.nodos.filter((n) => n.id === "al:alm-oeste").length === 1 && m.aristas.some((x) => x.tipo === "guarda" && x.de === "rp:caja-3:docs" && x.a === "al:alm-oeste"));
  cierto("la frase lo cuenta", m.frases.some((f) => f.includes("después, una copia derivada a B2 Oeste")));
  comprobar("derivadas", m);
}

console.log("\n· El mapa de todos los clientes, con muchos equipos");
{
  const clientes: PanelCliente[] = Array.from({ length: 5 }, (_, k) => {
    const alm = almacen(`alm-${k}`, `ALMACEN-${k}`, 1 + (k % 4));
    const destinos = [{ id: "d-alm", nombre: "Almacén", tipo: "rest", equipo_almacen: `alm-${k}` }, { id: "d-b2", nombre: "B2", tipo: "b2", donde: `cliente-${k}` }] as never;
    const equipos = [alm, ...Array.from({ length: 3 + k * 2 }, (_, i) => pc(`e-${k}-${i}`, `EQUIPO-${k}-${i}`, [repo(`r${i}`, `Datos ${i}`, i % 4 === 3 ? "d-b2" : "d-alm", i % 3 === 0 ? "Disco externo" : undefined)], destinos))];
    return { id: `cli-${k}`, nombre: `Cliente ${k}`, rol: "propietario", equipos, avisos_abiertos: 0, pendientes: 0, informes: [], informes_completos: true };
  });
  const g = construirMapaGlobal(clientes, { ahora });
  cierto("cinco columnas y muchos equipos", new Set(g.nodos.map((n) => n.col)).size === 5 && g.nodos.filter((n) => n.tipo === "equipo" || n.tipo === "grupo").length >= 10);
  comprobar("todos los clientes", g, { separacionCol: 48 });
}

console.log("\n· 0.8.1 (C2): agrupar por sede, desplegar, lo que hay en un grupo, lo que está en marcha y las capas");
{
  const alm = almacen("alm-o", "ALMACEN-O", 4);
  const destinos = [{ id: "d-alm", nombre: "Almacén", tipo: "rest", equipo_almacen: "alm-o" }] as never;
  const equipos = [alm, ...Array.from({ length: 6 }, (_, i) => pc(`s-${i}`, `PC-${i}`, [repo(`r${i}`, `Datos ${i}`, "d-alm")], destinos))];
  const org = leerOrganizacion({
    sedes: [
      { id: "central", nombre: "Oficina central", principal: true },
      { id: "planta", nombre: "Planta" },
    ],
    equipos: { "s-0": "planta", "s-1": "planta", "s-2": "planta" },
  });
  const ag = { modo: "sede" as const, n: 8 };
  const m = construirMapa(equipos, {}, { cliente: "c", ahora, agrupar: ag, organizacion: org });
  igual("por sede: un grupo por sede, con su nombre", m.nodos.filter((n) => n.col === 0).map((n) => n.nombre).sort(), ["Oficina central · 3 equipos", "Planta · 3 equipos"]);
  const g = m.nodos.find((n) => n.nombre.startsWith("Planta"))!;
  igual("el grupo sabe qué equipos lleva (para la lista al pasar)", g.miembros?.map((x) => x.nombre).sort(), ["PC-0", "PC-1", "PC-2"]);
  cierto("…y su id para desplegarlo", g.grupo === "gr:sede:planta");
  const pil = m.nodos.find((n) => n.tipo === "repo" && n.grupo === "gr:sede:planta")!;
  igual("su píldora lleva los repositorios con su equipo", pil.miembros?.map((x) => `${x.nombre} · ${x.detalle}`).sort(), ["Datos 0 · PC-0", "Datos 1 · PC-1", "Datos 2 · PC-2"]);
  const d = construirMapa(equipos, {}, { cliente: "c", ahora, agrupar: ag, organizacion: org, desplegados: new Set(["gr:sede:planta"]) });
  igual("desplegado: sus equipos, sueltos", d.nodos.filter((n) => n.col === 0).map((n) => n.nombre).sort(), ["Oficina central · 3 equipos", "PC-0", "PC-1", "PC-2"]);
  igual("…y dice qué se puede plegar", d.desplegados, [{ id: "gr:sede:planta", titulo: "Planta" }]);
  const no = construirMapa(equipos, {}, { cliente: "c", ahora, agrupar: { modo: "no", n: 8 }, organizacion: org });
  igual("«No agrupar»: todos sueltos", no.nodos.filter((n) => n.col === 0).length, 6);
  const mas = construirMapa(equipos, {}, { cliente: "c", ahora, agrupar: { modo: "mas_de", n: 4 } });
  cierto("«Con más de 4»: se juntan los que están al día (como antes)", mas.nodos.some((n) => n.tipo === "grupo" && /^\d+ equipos$/.test(n.nombre)));
  igual("las sedes de cada tarjeta (capa «Sedes»)", [...new Set(m.nodos.filter((n) => n.col <= 1).map((n) => n.sede?.nombre ?? "-"))].sort(), ["Oficina central", "Planta"]);
  comprobar("agrupado por sede", m);
  comprobar("desplegado", d);
}
{
  // 15cc: una derivada y un espejo en marcha se animan, con su porcentaje.
  const alm = almacen("alm-v", "ALMACEN-V", 4);
  (alm.resumen!.guarda_copias!.espejo!.destinos![3] as { trabajo?: string }).trabajo = "t-b2";
  const r = { ...repo("conta", "Contabilidad", "d-alm"), derivadas: [{ id: "d1", destino_id: "d-b2", destino: "B2 Sur", cuando: { tras_copia: true } }] } as RepositorioResumen;
  const e = pc("pc-v", "PC-V", [r], [{ id: "d-alm", nombre: "Almacén", tipo: "rest", equipo_almacen: "alm-v" }, { id: "d-b2", nombre: "B2 Sur", tipo: "b2", donde: "sur-copias" }] as never);
  const vivo = (_e: string, rr: string, t: string, id?: string) => (t === "derivada" && rr === "conta" && id === "d1" ? "Subiendo 45 %" : t === "espejo" && rr === "t-b2" ? "Subiendo 12 %" : null);
  const m = construirMapa([e, alm], {}, { cliente: "c", ahora, agruparDesde: 99, enVivo: vivo });
  const dv = m.aristas.find((a) => a.tipo === "derivada")!;
  igual("la derivada en marcha: se mueve, en «info» y con su porcentaje", [dv.vivo, dv.tono, dv.progreso], [true, "info", "45 %"]);
  igual("…y su destino dice lo que hace", m.nodos.find((n) => n.nombre === "B2 Sur")?.estado, "Subiendo 45 %");
  const es = m.aristas.find((a) => a.tipo === "espejo" && m.nodos.find((n) => n.id === a.a)?.nombre === "B2 Norte")!;
  igual("el espejo en marcha también", [es.vivo, es.tono, es.progreso], [true, "info", "12 %"]);
  igual("progreso corto", [progresoCorto("Copiando 7 %"), progresoCorto("En cola"), progresoCorto("Subiendo…")], ["7 %", "en cola", "en marcha"]);
  // La tarea de cada trazo (lib/mapaVivo.ts).
  const tareas = [
    { tipo: "copia", repo: "conta", fase: "en_marcha" },
    { tipo: "copia_externa", repo: "conta", fase: "en_marcha", derivada: "d1" },
    { tipo: "copia_externa", repo: "conta", fase: "en_cola" },
    { tipo: "espejo", repo: "t-b2", espejo: "t-b2", fase: "en_marcha" },
  ] as TareaEnMarcha[];
  igual(
    "cada trazo con su tarea",
    [tareaDelTrazo(tareas, "conta", "derivada", "d1")?.derivada, tareaDelTrazo(tareas, "conta", "copia_externa")?.fase, tareaDelTrazo(tareas, "conta", "derivada", "d9"), tareaDelTrazo(tareas, "t-b2", "espejo")?.tipo],
    ["d1", "en_cola", undefined, "espejo"],
  );
  igual("en palabras", [textoDelTrazo(tareas[0], "copia", 40), textoDelTrazo(tareas[2], "copia_externa", null), textoDelTrazo(undefined, "copia", 1)], ["Copiando 40 %", "En cola", null]);
}
{
  // 15l (C4): con la infraestructura, el servidor físico de cada tarjeta y el aviso de servidor compartido.
  const alm = almacen("alm-i", "ALMACEN-I", 0);
  const e = pc("vm-i", "VM-I", [repo("datos", "Datos", "d-alm")], [{ id: "d-alm", nombre: "Almacén", tipo: "rest", equipo_almacen: "alm-i" }] as never);
  const infra = leerInfra({
    servidores: [{ id: "hv-01", nombre: "hv-01", hipervisor: "proxmox", conjuntos: [{ id: "tank", nombre: "tank", tipo: "zfs_espejo", discos: 2 }] }],
    equipos: { "vm-i": { servidor: "hv-01" }, "alm-i": { servidor: "hv-01" } },
    destinos: { "zona:alm-i:principal": { servidor: "hv-01", conjunto: "tank" } },
  });
  const m = construirMapa([e, alm], {}, { cliente: "c", ahora, agruparDesde: 99, infra });
  igual("el equipo y el almacén, en hv-01", [m.nodos.find((n) => n.nombre === "VM-I")?.servidorFisico, m.nodos.find((n) => n.nombre === "ALMACEN-I")?.servidorFisico], ["hv-01 · Proxmox VE", "hv-01 · Proxmox VE"]);
  igual("…el almacén en tank (capa «Almacenamiento»)", m.nodos.find((n) => n.nombre === "ALMACEN-I")?.conjunto, "tank · espejo de 2 discos");
  igual("…y el aviso de servidor físico compartido", m.nodos.find((n) => n.nombre === "ALMACEN-I")?.aviso, "Mismo servidor físico que lo que copia");
  const sin = construirMapa([e, alm], {}, { cliente: "c", ahora, agruparDesde: 99 });
  cierto("sin infraestructura, nada de esto", !sin.nodos.some((n) => n.servidorFisico || n.conjunto) && !sin.nodos.some((n) => n.aviso?.includes("servidor físico")));
}
{
  // Las preferencias del mapa, por persona (lib/mapaPreferencias.ts).
  igual("sin nada guardado, lo de siempre", leerPrefs(""), PREFS_POR_DEFECTO);
  const p = leerPrefs(JSON.stringify({ agrupar: { modo: "sede", n: 3 }, desplegados: ["gr:sede:x", 4], capas: { sedes: true, estado: "no" } }));
  igual("lo que vale se lee; lo demás, lo de por defecto", [p.agrupar, p.desplegados, p.capas.sedes, p.capas.estado], [{ modo: "sede", n: 3 }, ["gr:sede:x"], true, true]);
  igual("un modo raro, el de siempre", leerPrefs(JSON.stringify({ agrupar: { modo: "todo", n: 1 } })).agrupar, { modo: "mas_de", n: 8 });
  const a = alternarGrupo(PREFS_POR_DEFECTO, "gr:x");
  igual("desplegar y plegar", [a.desplegados, alternarGrupo(a, "gr:x").desplegados], [["gr:x"], []]);
  igual("la clave es por persona y por mapa", clavePrefs("u1", "cli-1"), "mapa.prefs.u1.cli-1");
}

console.log("\n· Los cruces que se pueden quitar, se quitan");
{
  // a→y y b→x, con x arriba: la columna de la derecha se da la vuelta.
  const d = disponer({
    nodos: [
      { id: "a", col: 0 },
      { id: "b", col: 0 },
      { id: "x", col: 1 },
      { id: "y", col: 1 },
    ],
    aristas: [
      { id: "1", de: "a", a: "y" },
      { id: "2", de: "b", a: "x" },
    ],
  });
  igual("orden: la primera columna manda; la segunda la sigue", d.capas, [["a", "b"], ["y", "x"]]);
  igual("sin cruces", d.cruces, 0);
  cierto("cada uno a la altura de su origen (trazos rectos)", Math.abs(d.cajas.a.y - d.cajas.y.y) < 0.01 && Math.abs(d.cajas.b.y - d.cajas.x.y) < 0.01);
  // Un salto de columna pasa por un hueco entre las tarjetas de en medio.
  const s = disponer({
    nodos: [
      { id: "a", col: 0 },
      { id: "m1", col: 1 },
      { id: "m2", col: 1 },
      { id: "z", col: 2 },
    ],
    aristas: [
      { id: "a>m1", de: "a", a: "m1" },
      { id: "a>m2", de: "a", a: "m2" },
      { id: "a>z", de: "a", a: "z", marca: true },
    ],
  });
  igual("el salto, sin pasar por las de en medio", problemas(s), []);
  const ruta = s.rutas.find((r) => r.id === "a>z")!;
  igual("curva, recta por el hueco, curva", ruta.tramos.map((t) => t.tipo), ["curva", "recta", "curva"]);
  cierto("su marca, en la recta del hueco (entre las tarjetas)", !!ruta.marca && ruta.marca.x > s.cajas.m1.x && ruta.marca.x < s.cajas.m1.x + s.cajas.m1.w);
  // Control: la curva directa de antes (de tarjeta a tarjeta) sí pasaba por detrás.
  const directa = { ...ruta, tramos: [{ tipo: "curva" as const, de: ruta.tramos[0].de, a: ruta.tramos.at(-1)!.a }] };
  const m1 = s.cajas.m1;
  const enMedio = { ...s, cajas: { ...s.cajas, m1: { ...m1, y: (directa.tramos[0].de.y + directa.tramos[0].a.y) / 2 - m1.h / 2 } } };
  cierto("control: una curva directa por encima de una tarjeta se detecta", problemas({ ...enMedio, rutas: [directa] }).includes("«a>z» pasa por «m1»"));
}

console.log("\n· Casos raros");
igual("sin nada que dibujar", disponer({ nodos: [], aristas: [] }), { cajas: {}, rutas: [], capas: [], ancho: 0, alto: 0, cruces: 0 });
{
  const d = disponer({ nodos: [{ id: "a", col: 0 }], aristas: [{ id: "x", de: "a", a: "no-esta" }] });
  igual("un trazo a una tarjeta que no está, se ignora", [Object.keys(d.cajas), d.rutas.length], [["a"], 0]);
}

console.log("\n· Grafos al azar (con semilla): nunca por detrás de una tarjeta");
{
  let semilla = 12345;
  const azar = () => ((semilla = (semilla * 1103515245 + 12345) % 2 ** 31) / 2 ** 31);
  const malos: string[] = [];
  for (let k = 0; k < 60; k++) {
    const ncols = 2 + Math.floor(azar() * 4);
    const nodos: { id: string; col: number }[] = [];
    const altos: Record<string, number> = {};
    for (let c = 0; c < ncols; c++) {
      const n = 1 + Math.floor(azar() * 7);
      for (let i = 0; i < n; i++) {
        const id = `n${c}-${i}`;
        nodos.push({ id, col: c });
        altos[id] = 30 + Math.floor(azar() * 110);
      }
    }
    const aristas: { id: string; de: string; a: string; marca: boolean }[] = [];
    for (const a of nodos)
      for (const b of nodos)
        if (b.col > a.col && azar() < (b.col === a.col + 1 ? 0.3 : 0.12)) aristas.push({ id: `${a.id}>${b.id}`, de: a.id, a: b.id, marca: azar() < 0.3 });
    const d = disponer({ nodos, aristas, altos, separacionFila: 8 + Math.floor(azar() * 12), separacionCol: 40 + Math.floor(azar() * 40) });
    const p = problemas(d);
    if (p.length) malos.push(`grafo ${k}: ${p.slice(0, 3).join("; ")}`);
  }
  igual("60 grafos de 2 a 5 columnas, con saltos de columna", malos, []);
}

console.log(`\n${total - fallos}/${total} bien`);
if (fallos) process.exit(1);
