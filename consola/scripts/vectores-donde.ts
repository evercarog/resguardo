// Pruebas de «¿Dónde se guardan las copias?» (src/lib/dondeGuarda.ts) y de
// cómo lo enseñan la salud de la protección, el mapa y «Todos los clientes».
// `npm run test:vectores` (con las demás).
import type { DestinoResumen, Equipo, RepoInforme } from "../src/lib/tipos";
import { comprobacionLugar, lineaLugar, lugarDe, lugarRepo, riesgoMismoEquipo, riesgosDelCliente, servidorDe } from "../src/lib/dondeGuarda";
import { proteccion } from "../src/lib/repo";
import { construirMapa } from "../src/lib/mapa";
import { atencion } from "../src/lib/global";

let fallos = 0;
let total = 0;
function igual(nombre: string, obtenido: unknown, esperado: unknown) {
  total++;
  const ok = JSON.stringify(obtenido) === JSON.stringify(esperado);
  if (!ok) fallos++;
  console.log(`${ok ? "ok  " : "MAL "} ${nombre}${ok ? "" : `\n       obtenido: ${JSON.stringify(obtenido)}\n       esperado: ${JSON.stringify(esperado)}`}`);
}
const cierto = (nombre: string, v: boolean) => igual(nombre, v, true);

console.log("\n· Dónde se guardan las copias (lib/dondeGuarda.ts)");
const base = { so: "Windows 11", version_agente: "0.7.18", box_pub: "", sign_pub: "", sal_equipo: "", etiqueta: null, modo: "gestionado" as const, confirmado: true, conectado: true, ultimo_contacto: null, estado_servicio: "en_marcha" as const, siguiente_seq: 1, rol: "agente" as const };
const equipo = (id: string, nombre: string, destino: DestinoResumen, extra: Partial<Equipo["resumen"] & object> = {}): Equipo => ({
  ...base,
  id,
  nombre,
  resumen: {
    destinos: [destino],
    repositorios: [{ id: "r", nombre: "Archivos", destino: destino.id }],
    copias: [{ id: "k", nombre: "Archivos", repo: "r", activa: true }],
    ...extra,
  },
});
const almacen: Equipo = { ...base, id: "alm", nombre: "ALMACEN-OFICINA", rol: "almacenamiento", resumen: { guarda_copias: { activo: true, carpeta: "E:\\Copias" } } };
const local = (d: Partial<DestinoResumen> = {}): DestinoResumen => ({ id: "disco", nombre: "Disco D", tipo: "local", unidad: "D:", extraible: false, red: false, ...d });
const servidor = equipo("srv", "SERVIDOR", local());
const todos = [servidor, almacen];

const l = lugarRepo(servidor.resumen!.repositorios![0], servidor, todos);
igual("carpeta del propio equipo: «en este mismo equipo (D:)»", [l.clase, l.texto, l.mismoEquipo], ["carpeta", "En este mismo equipo (D:)", true]);
igual("…y la línea entera", lineaLugar(l), "En este mismo equipo (D:) · Disco D");
igual("disco USB", lugarDe(local({ extraible: true, unidad: "E:" }), servidor, todos).texto, "Disco extraíble (E:) de este equipo");
igual("carpeta de la red: no es el mismo equipo", [lugarDe(local({ red: true }), servidor, todos).clase, lugarDe(local({ red: true }), servidor, todos).mismoEquipo], ["red", false]);
const alm = lugarDe({ id: "almacen-alm", nombre: "ALMACEN-OFICINA", tipo: "rest", donde: "https://10.0.0.5:8000/servidor/", equipo_almacen: "alm" }, servidor, todos);
igual("el almacén de otro equipo, con su carpeta", [alm.clase, alm.texto, alm.detalle, alm.mismoEquipo], ["almacen", "Almacén ALMACEN-OFICINA (otro equipo)", "E:\\Copias", false]);
igual("su propio almacén: el mismo equipo", lugarDe({ id: "almacen-alm", nombre: "ALMACEN-OFICINA", tipo: "rest", equipo_almacen: "alm" }, almacen, todos).clase, "almacen_propio");
igual("la nube, con su bucket", lineaLugar(lugarDe({ id: "b2", nombre: "B2", tipo: "b2", donde: "copias-x" }, servidor, todos)), "Nube (Backblaze B2) · bucket copias-x");
igual("un servidor de copias de fuera, con su máquina", lineaLugar(lugarDe({ id: "rs", nombre: "Proveedor", tipo: "rest", donde: "https://copias.ejemplo.com:8000/x/" }, servidor, todos)), "Servidor de copias externo · Proveedor · copias.ejemplo.com:8000");
igual("servidorDe quita el esquema de restic y el usuario", servidorDe("rest:https://ana@nas.local:8000/a/"), "nas.local:8000");

console.log("\n· Copias en el mismo equipo");
const r = riesgoMismoEquipo(servidor.resumen!.repositorios![0], servidor, todos);
cierto("en una carpeta del propio equipo, sin copia externa: aviso", !!r);
igual("…con la frase de siempre", r?.texto, "Las copias de SERVIDOR se guardan en el propio SERVIDOR: si ese equipo se daña o lo cifra un ransomware, se pierden las dos. Guárdalas en un almacén de otro equipo o añade una copia externa.");
igual("…sin nota si se sabe qué disco es", r?.nota, null);
const conExterna = { ...servidor.resumen!.repositorios![0], externa: { destino: "Nube", hora: "21:00" } };
igual("con copia externa: sin aviso", riesgoMismoEquipo(conExterna, servidor, todos), null);
const usb = equipo("u", "PORTATIL", local({ extraible: true }));
igual("en un disco USB: sin aviso", riesgoMismoEquipo(usb.resumen!.repositorios![0], usb, [usb]), null);
const antiguo = equipo("a", "VIEJO", { id: "disco", nombre: "Disco D", tipo: "local" });
const ra = riesgoMismoEquipo(antiguo.resumen!.repositorios![0], antiguo, [antiguo]);
cierto("un agente que no dice qué disco es: aviso igual, y pide actualizarlo", !!ra && /actualízalo/.test(ra.nota ?? ""));
const nula = equipo("n", "NULO", local({ extraible: null }));
cierto("no se pudo saber si es USB: aviso, con nota", /No se pudo saber/.test(riesgoMismoEquipo(nula.resumen!.repositorios![0], nula, [nula])?.nota ?? ""));
const sinCopias = equipo("s", "SIN", local(), { copias: [] });
igual("ninguna copia guarda ya ahí (tras moverlo): sin aviso", riesgoMismoEquipo(sinCopias.resumen!.repositorios![0], sinCopias, [sinCopias]), null);
const propio: Equipo = { ...almacen, resumen: { ...almacen.resumen!, destinos: [{ id: "almacen-alm", nombre: "ALMACEN-OFICINA", tipo: "rest", equipo_almacen: "alm" }], repositorios: [{ id: "c", nombre: "Consola", destino: "almacen-alm" }], copias: [{ id: "k", nombre: "Consola", repo: "c", activa: true }] } };
cierto("en su propio almacén sin espejo: aviso", !!riesgoMismoEquipo(propio.resumen!.repositorios![0], propio, [propio]));
const conEspejo: Equipo = { ...propio, resumen: { ...propio.resumen!, guarda_copias: { activo: true, espejo: { hora: "02:00", destinos: [{ tipo: "nube", nube: "Dropbox" }] } } } };
igual("…con espejo a otro sitio: sin aviso", riesgoMismoEquipo(conEspejo.resumen!.repositorios![0], conEspejo, [conEspejo]), null);
igual("riesgosDelCliente: solo los que se quedan en su equipo", riesgosDelCliente([servidor, usb, almacen], todos).map((x) => x.equipo.nombre), ["SERVIDOR"]);

console.log("\n· Salud de la protección: «Fuera de este equipo»");
const inf = (items: { id: string; estado: "ok" | "aviso" }[]): RepoInforme => ({ id: "r", nombre: "Archivos", versiones: [], versiones_leidas: null, ejecuciones: [], espacio: null, verificacion: null, prueba_restauracion: null, externa: null, proteccion: { puntuacion: items.filter((i) => i.estado === "ok").length, total: items.length, items: items.map((i) => ({ ...i, etiqueta: i.id, detalle: "" })) } });
const extra = comprobacionLugar(servidor.resumen!.repositorios![0], servidor, todos);
const p = proteccion(inf([{ id: "copias", estado: "ok" }]), extra);
igual("un agente anterior: la consola la suma (en aviso)", [p?.puntuacion, p?.total, p?.items.at(-1)?.id, p?.tono], [1, 2, "lugar", "warn"]);
const p2 = proteccion(inf([{ id: "copias", estado: "ok" }, { id: "lugar", estado: "aviso" }]), extra);
igual("si el agente ya la manda, no se repite", [p2?.total, p2?.items.filter((i) => i.id === "lugar").length], [2, 1]);
igual("fuera del equipo: nada que sumar", comprobacionLugar(conExterna, servidor, todos), null);

console.log("\n· El mapa y «Todos los clientes»");
const mapa = construirMapa([servidor, almacen], {}, { cliente: "c" });
const dest = mapa.nodos.find((n) => n.tipo === "destino");
igual("el disco del propio equipo dice de quién es y lo avisa", [dest?.sub, dest?.aviso], ["Disco de SERVIDOR (D:)", "En el mismo equipo que protege"]);
cierto("…y la alternativa en texto también", mapa.frases.some((f) => f.includes("en el mismo equipo que protege")));
const otro = equipo("o", "OTRO", local());
const m2 = construirMapa([servidor, otro], {}, { cliente: "c" });
igual("dos equipos con su «Disco D»: dos destinos, no uno", m2.nodos.filter((n) => n.tipo === "destino").length, 2);
const a = atencion([{ id: "c", nombre: "Cliente", rol: "propietario", equipos: [servidor, almacen], informes: [], informes_completos: true, avisos_abiertos: 0, pendientes: 0, marca: null }]);
cierto("«Todos los clientes» lo pone en lo que necesita atención", a.some((x) => x.titulo === "SERVIDOR · copias en el mismo equipo"));

console.log(`\n${total - fallos} de ${total} comprobaciones correctas.`);
if (fallos) process.exit(1);
