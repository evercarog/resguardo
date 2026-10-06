// Pruebas de los destinos de primera clase y las zonas del almacén (tareas
// 7a y 7b, src/lib/destinos.ts; docs/copias-en-cadena.md).
// `npm run test:vectores` (con las demás).
import type { DestinoCatalogo, DestinoResumen, Equipo } from "../src/lib/tipos";
import {
  admiteZonas,
  claveNube,
  claveZona,
  destinosDelCliente,
  errorDondeCatalogo,
  errorNombreDestino,
  errorRespuestaZona,
  errorZonaNueva,
  idDestinoZona,
  nombreDestino,
  puertoDe,
  puertoParaZona,
  seSolapan,
  unidadDe,
  zonaDeDestino,
  zonasDe,
  zonasNuevasPara,
} from "../src/lib/destinos";
import { lugarDe } from "../src/lib/dondeGuarda";
import { actividadDestino, claveDeDestino, hrefDestino, reposEnDestino, usarEnCopia, usosDeDestino, vistaPorClave } from "../src/lib/fichaDestino";
import { esDestructiva } from "../src/lib/cripto/ordenes";

let fallos = 0;
let total = 0;
function igual(nombre: string, obtenido: unknown, esperado: unknown) {
  total++;
  const ok = JSON.stringify(obtenido) === JSON.stringify(esperado);
  if (!ok) fallos++;
  console.log(`${ok ? "ok  " : "MAL "} ${nombre}${ok ? "" : `\n       obtenido: ${JSON.stringify(obtenido)}\n       esperado: ${JSON.stringify(esperado)}`}`);
}
const cierto = (nombre: string, v: boolean) => igual(nombre, v, true);

const base = { so: "Windows 11", version_agente: "0.7.23", box_pub: "", sign_pub: "", sal_equipo: "", etiqueta: null, modo: "gestionado" as const, confirmado: true, conectado: true, ultimo_contacto: null, estado_servicio: "en_marcha" as const, siguiente_seq: 1 };
const almacen: Equipo = {
  ...base,
  id: "0b5c1f8e-1d2a-4c3b-9e8f-7a6b5c4d3e2f",
  nombre: "ALMACEN-01",
  rol: "almacenamiento",
  resumen: {
    admite: ["zonas_almacen"],
    guarda_copias: {
      activo: true,
      puerto: 8000,
      carpeta: "D:\\Resguardo",
      usuarios: 2,
      zonas: [{ id: "z1a2b3c", nombre: "Disco E", carpeta: "E:\\Resguardo", puerto: 8002, usuarios: 1, escucha: true, repositorios: [{ usuario: "recepcion-2", repos: ["documentos"] }] }],
      nubes: [{ nombre: "Dropbox Oficina", tipo: "dropbox" }],
    },
  },
};
const enD: DestinoResumen = { id: "almacen-0b5c1f8e", nombre: "ALMACEN-01", tipo: "rest", donde: "https://192.168.1.20:8000/recepcion/", equipo_almacen: almacen.id };
const enE: DestinoResumen = { id: "almacen-0b5c1f8e-z1a2b3c", nombre: "ALMACEN-01 · Disco E", tipo: "rest", donde: "https://192.168.1.20:8002/recepcion-2/", equipo_almacen: almacen.id };
const b2: DestinoResumen = { id: "destino-1a2b3c4d", nombre: "B2 de la oficina", tipo: "b2", donde: "copias-sur" };
const recepcion: Equipo = { ...base, id: "e-recepcion", nombre: "RECEPCION", rol: "agente", resumen: { destinos: [enD, enE, b2], repositorios: [] } };
const caja: Equipo = { ...base, id: "e-caja", nombre: "CAJA", rol: "agente", resumen: { destinos: [{ ...enD, donde: "https://192.168.1.20:8000/caja/" }], repositorios: [] } };
const equipos = [almacen, recepcion, caja];

console.log("\n· Zonas del almacén (7b)");
igual("unidadDe", [unidadDe("e:\\copias"), unidadDe("/srv/x"), unidadDe(null)], ["E:", null, null]);
igual("puertoDe", [puertoDe("https://192.168.1.20:8002/x/"), puertoDe("rest:https://nas:8000/"), puertoDe("https://nas/x"), puertoDe("https://[::1]:8004/a/"), puertoDe("nas:99999/x")], [8002, 8000, null, 8004, null]);
const zonas = zonasDe(almacen);
igual("la principal primero y después la zona", zonas.map((z) => [z.id, z.nombre, z.puerto, z.principal]), [["principal", "Disco D", 8000, true], ["z1a2b3c", "Disco E", 8002, false]]);
cierto("admite zonas", admiteZonas(almacen) && !admiteZonas(recepcion));
igual("un destino en la zona E se reconoce por el puerto", zonaDeDestino(enE, equipos)?.id, "z1a2b3c");
igual("…y uno de siempre, en la principal", zonaDeDestino(enD, equipos)?.id, "principal");
igual("…un destino de la nube no es de ningún almacén", zonaDeDestino(b2, equipos), null);
igual("id del destino de una zona (la principal, como siempre)", [idDestinoZona(almacen.id, "principal"), idDestinoZona(almacen.id, "z1a2b3c")], ["almacen-0b5c1f8e", "almacen-0b5c1f8e-z1a2b3c"]);
igual("puerto propuesto para otra zona: de dos en dos, libre", puertoParaZona(almacen), 8004);
igual("…sin zonas, 8002", puertoParaZona({ ...almacen, resumen: { guarda_copias: { activo: true, puerto: 8000 } } }), 8002);
igual("zonas a las que CAJA aún no copia: solo la E", zonasNuevasPara(caja, equipos).map((z) => z.id), ["z1a2b3c"]);
igual("RECEPCION ya copia en las dos", zonasNuevasPara(recepcion, equipos).length, 0);
const viejo: Equipo = { ...almacen, resumen: { ...almacen.resumen, admite: [] } };
igual("con un agente anterior, solo la principal se ofrece", zonasNuevasPara(caja, [viejo, recepcion, caja]).map((z) => z.id), []);
const lugarE = lugarDe(enE, recepcion, equipos);
igual("«Se guarda en» dice la zona y su carpeta", [lugarE.clase, lugarE.texto, lugarE.detalle], ["almacen", "Almacén ALMACEN-01 · Disco E (otro equipo)", "E:\\Resguardo"]);
igual("…y la principal, como siempre", lugarDe(enD, recepcion, equipos).texto, "Almacén ALMACEN-01 (otro equipo)");

console.log("\n· Una zona nueva (como lo comprueba el agente)");
igual("en otro disco, con otro puerto: bien", errorZonaNueva(almacen, "F:\\Resguardo", 8004), null);
cierto("dentro de la principal, no", !!errorZonaNueva(almacen, "d:\\resguardo\\otra", 8004));
cierto("la principal dentro de ella, tampoco", !!errorZonaNueva(almacen, "D:\\", 8004));
cierto("solapada con la zona E (sin distinguir mayúsculas)", errorZonaNueva(almacen, "e:\\RESGUARDO", 8004)?.includes("otra zona") === true);
igual("con el puerto de la zona E, propone otro", errorZonaNueva(almacen, "F:\\Resguardo", 8002), "El puerto 8002 ya es de este almacén: elige otro (por ejemplo, 8004).");
cierto("puerto bajo, no", !!errorZonaNueva(almacen, "F:\\Resguardo", 80));
cierto("seSolapan por partes, no por texto", !seSolapan("E:\\Resguardo-2", "E:\\Resguardo", true) && seSolapan("/srv/a/b", "/srv/a", false) && !seSolapan("/srv/A", "/srv/a", false));

console.log("\n· La respuesta del almacén es de la zona pedida");
const zE = zonas[1];
igual("bien", errorRespuestaZona({ zona: "z1a2b3c", destino: { donde: "https://192.168.1.20:8002/caja/" } }, zE), null);
cierto("un agente anterior ignora `zona`: no se crea nada", !!errorRespuestaZona({ destino: { donde: "https://192.168.1.20:8000/caja/" } }, zE));
cierto("otra dirección: tampoco", !!errorRespuestaZona({ zona: "z1a2b3c", destino: { donde: "https://192.168.1.20:8000/caja/" } }, zE));
igual("la principal, sin zona", errorRespuestaZona({ destino: { donde: "https://x:8000/a/" } }, zonas[0]), null);

console.log("\n· Destinos del cliente (7a)");
const catalogo: DestinoCatalogo[] = [
  { id: claveZona(almacen.id, "z1a2b3c"), nombre: "Almacén · Disco E (rápido)", tipo: "zona" },
  { id: "destino-9f8e7d6c", nombre: "Wasabi de la sede", tipo: "s3", donde: "s3.wasabisys.com/copias-sede" },
  { id: claveZona(almacen.id, "z0000ff"), nombre: "Una zona que ya no está", tipo: "zona" },
  { id: claveNube(almacen.id, "Dropbox Oficina"), nombre: "Dropbox de la oficina", tipo: "nube" },
];
const vistas = destinosDelCliente(equipos, catalogo);
igual(
  "zonas primero, después los de los equipos y los sueltos, y las nubes",
  vistas.map((v) => [v.clase, v.nombre]),
  [
    ["zona", "Almacén ALMACEN-01 · Disco D"],
    ["zona", "Almacén · Disco E (rápido)"],
    ["equipo", "B2 de la oficina"],
    ["suelto", "Wasabi de la sede"],
    ["nube", "Dropbox de la oficina"],
  ],
);
const vD = vistas[0];
igual("la principal junta los destinos de RECEPCION y CAJA", [vD.ids, vD.equipos], [["almacen-0b5c1f8e"], ["RECEPCION", "CAJA"]]);
igual("la zona E, con su nombre del catálogo", [vistas[1].renombrado, vistas[1].nombrePorDefecto, vistas[1].ids], [true, "Almacén ALMACEN-01 · Disco E", ["almacen-0b5c1f8e-z1a2b3c"]]);
igual("un suelto lleva su dirección y aún no lo usa nadie", [vistas[3].donde, vistas[3].equipos], ["s3.wasabisys.com/copias-sede", []]);
igual("el nombre de un destino de un equipo, el del catálogo", nombreDestino(enE, equipos, catalogo), "Almacén · Disco E (rápido)");
igual("…o el de siempre", nombreDestino(enD, equipos, catalogo), "Almacén ALMACEN-01 · Disco D");
igual("sin catálogo (servidor anterior), igual pero sin nombres ni sueltos", destinosDelCliente(equipos).map((v) => v.nombre), ["Almacén ALMACEN-01 · Disco D", "Almacén ALMACEN-01 · Disco E", "B2 de la oficina", "Dropbox Oficina"]);
igual("un nombre igual al de siempre no cuenta como cambiado", destinosDelCliente(equipos, [{ id: "destino-1a2b3c4d", nombre: "B2 de la oficina", tipo: "b2", donde: "copias-sur" }]).find((v) => v.clave === "destino-1a2b3c4d")?.renombrado, false);
igual("claveNube sin acentos ni espacios", claveNube("e1", "Dropbox Oficina Ñandú"), "nube:e1:dropbox-oficina-nandu");

console.log("\n· Lo que el catálogo no admite (como el servidor)");
igual("bucket", errorDondeCatalogo("b2", "copias-sur"), null);
igual("servidor de copias", errorDondeCatalogo("rest", "https://copias.ejemplo.com:8000"), null);
igual("SFTP con usuario (no es secreto)", errorDondeCatalogo("sftp", "copias@nas.ejemplo.com:/copias"), null);
cierto("con contraseña en la dirección, no", !!errorDondeCatalogo("rest", "https://ana:clave@copias.ejemplo.com:8000"));
cierto("una carpeta local, no", !!errorDondeCatalogo("rest", "D:\\Copias") && !!errorDondeCatalogo("rest", "\\\\nas\\copias") && !!errorDondeCatalogo("sftp", "/srv/copias"));
cierto("vacío, no", !!errorDondeCatalogo("s3", "  "));
igual("nombres", [errorNombreDestino("Dropbox Oficina"), !!errorNombreDestino(" "), !!errorNombreDestino("x".repeat(81))], [null, true, true]);

console.log("\n· Quitar una zona espera");
cierto("quitar_zona es destructiva", esDestructiva("guarda_copias", { quitar_zona: "z1a2b3c" }));
cierto("quitar un equipo de una zona también", esDestructiva("guarda_copias", { quitar: "caja", zona: "z1a2b3c" }));
cierto("crear o renombrar una zona, no", !esDestructiva("guarda_copias", { zona: { carpeta: "E:\\Resguardo", puerto: 8002 } }) && !esDestructiva("guarda_copias", { zona: { id: "z1a2b3c", nombre: "Disco E" } }));
cierto("añadir un equipo a una zona, no", !esDestructiva("guarda_copias", { anadir: "e-caja", zona: "z1a2b3c" }));

console.log("\n· La página de un destino (lib/fichaDestino.ts)");
{
  const hace = (h: number) => new Date(Date.now() - h * 3_600_000).toISOString();
  const repo = (id: string, destino: string, extra: object = {}) => ({ id, nombre: id, destino, versiones: 3, ...extra }) as never;
  const alm2: Equipo = {
    ...almacen,
    resumen: {
      ...almacen.resumen,
      admite: ["zonas_almacen", "espejo_zonas"],
      guarda_copias: {
        ...almacen.resumen!.guarda_copias!,
        espejo: { hora: "23:00", destinos: [{ tipo: "nube", nube: "Dropbox Oficina", ultima: hace(5), resultado: "ok" }, { tipo: "zona", carpeta: "z1a2b3c", ultima: hace(30), resultado: "fallo" }] },
      },
    },
  };
  const rec2 = {
    ...recepcion,
    resumen: {
      admite: ["derivadas", "nube_equipo"],
      destinos: [enD, enE, b2],
      repositorios: [repo("documentos", enD.id, { externa: { destino: "B2 de la oficina", destino_id: b2.id, hora: "23:00" } }), repo("contabilidad", enE.id)],
      copias: [
        { id: "k1", nombre: "Documentos", repo: "documentos", activa: true, ultima: { cuando: hace(2), estado: "ok" } },
        { id: "k2", nombre: "Contabilidad", repo: "contabilidad", activa: true, ultima: { cuando: hace(1), estado: "fallo", mensaje: "sin red" } },
      ],
      nubes: [],
    },
  } as unknown as Equipo;
  const eqs = [alm2, rec2, caja];
  const vs = destinosDelCliente(eqs);
  const porClave = (k: string) => vistaPorClave(k, eqs, [])!;
  const vD2 = porClave(claveZona(almacen.id, "principal"));
  const vE2 = porClave(claveZona(almacen.id, "z1a2b3c"));
  const vNube = porClave(claveNube(almacen.id, "Dropbox Oficina"));
  const vB2 = porClave(b2.id);
  igual("la dirección de su página (la clave, codificada)", hrefDestino("c1", claveZona(almacen.id, "principal")), `/c/c1/destinos/zona%3A${almacen.id}%3Aprincipal`);
  igual("la clave de un destino de un equipo: su zona o su id", [claveDeDestino(enE, eqs), claveDeDestino(b2, eqs)], [claveZona(almacen.id, "z1a2b3c"), b2.id]);
  igual("todos los destinos tienen página", vs.every((v) => vistaPorClave(v.clave, eqs, [])?.clave === v.clave), true);
  igual("una clave que ya no está: ninguna", vistaPorClave("zona:otro:principal", eqs, []), null);
  igual("los repositorios de cada zona", [reposEnDestino(vD2, eqs).map((x) => x.repo.id), reposEnDestino(vE2, eqs).map((x) => x.repo.id)], [["documentos"], ["contabilidad"]]);
  igual("lo que usa el B2: la copia externa de «documentos»", usosDeDestino(vB2, eqs, "c1").map((u) => [u.tipo, u.texto]), [["externa", "Copia externa de «documentos» (RECEPCION)"]]);
  igual("la nube del almacén: le llega el espejo de la principal", usosDeDestino(vNube, eqs, "c1").map((u) => [u.tipo, u.resultado]), [["espejo_entra", "ok"]]);
  igual(
    "la zona principal sale a la nube y a la E; a la E le llega",
    [usosDeDestino(vD2, eqs, "c1").map((u) => u.tipo), usosDeDestino(vE2, eqs, "c1").map((u) => u.tipo)],
    [["espejo_sale", "espejo_sale"], ["espejo_entra"]],
  );
  igual("lo que sale lleva a la página del otro destino", usosDeDestino(vD2, eqs, "c1")[0].href, hrefDestino("c1", claveNube(almacen.id, "Dropbox Oficina")));
  const act = actividadDestino(vE2, eqs, "c1");
  igual("lo último de la zona E: la copia que falló y el espejo que llega, lo más reciente primero", act.map((x) => [x.tono, x.texto]), [
    ["bad", "Copia «Contabilidad» de RECEPCION"],
    ["bad", "Espejo de Disco D del almacén ALMACEN-01"],
  ]);
  igual("…con el motivo", act[0].detalle, "sin red");
  const usar = usarEnCopia(vNube, eqs, "c1");
  const q = (v: { clave: string }) => `destino=${encodeURIComponent(v.clave)}`;
  igual(
    "usar la nube del almacén: el espejo de cada repositorio del almacén o una copia a partir de él",
    usar.map((u) => u.href),
    ["paso_espejo=documentos", "derivada=documentos", "paso_espejo=contabilidad", "derivada=contabilidad"].map((x) => `/c/c1/equipos/e-recepcion?${x}&${q(vNube)}`),
  );
  cierto("…y dice que hay que conectar la nube en el equipo dueño si no la tiene", usar[1].detalle.includes("conectar «Dropbox Oficina» también"));
  igual(
    "usar una zona: una copia nueva de cada equipo (no del propio almacén) y, de lo que está en otra zona, espejo o repositorio a partir de él (no lo que ya está aquí)",
    usarEnCopia(vE2, eqs, "c1").map((u) => u.texto),
    ["Copia nueva de RECEPCION", "Copia nueva de CAJA", "Espejo de «documentos» (RECEPCION)", "Repositorio nuevo a partir de «documentos» (RECEPCION)"],
  );
  igual(
    "usar un destino de un equipo: copia nueva desde quien lo tiene y repositorios a partir de los suyos",
    usarEnCopia(vB2, eqs, "c1").map((u) => u.href),
    [`/c/c1/equipos/e-recepcion/copias?nueva=1&${q(vB2)}`, `/c/c1/equipos/e-recepcion?derivada=documentos&${q(vB2)}`, `/c/c1/equipos/e-recepcion?derivada=contabilidad&${q(vB2)}`],
  );
  cierto("un agente sin copias derivadas: no se ofrecen", usarEnCopia(vB2, [alm2, { ...rec2, resumen: { ...rec2.resumen!, admite: [] } }, caja], "c1").every((u) => !u.href.includes("derivada=")));
}

console.log(`\n${fallos ? "MAL" : "ok"}: ${total - fallos}/${total} (destinos y zonas)`);
if (fallos) process.exit(1);
