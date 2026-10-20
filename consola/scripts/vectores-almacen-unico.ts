// Pruebas de «un almacén es uno solo» y «¿de quién es esta carpeta del almacén?»
// (src/lib/almacenUnico.ts; plan 0.8.1, B3 y B6; plan-mejoras 15dd y 15k).
// `npm run test:vectores` (con las demás). Nombres y direcciones inventados.
import type { Equipo } from "../src/lib/tipos";
import { almacenDetras, contenidoAlmacen, duenoDeCarpeta, esAdoptado, nombresDelAlmacen, rutaEnElAlmacen, servidorSinPuerto, zonaPorPuerto } from "../src/lib/almacenUnico";
import { claveZona, destinosDelCliente } from "../src/lib/destinos";
import { lugarRepo } from "../src/lib/dondeGuarda";
import { repoEnAlmacen } from "../src/lib/cadenas";
import { opcionesRepos } from "../src/lib/espejoTrabajos";
import { reposDeTrabajo, reposEnDestino } from "../src/lib/fichaDestino";
import { mapaNombres, porOrdenar } from "../src/lib/nombres";
import { conexionDe, nubesVista } from "../src/lib/nubesAlmacen";
import { retencionesDelAlmacen } from "../src/lib/almacenUnico";

let fallos = 0;
let total = 0;
function igual(nombre: string, obtenido: unknown, esperado: unknown) {
  total++;
  const ok = JSON.stringify(obtenido) === JSON.stringify(esperado);
  if (!ok) fallos++;
  console.log(`${ok ? "ok  " : "MAL "} ${nombre}${ok ? "" : `\n       obtenido: ${JSON.stringify(obtenido)}\n       esperado: ${JSON.stringify(esperado)}`}`);
}

const base = { so: "Windows 11", version_agente: "0.8.1", box_pub: "", sign_pub: "", sal_equipo: "", etiqueta: null, modo: "gestionado" as const, confirmado: true, conectado: true, ultimo_contacto: null, estado_servicio: "en_marcha" as const, siguiente_seq: 1, rol: "agente" as const };
const repo = (id: string, nombre: string, destino: string, extra: Record<string, unknown> = {}) => ({ id, nombre, destino, versiones: 3, bytes: 1000, ...extra }) as never;

const almacen: Equipo = {
  ...base,
  id: "a1b2c3d4-0000-4000-8000-000000000001",
  nombre: "ALMACEN-01",
  rol: "almacenamiento",
  resumen: {
    admite: ["zonas_almacen", "espejo_trabajos"],
    guarda_copias: {
      activo: true,
      puerto: 8000,
      solo_red_local: true,
      usuarios: 2,
      carpeta: "D:\\Copias",
      repositorios: [
        { usuario: "pc-01-3f2a", repos: ["contabilidad"] },
        { usuario: "viejo", repos: ["historico"] },
        { usuario: "alm-propio", repos: ["consola"] },
        { usuario: "otro-9z9z", repos: ["suelto"] },
      ],
      zonas: [{ id: "z1a2b3c", nombre: "Disco E", carpeta: "E:\\Copias", puerto: 8002, usuarios: 1, repositorios: [{ usuario: "pc-01-3f2a", repos: ["contabilidad-d1"] }] }],
    },
    destinos: [{ id: "propio", nombre: "Servidor de copias", tipo: "rest", donde: "https://localhost:8000" }],
    repositorios: [repo("consola", "Copias de la consola", "propio", { ruta: "alm-propio/consola" })],
  } as never,
};
const pc: Equipo = {
  ...base,
  id: "e1e1e1e1-0000-4000-8000-000000000002",
  nombre: "PC-01",
  resumen: {
    destinos: [
      { id: "almacen-a1b2c3d4", nombre: "ALMACEN-01", tipo: "rest", donde: "https://almacen.ejemplo.com:8000/pc-01-3f2a/", equipo_almacen: almacen.id },
      { id: "almacen-a1b2c3d4-z1a2b3c", nombre: "ALMACEN-01 · Disco E", tipo: "rest", donde: "https://almacen.ejemplo.com:8002/pc-01-3f2a/", equipo_almacen: almacen.id },
      { id: "servidor-copias", nombre: "Servidor de copias", tipo: "rest", donde: "https://almacen.ejemplo.com:8000" },
      { id: "nas", nombre: "NAS", tipo: "rest", donde: "https://nas.ejemplo.com:8000/pc-01/" },
    ],
    repositorios: [
      repo("contabilidad", "Contabilidad", "almacen-a1b2c3d4", { derivadas: [{ id: "d1", destino: "ALMACEN-01 · Disco E", destino_id: "almacen-a1b2c3d4-z1a2b3c" }] }),
      repo("historico", "Histórico", "servidor-copias", { ruta: "viejo/historico" }),
      repo("fotos", "Fotos", "nas"),
    ],
  } as never,
};
const equipos = [almacen, pc];
const d = (e: Equipo, id: string) => e.resumen!.destinos!.find((x) => x.id === id)!;

console.log("\n· 15dd: el almacén detrás de un destino");
igual("servidor sin puerto ni usuario", servidorSinPuerto("rest:https://usuario@Almacen.Ejemplo.com:8000/x/"), "almacen.ejemplo.com");
igual("el destino que dio el almacén", almacenDetras(pc, d(pc, "almacen-a1b2c3d4"), equipos)?.nombre, "ALMACEN-01");
igual("un «Servidor de copias» al mismo servidor es el almacén", almacenDetras(pc, d(pc, "servidor-copias"), equipos)?.nombre, "ALMACEN-01");
igual("otro servidor no", almacenDetras(pc, d(pc, "nas"), equipos), undefined);
igual("localhost desde el propio almacén", almacenDetras(almacen, d(almacen, "propio"), equipos)?.nombre, "ALMACEN-01");
igual("localhost desde otro equipo no es el almacén", almacenDetras(pc, { id: "x", nombre: "x", tipo: "rest", donde: "https://localhost:8000" }, equipos), undefined);
igual("adoptado: el genérico", esAdoptado(pc, d(pc, "servidor-copias"), equipos), true);
igual("no adoptado: el que dio el almacén", esAdoptado(pc, d(pc, "almacen-a1b2c3d4"), equipos), false);
igual("no adoptado: otro servidor", esAdoptado(pc, d(pc, "nas"), equipos), false);
igual("la zona por el puerto", zonaPorPuerto(almacen, "https://almacen.ejemplo.com:8002/x/"), "z1a2b3c");
igual("sin puerto: la principal", zonaPorPuerto(almacen, "https://almacen.ejemplo.com/x/"), "principal");
igual("ruta en el almacén: la del destino y la propia", rutaEnElAlmacen("https://almacen.ejemplo.com:8000/pc-01-3f2a/", "contabilidad"), { valor: "pc-01-3f2a/contabilidad", usuario: "pc-01-3f2a" });
igual("ruta de un adoptado: sin usuario en la dirección", rutaEnElAlmacen("https://almacen.ejemplo.com:8000", "viejo/historico"), { valor: "viejo/historico", usuario: "viejo" });

console.log("\n· 15dd: una sola ficha por almacén");
{
  const vistas = destinosDelCliente(equipos);
  const principal = vistas.find((v) => v.clave === claveZona(almacen.id, "principal"))!;
  igual("el «Servidor de copias» no sale aparte", vistas.some((v) => v.clave === "servidor-copias"), false);
  igual("cae en la zona principal del almacén", principal.ids.includes("servidor-copias"), true);
  igual("y se marca como adoptado (también localhost desde el propio almacén)", principal.adoptados, ["propio", "servidor-copias"]);
  const repos = reposEnDestino(principal, equipos).map((x) => [x.repo.nombre, !!x.adoptado]);
  igual("la ficha tiene todos sus repositorios, marcando los adoptados", repos, [["Copias de la consola", true], ["Contabilidad", false], ["Histórico", true]]);
  igual("el NAS sigue siendo otro destino", vistas.some((v) => v.clave === "nas"), true);
  const l = lugarRepo(pc.resumen!.repositorios!.find((r) => r.id === "historico"), pc, equipos);
  igual("«Se guarda en» de un adoptado: el almacén, marcado", [l.clase, l.texto, l.detalle, l.adoptado], ["almacen", "Almacén ALMACEN-01 (otro equipo)", "adoptado · D:\\Copias", true]);
  igual("su clave lleva a la zona", l.clave, claveZona(almacen.id, "principal"));
  igual("en el espejo del almacén, por su carpeta", repoEnAlmacen(pc, pc.resumen!.repositorios!.find((r) => r.id === "historico")!, equipos), { almacen, nombre: "viejo/historico", zona: "principal" });
}

console.log("\n· 15k: ¿de quién es esta carpeta del almacén?");
{
  const c = contenidoAlmacen(almacen, equipos);
  igual("lo que hay: repositorios (también adoptados) y derivadas", [...c.keys()].sort(), ["principal|alm-propio/consola", "principal|pc-01-3f2a/contabilidad", "principal|viejo/historico", "z1a2b3c|pc-01-3f2a/contabilidad-d1"]);
  igual("un repositorio", duenoDeCarpeta(almacen, "pc-01-3f2a/contabilidad", equipos).texto, "Contabilidad · PC-01");
  igual("una copia derivada, con su nombre y su equipo", duenoDeCarpeta(almacen, "pc-01-3f2a/contabilidad-d1", equipos, "z1a2b3c").texto, "Contabilidad (copia derivada) · PC-01");
  const dv = duenoDeCarpeta(almacen, "pc-01-3f2a/contabilidad-d1", equipos, "z1a2b3c");
  igual("la derivada sabe su repositorio de origen", [dv.tipo, dv.repo?.id, dv.derivada?.id], ["derivada", "contabilidad", "d1"]);
  igual("un adoptado", [duenoDeCarpeta(almacen, "viejo/historico", equipos).texto, duenoDeCarpeta(almacen, "viejo/historico", equipos).adoptado], ["Histórico · PC-01", true]);
  igual("en otra zona: solo se sabe el equipo", [duenoDeCarpeta(almacen, "pc-01-3f2a/contabilidad-d1", equipos).tipo, duenoDeCarpeta(almacen, "pc-01-3f2a/contabilidad-d1", equipos).texto], ["equipo", "contabilidad-d1 · PC-01"]);
  igual("el usuario solo: todo lo del equipo", duenoDeCarpeta(almacen, "pc-01-3f2a", equipos).texto, "Todo lo de PC-01");
  igual("nadie: «no sé de qué equipo es» (no «no está en esta consola»)", duenoDeCarpeta(almacen, "otro-9z9z/suelto", equipos).de, "no sé de qué equipo es");
  igual("los nombres para los textos", nombresDelAlmacen(almacen, equipos).get("pc-01-3f2a/contabilidad-d1"), "Contabilidad (copia derivada) · PC-01");
  igual("«Nuevo espejo → Algunos repositorios»: la derivada con su nombre", opcionesRepos(almacen, "almacen", equipos, "z1a2b3c"), [{ valor: "pc-01-3f2a/contabilidad-d1", nombre: "Contabilidad (copia derivada)", equipo: "PC-01" }]);
  const t = { id: "t1", nombre: "Todo", activo: true, quien: "almacen", que: { tipo: "todos" }, adonde: { tipo: "carpeta", carpeta: "F:\\Espejo" }, cuando: {}, retencion: { modo: "nunca" }, freno: { pct: 10, min_archivos: 100, min_faltan: 20, accion: "confirmar" }, orden: 0 } as never;
  igual(
    "«Qué guarda» de un espejo: cada carpeta con su dueño",
    reposDeTrabajo(t, almacen, equipos).map((r) => [r.valor, r.nombre, r.de]),
    [
      ["alm-propio/consola", "Copias de la consola", "ALMACEN-01"],
      ["otro-9z9z/suelto", "otro-9z9z/suelto", "no sé de qué equipo es"],
      ["pc-01-3f2a/contabilidad", "Contabilidad", "PC-01"],
      ["viejo/historico", "Histórico", "PC-01"],
    ],
  );
  igual("los nombres de los textos (órdenes, avisos) también ven las derivadas", mapaNombres(almacen, equipos).get("pc-01-3f2a/contabilidad-d1"), "Contabilidad (copia derivada), de PC-01");
}

console.log("\n· 15s: la retención del almacén (con lo que no la tiene) y sus nubes");
{
  const alm: Equipo = JSON.parse(JSON.stringify(almacen));
  alm.resumen!.guarda_copias!.retenciones = [{ usuario: "pc-01-3f2a", repo: "contabilidad", retencion: { diarias: 7, semanales: 4, mensuales: 12, anuales: 2 }, horario: { dias: [7], hora: "03:00" }, proxima: "2026-10-25T03:00:00Z" }];
  const r = retencionesDelAlmacen(alm, [alm, pc]);
  igual("el almacén manda la lista", r.admite, true);
  igual("la que tiene, con su dueño", r.con.map((x) => x.dueno.texto), ["Contabilidad · PC-01"]);
  igual("lo que no la tiene: repositorios (también adoptados) y derivadas", r.sin.map((x) => [x.texto, x.tipo, !!x.adoptado]), [["Copias de la consola · ALMACEN-01", "repo", true], ["Contabilidad (copia derivada) · PC-01", "derivada", false], ["Histórico · PC-01", "repo", true]]);
  igual("un almacén anterior no la manda: no puede aplicarla", retencionesDelAlmacen(almacen, equipos).admite, false);

  alm.resumen!.nubes = [{ nombre: "B2 Oficina", tipo: "b2" }];
  alm.resumen!.guarda_copias!.nubes = [{ nombre: "B2 Oficina", tipo: "b2" }];
  alm.resumen!.guarda_copias!.espejo = {
    hora: "02:00",
    trabajos: [
      { id: "t1", nombre: "Todo a B2", activo: true, quien: "almacen", que: { tipo: "todos" }, adonde: { tipo: "nube", nube: "B2 Oficina", carpeta: "oficina-copias/alm" }, cuando: {}, retencion: { modo: "nunca" }, freno: { pct: 10, min_archivos: 100, min_faltan: 20, accion: "confirmar" }, orden: 0, ultima: "2026-10-19T02:00:00Z", resultado: "Espejo hecho.", ocupa: { bytes: 5000, repos: { "pc-01-3f2a/contabilidad": 4000 }, cuando: "2026-10-19T02:00:00Z" } },
    ],
  } as never;
  const cat = [{ id: `nube:${alm.id}:b2-oficina`, nombre: "B2 Oficina", tipo: "nube" as const, donde: "oficina-copias", carpeta: "alm" }];
  const [n] = nubesVista(alm, [alm, pc], cat, "c1");
  igual("la nube: tipo, bucket y carpeta base (del catálogo)", [n.nombre, n.tipoTexto, n.bucket, n.carpetaBase], ["B2 Oficina", "Backblaze B2", "oficina-copias", "alm"]);
  igual("qué la usa", n.usos, ["el espejo «Todo a B2»"]);
  igual("no se puede desconectar mientras la usa un espejo", !!n.motivo, true);
  igual("lo que ocupa allí", n.ocupa, 5000);
  igual("la conexión, por la última subida", [n.conexion.tono, n.conexion.texto], ["ok", "Responde"]);
  const c = n.espejos[0].repos.find((x) => x.valor === "pc-01-3f2a/contabilidad")!;
  igual("lo que guarda cada espejo, con su ruta en la nube y su tamaño allí", [c.nombre, c.de, c.ruta, c.bytes], ["Contabilidad", "PC-01", "oficina-copias/alm/pc-01-3f2a/contabilidad", 4000]);
  igual("una subida que falló", conexionDe([{ trabajo: { ultima: "2026-10-19T02:00:00Z", resultado: "ERROR: token caducado", activo: true } as never }]), { tono: "bad", texto: "La última subida falló", cuando: "2026-10-19T02:00:00Z", error: "token caducado" });
}

console.log("\n· 15h: «Ordenar nombres de carpeta», con lo que volverían a subir los espejos");
{
  const uuid = "e2e2e2e2-0000-4000-8000-00000000";
  const alm: Equipo = JSON.parse(JSON.stringify(almacen));
  alm.resumen!.admite = ["nombres_legibles", "espejo_trabajos"];
  alm.resumen!.guarda_copias!.espejo = {
    hora: "02:00",
    trabajos: [
      { id: "t1", nombre: "Disco E", activo: true, quien: "almacen", que: { tipo: "todos" }, adonde: { tipo: "carpeta", carpeta: "E:\\Espejo" }, cuando: {}, retencion: { modo: "retraso", dias: 30 }, freno: { pct: 10, min_archivos: 100, min_faltan: 20, accion: "confirmar" }, orden: 0, ocupa: { bytes: 9000, repos: { [`${uuid}/ventas-7c2a`]: 7000 }, cuando: "2026-10-19T02:00:00Z" } },
      { id: "t2", nombre: "Solo otra cosa", activo: true, quien: "almacen", que: { tipo: "repos", repos: ["pc-01-3f2a/contabilidad"] }, adonde: { tipo: "carpeta", carpeta: "F:\\Otro" }, cuando: {}, retencion: { modo: "nunca" }, freno: { pct: 10, min_archivos: 100, min_faltan: 20, accion: "confirmar" }, orden: 1 },
    ],
  } as never;
  const pc2: Equipo = {
    ...base,
    id: "e2e2e2e2-0000-4000-8000-000000000003",
    nombre: "PC-02",
    resumen: {
      destinos: [{ id: "almacen-a1b2c3d4", nombre: "ALMACEN-01", tipo: "rest", donde: `https://almacen.ejemplo.com:8000/${uuid}/`, equipo_almacen: alm.id }],
      repositorios: [repo("ventas", "Ventas", "almacen-a1b2c3d4", { ruta: "ventas-7c2a", bytes: 5000 }), repo("caja", "Caja", "almacen-a1b2c3d4", { bytes: 300 })],
    } as never,
  };
  const l = porOrdenar(alm, [alm, pc, pc2]);
  igual("solo el equipo con nombres antiguos", l.map((x) => x.en.equipo.nombre), ["PC-02"]);
  igual("qué se movería: la carpeta del equipo y un repositorio", [l[0]?.plan.usuario, l[0]?.plan.repos.map((r) => `${r.de} → ${r.propuesta}`)], [true, ["ventas-7c2a → ventas"]]);
  igual("qué espejos volverían a subir y cuánto (lo de allí o lo del almacén)", l[0]?.resubidas.map((r) => [r.trabajo.nombre, r.bytes, r.repos]), [["Disco E", 7300, 2]]);
  const solo = porOrdenar(alm, [alm, pc, pc2], pc2.id, "ventas");
  igual("desde el repositorio: solo el suyo (sin la carpeta del equipo)", [solo[0]?.plan.usuario, solo[0]?.resubidas.map((r) => [r.trabajo.nombre, r.bytes])], [false, [["Disco E", 7000]]]);
  igual("sin «nombres_legibles» en el almacén, nada", porOrdenar(almacen, [almacen, pc2]), []);
}

console.log(`\n${total - fallos}/${total} bien`);
if (fallos) process.exit(1);
