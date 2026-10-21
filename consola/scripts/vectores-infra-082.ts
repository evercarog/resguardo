// Pruebas del plan 0.8.2, bloque A y C4 (todo con nombres inventados):
// - A5: la infraestructura como dato común (`infraValida`, con los vectores de Rust de
//   crates/protocolo/vectors/datos-cliente.json, «infra»; claves, clave de administración,
//   en palabras; lo que ocupa).
// - A2: discos físicos uno a uno y conjuntos con sus discos (leer, guardar, proponer).
// - A3: los discos de un equipo en tres niveles (`discosEnNiveles`) y dónde está cada volumen.
// - A1: lo que falta por anotar (`sinAnotar`).
// - A4: filtrar el mapa por servidor físico o por disco o conjunto.
// - C4: el mapa de Equipos con recuadros por grupo, plegables, y el orden de la lista.
// `npm run test:vectores` (con las demás).
import { readFileSync } from "node:fs";
import type { Equipo, RepositorioResumen } from "../src/lib/tipos";
import {
  bytesInfra,
  datosInfra,
  discoParaAgrupar,
  discosEnNiveles,
  enFiltroInfra,
  fisicoDe,
  formaConjunto,
  infraValida,
  leerFiltroInfra,
  leerInfra,
  lineaFisica,
  MAX_BYTES_INFRA,
  opcionesFiltroInfra,
  paraGuardar,
  propuestas,
  servidorParaAgrupar,
  sinAnotar,
  sitioDeDestino,
  sitioDeEquipo,
  sitioDeVolumen,
  textoFiltroInfra,
  valorFiltroInfra,
} from "../src/lib/infra";
import { admiteClave, claseDe, pideClave, textoAviso, textoValor, tituloDato, pendiente } from "../src/lib/datosComunes";
import { agruparPorNiveles, leerOrganizacion, nivelesDisponibles } from "../src/lib/sedes";
import { construirMapa } from "../src/lib/mapa";
import { alternarPlegado, leerPrefs, PREFS_POR_DEFECTO } from "../src/lib/mapaPreferencias";

let fallos = 0;
let total = 0;
function igual(nombre: string, obtenido: unknown, esperado: unknown) {
  total++;
  const ok = JSON.stringify(obtenido) === JSON.stringify(esperado);
  if (!ok) fallos++;
  console.log(`${ok ? "ok  " : "MAL "} ${nombre}${ok ? "" : `\n       obtenido: ${JSON.stringify(obtenido)}\n       esperado: ${JSON.stringify(esperado)}`}`);
}
const cierto = (nombre: string, v: boolean) => igual(nombre, v, true);
const sinFinos = (t: string | null | undefined) => t?.replace(/[\u202f\u2009\u00a0]/g, " ") ?? t;

const T = 1e12;
const ahora = Date.parse("2026-10-21T12:00:00Z");
const h = (horas: number) => new Date(ahora - horas * 3600_000).toISOString();
const base = { so: "Windows Server 2019", version_agente: "0.8.2", box_pub: "", sign_pub: "", sal_equipo: "", etiqueta: null, modo: "gestionado" as const, confirmado: true, conectado: true, ultimo_contacto: h(0.1), estado_servicio: "en_marcha" as const, siguiente_seq: 1, rol: "agente" as const };
const eq = (id: string, nombre: string, resumen: Equipo["resumen"]): Equipo => ({ ...base, id, nombre, resumen });

console.log("\n· A5: los vectores de Rust (crates/protocolo/vectors/datos-cliente.json, «infra»)");
{
  const v = JSON.parse(readFileSync(new URL("../../crates/protocolo/vectors/datos-cliente.json", import.meta.url), "utf8")) as {
    infra: { nombre: string; clave: string; valor: unknown; valido: boolean; limpio?: unknown }[];
    claves: { clave: string; valida: boolean; pide_admin: boolean }[];
  };
  for (const c of v.infra) {
    const r = infraValida(c.clave, c.valor);
    igual(`${c.nombre}: ${c.valido ? "vale" : "no vale"}`, r.ok, c.valido);
    if ("limpio" in c && r.ok) igual(`${c.nombre}: limpio`, r.limpio, c.limpio);
  }
  for (const c of v.claves.filter((x) => x.clave.startsWith("infra:"))) {
    igual(`clave ${c.clave}`, [!!claseDe(c.clave), pideClave(c.clave)], [c.valida, c.pide_admin]);
  }
}

console.log("\n· A2: discos físicos y conjuntos con sus discos");
const infra = leerInfra({
  servidores: [
    {
      id: "srv-01",
      nombre: "SERVIDOR-01",
      hipervisor: "hyperv",
      sede: "central",
      discos: [
        { id: "d0", nombre: "SSD del sistema", tipo: "ssd", modelo: "SSD 240", tamano: 0.24 * T, visto: "disco0", estado: "bien" },
        { id: "b2", nombre: "Bahía 2", tipo: "hdd", serie: "SN-EJ-1002", tamano: 4 * T, bahia: "2" },
        { id: "b3", nombre: "Bahía 3", tipo: "hdd", serie: "SN-EJ-1003", tamano: 4 * T, bahia: "3", estado: "averiado" },
        { id: "mal", nombre: "", tipo: "ssd" },
        { id: "b9", nombre: "Raro", tipo: "cinta" },
      ],
      conjuntos: [
        { id: "dv2", nombre: "Disco virtual 2", tipo: "raid1", miembros: ["b2", "b3", "b3", "zz"], capacidad: 4 * T, visto: "disco1" },
        { id: "viejo", nombre: "De antes", tipo: "raid5", discos: 4 },
      ],
    },
  ],
  equipos: {
    "srv-eq": { servidor: "srv-01" },
    "vm-01": { servidor: "srv-01", conjunto: "dv2", volumenes: [{ punto: "C:", disco: "d0" }, { punto: "D:" }, { punto: "E:", disco: "no-existe" }] },
    "vm-02": { servidor: "srv-01", conjunto: "dv2", disco: "d0" },
  },
  destinos: { "zona:alm:principal": { servidor: "srv-01", disco: "b2", volumenes: [{ punto: "C:", disco: "d0" }] } },
});
const srv = infra.servidores[0];
igual("los discos que valen (sin nombre o de un tipo raro: el raro se queda sin tipo)", srv.discos.map((d) => [d.id, d.tipo ?? null]), [["d0", "ssd"], ["b2", "hdd"], ["b3", "hdd"], ["b9", null]]);
igual("un conjunto con sus discos (sin repetir ni ajenos) y uno de antes con su número", srv.conjuntos.map((c) => [c.miembros ?? null, formaConjunto(c)]), [[["b2", "b3"], "RAID 1 de 2 discos"], [null, "RAID 5 de 4 discos"]]);
igual("los volúmenes de un equipo: solo los que tienen sitio", infra.equipos["vm-01"].volumenes, [{ punto: "C:", disco: "d0" }]);
igual("conjunto y disco a la vez: manda el conjunto", infra.equipos["vm-02"], { servidor: "srv-01", conjunto: "dv2" });
igual("un destino no tiene volúmenes", infra.destinos["zona:alm:principal"], { servidor: "srv-01", disco: "b2" });
const g = paraGuardar(infra);
cierto("para guardar: sin nulos ni listas vacías", !JSON.stringify(g).includes("null") && !JSON.stringify(g).includes("[]"));
const datos = datosInfra(infra);
igual("como datos comunes: uno por servidor, equipo y destino", Object.keys(datos).sort(), ["infra:destino:zona:alm:principal", "infra:equipo:srv-eq", "infra:equipo:vm-01", "infra:equipo:vm-02", "infra:servidor:srv-01"]);
cierto("…y todos valen como en el protocolo (y ya están limpios)", Object.entries(datos).every(([k, v]) => {
  const r = infraValida(k, v);
  return r.ok && JSON.stringify(r.limpio) === JSON.stringify(v);
}));
const b = bytesInfra(infra);
cierto(`lo que ocupa (${b} bytes) cabe de sobra`, b > 1000 && b < MAX_BYTES_INFRA / 10);

console.log("\n· A3: dónde está cada volumen y la línea de cada sitio");
const org = leerOrganizacion({ sedes: [{ id: "central", nombre: "Oficina central", principal: true }] });
igual("un volumen anotado aparte", lineaFisica(sitioDeVolumen(infra, "vm-01", "C:"), org), "SERVIDOR-01 · SSD del sistema · Oficina central");
igual("uno sin anotar: el del equipo", lineaFisica(sitioDeVolumen(infra, "vm-01", "D:"), org), "SERVIDOR-01 · Disco virtual 2, RAID 1 de 2 discos · Oficina central");
igual("el propio servidor: por lo que ve su equipo («disco1» → el disco virtual 2)", sitioDeVolumen(infra, "srv-eq", "D:", "disco1")?.conjunto?.nombre, "Disco virtual 2");
igual("un destino en un disco suelto", lineaFisica(sitioDeDestino(infra, "zona:alm:principal", null)), "SERVIDOR-01 · Bahía 2");
igual("la regla: el soporte de un disco suelto", fisicoDe(sitioDeDestino(infra, "zona:alm:principal", null)), { fisico: "srv-01", soporte: "disco:srv-01:b2" });

console.log("\n· A3: los discos del equipo en tres niveles (nunca «disco0»)");
const fisico = eq("srv-eq", "SERVIDOR-01", {
  datos_basicos: {
    fabricante: "Marca Z",
    modelo: "Torre 2U",
    discos: [
      { punto: "C:", total: 240e9, libre: 88e9, sistema_archivos: "NTFS", en_uso: true, volumen: "disco0", etiqueta: "Sistema" },
      { punto: "D:", total: 4e12, libre: 1.4e12, sistema_archivos: "ReFS", en_uso: true, volumen: "disco1", etiqueta: "Datos" },
      { punto: "E:", total: 2e12, libre: 1.9e12, sistema_archivos: "NTFS", volumen: "disco2" },
    ],
    almacenamiento: {
      discos: [
        { id: "disco0", total: 240e9, clase: "ssd", bus: "sata", modelo: "SSD 240" },
        { id: "disco1", total: 4e12, bus: "raid", modelo: "Controladora RAID" },
        { id: "disco2", total: 2e12, bus: "spaces", modelo: "Espacio de almacenamiento" },
        { id: "disco3", total: 1e12, clase: "hdd", bus: "sata", modelo: "HDD 1T" },
      ],
      conjuntos: [
        { id: "disco1", tipo: "raid", total: 4e12 },
        { id: "disco2", tipo: "storage_spaces", total: 2e12 },
      ],
    },
  } as never,
  destinos: [{ id: "carpeta-d", nombre: "Copias en el disco D", tipo: "local", unidad: "D:", carpeta: "D:\\Copias" }] as never,
  espejo_equipo: { trabajos: [{ id: "t1", nombre: "USB de la oficina", activo: true, quien: "equipo", que: { tipo: "todos" }, adonde: { tipo: "carpeta", carpeta: "E:\\Espejo" }, cuando: {}, retencion: {}, freno: {}, orden: 0 }] } as never,
});
const n = discosEnNiveles(fisico, infra, org);
igual(
  "discos físicos, el disco virtual de la controladora y los Espacios de almacenamiento",
  n.nodos.map((x) => [x.nivel, sinFinos(x.titulo)]),
  [
    ["fisico", "SSD del sistema"],
    ["fisico", "Disco 2"],
    ["conjunto", "Disco virtual 2"],
    ["conjunto", "Espacios de almacenamiento de 2 TB"],
  ],
);
cierto("nada de «disco0» ni «disco1» en lo que se enseña", !JSON.stringify(n).match(/"(titulo|detalle)":"[^"]*disco\d/i));
igual("cada volumen cuelga del suyo, con su etiqueta", n.nodos.map((x) => x.volumenes.map((v) => `${v.punto} ${v.etiqueta ?? ""}`.trim())), [["C: Sistema"], [], ["D: Datos"], ["E:"]]);
igual("lo que guarda cada volumen", n.nodos.flatMap((x) => x.volumenes.map((v) => [v.punto, v.guarda])), [["C:", ["con copias"]], ["D:", ["destino «Copias en el disco D»"]], ["E:", ["espejo «USB de la oficina»"]]]);
igual("el disco virtual 2: hecho de sus dos bahías (una averiada)", [n.nodos[2].miembros.map((m) => m.titulo), sinFinos(n.nodos[2].detalle)], [["Bahía 2", "Bahía 3"], "RAID 1 de 2 discos · 4 TB"]);
igual("el disco anotado con su estado; el que no, con su modelo y tamaño", [n.nodos[0].estado, sinFinos(n.nodos[1].detalle), n.nodos[1].anotado], ["Bien", "HDD 1 TB · HDD 1T", false]);
const vm = eq("vm-01", "VM-01", {
  entorno: { virtual: "hyperv" },
  datos_basicos: { discos: [{ punto: "C:", total: 128e9, libre: 40e9, sistema_archivos: "NTFS" }, { punto: "D:", total: 500e9, libre: 3e9, sistema_archivos: "NTFS", en_uso: true }], almacenamiento: { discos: [{ id: "disco0", total: 128e9, bus: "virtual", modelo: "Virtual Disk" }], conjuntos: [] } } as never,
});
const nv = discosEnNiveles(vm, infra, org);
igual("una máquina virtual: su anfitrión y sus discos virtuales", [nv.anfitrion, nv.nodos.map((x) => [x.nivel, x.titulo])], ["SERVIDOR-01 · Disco virtual 2, RAID 1 de 2 discos · Oficina central", [["virtual", "Disco virtual 1"]]]);
igual("…y cada volumen dónde está (uno anotado aparte)", nv.sueltos.map((v) => [v.punto, v.fisico, v.tono]), [["C:", "SERVIDOR-01 · SSD del sistema · Oficina central", "ok"], ["D:", null, "bad"]]);
cierto("…con algún volumen aparte", nv.porVolumen);
igual("sin nada anotado: igual de claro, sin anfitrión", discosEnNiveles(vm, null).anfitrion, null);

console.log("\n· A2: lo que se propone con lo que ve el agente");
const ps = propuestas([fisico], leerInfra({ servidores: [{ id: "srv-01", nombre: "SERVIDOR-01", discos: [{ id: "d0", nombre: "SSD del sistema", visto: "disco0" }] }], equipos: { "srv-eq": { servidor: "srv-01" } } }));
igual("el propio servidor ve un disco que no está anotado (ni los de la controladora ni los Espacios)", ps.map((p) => [p.clase, p.discosNuevos?.map((d) => [d.visto, d.tipo, d.nombre])]), [["discos", [["disco3", "hdd", "HDD 1T"]]]]);
cierto("…y lo dice sin número de serie (lo escribe la persona)", ps[0].frase.includes("El número de serie lo escribes tú") && !JSON.stringify(ps[0].discosNuevos).includes("serie"));
const nuevo = propuestas([fisico], null)[0];
igual("un servidor nuevo: con sus discos (y cómo los ve el equipo) y sus conjuntos", [nuevo.clase, nuevo.servidor?.discos.map((d) => d.visto), nuevo.servidor?.conjuntos.map((c) => [c.tipo, c.visto])], ["servidor", ["disco0", "disco3"], [["otro", "disco1"], ["storage_spaces", "disco2"]]]);

console.log("\n· A1: lo que falta por anotar");
const otro = eq("pc-01", "PC-01", {});
const sinA = sinAnotar(infra, [fisico, vm, otro, { ...otro, id: "pc-02", confirmado: false }], [
  { clave: "zona:alm:principal", tipo: "zona" },
  { clave: "zona:alm:z1", tipo: "zona", equipo: "srv-eq" },
  { clave: "carpeta-x", tipo: "local", equipo: null },
  { clave: "d-b2", tipo: "b2" },
]);
igual("equipos y destinos sin anotar (la nube no; una zona hereda el de su almacén; uno sin confirmar, no)", [sinA.equipos.map((e) => e.id), sinA.destinos.map((d) => d.clave)], [["pc-01"], ["carpeta-x"]]);
igual("sin ningún servidor anotado, no se avisa", sinAnotar(null, [otro], [{ clave: "x", tipo: "local" }]), { equipos: [], destinos: [] });

console.log("\n· A5: en «Lo mismo en todas las consolas»");
igual("claves, la clave de administración y si lo guarda el equipo", [!!claseDe("infra:servidor:srv-01"), pideClave("infra:servidor:srv-01"), pideClave("infra:equipo:e1"), pideClave("infra:destino:zona:a:principal")], [true, false, true, true]);
const guarda = { resumen: { admite: ["datos_cliente", "infra_comun"], datos_cliente: { bytes: 1000 } } } as unknown as Equipo;
const lleno = { resumen: { admite: ["datos_cliente", "infra_comun"], datos_cliente: { bytes: 230 * 1024 } } } as unknown as Equipo;
const viejo = { resumen: { admite: ["datos_cliente"] } } as unknown as Equipo;
igual("solo a los agentes que la guardan y con sitio en su documento", [admiteClave(guarda, "infra:servidor:x"), admiteClave(lleno, "infra:servidor:x"), admiteClave(viejo, "infra:servidor:x")], [true, false, false]);
const valorSrv = datos["infra:servidor:srv-01"];
igual("en palabras", [textoValor("infra:servidor:srv-01", valorSrv), textoValor("infra:equipo:vm-01", datos["infra:equipo:vm-01"]), textoValor("infra:destino:x", null)], ["«SERVIDOR-01», 4 discos, 2 conjuntos", "en srv-01, conjunto dv2, 1 volumen aparte", "sin anotar"]);
igual("el título", [tituloDato({ clave: "infra:servidor:srv-01", valor: valorSrv, local: null }), tituloDato({ clave: "infra:destino:zona:a:principal", valor: null, local: null }, { destino: () => "Almacén" })], ["Servidor físico «SERVIDOR-01»", "Dónde está físicamente «Almacén»"]);
const p = pendiente({ filas: [{ clave: "infra:equipo:vm-01", valor: { servidor: "srv-01" }, cambiado: h(1), consola: null, esta: true, por: "Ana", semilla: false, estado: "aplicado", local: null, por_enviar: true }], sin_compartir: [{ clave: "infra:servidor:srv-01", valor: valorSrv }] });
igual("lo que va con la clave y lo que va solo", [p.porEnviarConClave.map((x) => x.clave), p.sinCompartir.map((x) => x.clave), textoAviso(p)], [["infra:equipo:vm-01"], ["infra:servidor:srv-01"], "1 ubicación física por repartir (pide la clave)"]);

console.log("\n· A4: filtrar el mapa por infraestructura");
igual("leer y guardar el filtro", [leerFiltroInfra("cj:srv-01:dv2", infra), leerFiltroInfra("cj:srv-01:otro", infra), leerFiltroInfra("srv:srv-01"), leerFiltroInfra("x"), valorFiltroInfra({ servidor: "srv-01", disco: "b2" })], [{ servidor: "srv-01", conjunto: "dv2" }, null, { servidor: "srv-01" }, null, "dk:srv-01:b2"]);
igual("las opciones: cada servidor y, dentro, sus conjuntos y discos", opcionesFiltroInfra(infra).map((o) => o.valor), ["srv:srv-01", "cj:srv-01:dv2", "cj:srv-01:viejo", "dk:srv-01:d0", "dk:srv-01:b2", "dk:srv-01:b3", "dk:srv-01:b9"]);
igual("en palabras", textoFiltroInfra({ servidor: "srv-01", conjunto: "dv2" }, infra), "SERVIDOR-01 · Disco virtual 2");
cierto("un disco de un conjunto está en lo filtrado por ese disco", enFiltroInfra(sitioDeEquipo(infra, "vm-01"), { servidor: "srv-01", disco: "b3" }));
{
  const repo = (id: string, nombre: string, destino: string): RepositorioResumen => ({ id, nombre, destino, versiones: 3, bytes: 1e9, ultima_version: h(2) }) as RepositorioResumen;
  const alm = eq("alm", "ALMACEN-01", { guarda_copias: { activo: true, carpeta: "/tank/copias" } } as never);
  alm.rol = "almacenamiento";
  const pc = (id: string, nombre: string, destinos: unknown, repos: RepositorioResumen[]) =>
    eq(id, nombre, { destinos, repositorios: repos, copias: repos.map((r) => ({ id: `k-${r.id}`, nombre: r.nombre, repo: r.id, activa: true, ultima: { cuando: h(2), estado: "ok" }, proxima: h(-5) })) } as never);
  const vm1 = pc("vm-1", "VM-1", [{ id: "d-alm", nombre: "Almacén", tipo: "rest", equipo_almacen: "alm" }], [repo("datos", "Datos", "d-alm")]);
  const pc2 = pc("pc-2", "PC-2", [{ id: "d-alm", nombre: "Almacén", tipo: "rest", equipo_almacen: "alm" }], [repo("docs", "Documentos", "d-alm")]);
  const pc3 = pc("pc-3", "PC-3", [{ id: "d-b2", nombre: "B2", tipo: "b2", donde: "copias" }], [repo("fotos", "Fotos", "d-b2")]);
  const inf = leerInfra({
    servidores: [
      { id: "hv-01", nombre: "hv-01", conjuntos: [{ id: "tank", nombre: "tank", tipo: "zfs_espejo", discos: 2 }, { id: "ssd", nombre: "SSD", tipo: "disco" }] },
      { id: "hv-02", nombre: "hv-02" },
    ],
    equipos: { "vm-1": { servidor: "hv-02" }, alm: { servidor: "hv-01", conjunto: "ssd" }, "pc-3": { servidor: "hv-02" } },
    destinos: { "zona:alm:principal": { servidor: "hv-01", conjunto: "tank" } },
  });
  const todos = [vm1, pc2, pc3, alm];
  const nombres = (f: unknown) =>
    construirMapa(todos, {}, { cliente: "c", ahora, infra: inf, filtroInfra: f as never, agruparDesde: 99 })
      .nodos.filter((x) => x.tipo === "equipo" || x.tipo === "destino")
      .map((x) => x.nombre)
      .sort();
  igual("por un servidor: lo que recibe su almacén (y de quién) ", nombres({ servidor: "hv-01" }), ["ALMACEN-01", "PC-2", "VM-1"]);
  igual("por otro: sus equipos y lo que guardan", nombres({ servidor: "hv-02" }), ["ALMACEN-01", "B2", "PC-3", "VM-1"]);
  igual("por un conjunto: lo que se guarda en él", nombres({ servidor: "hv-01", conjunto: "tank" }), ["ALMACEN-01", "PC-2", "VM-1"]);
  const vacio = construirMapa(todos, {}, { cliente: "c", ahora, infra: inf, filtroInfra: { servidor: "hv-01", conjunto: "nada" } });
  igual("nada en lo filtrado: lo dice", vacio.filtro, { texto: "hv-01", nada: true });
  const m = construirMapa(todos, {}, { cliente: "c", ahora, infra: inf, agruparDesde: 99 });
  igual("lo anotado en cada tarjeta (servidor · discos)", [m.nodos.find((x) => x.nombre === "ALMACEN-01")?.conjunto, m.nodos.find((x) => x.nombre === "VM-1")?.servidorFisico], ["tank · espejo de 2 discos", "hv-02"]);
  igual("por defecto se ve lo anotado (capas Infraestructura y Almacenamiento)", [PREFS_POR_DEFECTO.capas.infra, PREFS_POR_DEFECTO.capas.almacenamiento], [true, true]);
  igual("el filtro se recuerda (y uno raro no)", [leerPrefs(JSON.stringify({ infra: "cj:hv-01:tank" })).infra, leerPrefs(JSON.stringify({ infra: "../x" })).infra], ["cj:hv-01:tank", ""]);

  console.log("\n· C4: el mapa de Equipos con «Agrupar por»");
  const grupoDe = (e: Equipo) => (e.id === "pc-3" ? { id: "b", nombre: "Planta" } : { id: "a", nombre: "Oficina" });
  const r = construirMapa([pc3, pc2, vm1], {}, { cliente: "c", ahora, todos, agrupar: { modo: "recuadros", n: 8 }, grupoDe, ordenEquipos: ["vm-1", "pc-2", "pc-3"] });
  igual("cada equipo en el recuadro de su grupo (y sus repositorios también)", r.nodos.filter((x) => x.col <= 1).map((x) => [x.nombre, x.recuadro?.nombre ?? null]), [
    ["VM-1", "Oficina"],
    ["PC-2", "Oficina"],
    ["PC-3", "Planta"],
    ["Datos", "Oficina"],
    ["Documentos", "Oficina"],
    ["Fotos", "Planta"],
  ]);
  igual("los grupos, desplegados", r.grupos?.map((x) => [x.titulo, x.plegado, x.equipos]), [["Oficina", false, 2], ["Planta", false, 1]]);
  const pl = construirMapa([pc3, pc2, vm1], {}, { cliente: "c", ahora, todos, agrupar: { modo: "recuadros", n: 8 }, grupoDe, plegados: new Set(["gr:rq:a"]), ordenEquipos: ["pc-3", "vm-1", "pc-2"] });
  igual("uno plegado: un solo nodo en su recuadro, en el orden de la lista", pl.nodos.filter((x) => x.col === 0).map((x) => [x.tipo, x.nombre, x.recuadro?.nombre]), [["equipo", "PC-3", "Planta"], ["grupo", "Oficina · 2 equipos", "Oficina"]]);
  const dest = construirMapa([pc2, vm1], {}, { cliente: "c", ahora, todos, agrupar: { modo: "recuadros", n: 8 }, grupoDe, grupoDestino: () => ({ id: "a", nombre: "Oficina" }) });
  igual("los destinos, en el recuadro que diga quien agrupa (sede o servidor)", dest.nodos.filter((x) => x.tipo === "destino").map((x) => x.recuadro?.id), ["rq:a"]);
  igual("plegar y desplegar se recuerda", [alternarPlegado(PREFS_POR_DEFECTO, "gr:rq:a").plegados, alternarPlegado(alternarPlegado(PREFS_POR_DEFECTO, "gr:rq:a"), "gr:rq:a").plegados], [["gr:rq:a"], []]);

  console.log("\n· C4: «Agrupar por» servidor físico o disco");
  igual("se ofrece solo con infraestructura anotada", [nivelesDisponibles(null, false, false, true).map((x) => x.nivel), nivelesDisponibles(null, false).map((x) => x.nivel)], [["servidor", "disco", "etiqueta"], ["etiqueta"]]);
  const ctx = { organizacion: null, sistema: () => "", servidor: (e: Equipo) => servidorParaAgrupar(inf, e.id), disco: (e: Equipo) => discoParaAgrupar(inf, e.id) };
  igual("por servidor", agruparPorNiveles(todos, ["servidor"], ctx).map((x) => [x.titulo, x.equipos.map((e) => e.nombre)]), [["hv-01", ["ALMACEN-01"]], ["hv-02", ["VM-1", "PC-3"]], ["Sin servidor anotado", ["PC-2"]]]);
  igual("por disco o conjunto", agruparPorNiveles(todos, ["disco"], ctx).map((x) => x.titulo), ["hv-01 · SSD", "Sin disco anotado"]);
}

console.log(`\n${total - fallos}/${total} bien`);
if (fallos) process.exit(1);
