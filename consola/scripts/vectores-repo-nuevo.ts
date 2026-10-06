// Pruebas de «Nuevo repositorio» (src/lib/repoNuevo.ts): todos los destinos,
// también las nubes (tarea 4a), con si sirven desde el equipo y qué hacer si no.
// `npm run test:vectores` (con las demás).
import type { DestinoCatalogo, DestinoResumen, Equipo } from "../src/lib/tipos";
import { destinoNubeCuerpo, errorCarpetaNube, idDestinoNube, nubeNoInmutable, opcionesRepoNuevo, opcionInicial, type OpcionRepo } from "../src/lib/repoNuevo";

let fallos = 0;
let total = 0;
function igual(nombre: string, obtenido: unknown, esperado: unknown) {
  total++;
  const ok = JSON.stringify(obtenido) === JSON.stringify(esperado);
  if (!ok) fallos++;
  console.log(`${ok ? "ok  " : "MAL "} ${nombre}${ok ? "" : `\n       obtenido: ${JSON.stringify(obtenido)}\n       esperado: ${JSON.stringify(esperado)}`}`);
}
const cierto = (nombre: string, v: boolean) => igual(nombre, v, true);

const base = { so: "Windows 11", version_agente: "0.7.25", box_pub: "", sign_pub: "", sal_equipo: "", etiqueta: null, modo: "gestionado" as const, confirmado: true, conectado: true, ultimo_contacto: null, estado_servicio: "en_marcha" as const, siguiente_seq: 1 };
const NUEVO = ["cadenas", "derivadas", "nube_equipo", "repo_en_nube"];
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
      zonas: [{ id: "z1a2b3c", nombre: "Disco E", carpeta: "E:\\Resguardo", puerto: 8002, usuarios: 0 }],
      nubes: [{ nombre: "Dropbox Oficina", tipo: "dropbox" }],
    },
  },
};
const apps = (admite = NUEVO, nubes: { nombre: string; tipo: string }[] = [], destinos: DestinoResumen[] = []): Equipo => ({
  ...base,
  id: "e-apps",
  nombre: "SERVIDOR-APPS",
  rol: "agente",
  resumen: { admite, nubes, destinos, repositorios: [], copias: [] },
});
const catalogo: DestinoCatalogo[] = [{ id: "destino-b2b2b2b2", nombre: "B2 de la oficina", tipo: "b2", donde: "copias-sur", actualizado: "", por: "" }];
const ver = (l: OpcionRepo[]) => l.map((o) => [o.valor, o.uso.ok, o.uso.accion?.tipo ?? null]);

console.log("\n· Un equipo nuevo sin nubes, con el almacén y su Dropbox");
{
  const e = apps();
  const l = opcionesRepoNuevo(e, [almacen, e], catalogo);
  igual(
    "todos, en orden: el almacén (recomendado), su otra zona, el catálogo, la Dropbox del almacén y uno nuevo",
    ver(l),
    [
      [`almacen:${almacen.id}`, true, null],
      [`zona:${almacen.id}:z1a2b3c`, true, null],
      ["catalogo:destino-b2b2b2b2", true, null],
      ["no:nube:Dropbox Oficina", false, "conectar_nube"],
      ["nuevo", true, null],
    ],
  );
  const dbx = l.find((o) => o.nombre === "Dropbox Oficina")!;
  igual("la Dropbox del almacén: «Conectar Dropbox también en SERVIDOR-APPS»", [dbx.uso.motivo, dbx.uso.accion && "texto" in dbx.uso.accion ? dbx.uso.accion.texto : null, dbx.detalle], [
    "Hace falta también en SERVIDOR-APPS",
    "Conectar Dropbox también en SERVIDOR-APPS",
    "Dropbox · conectada en ALMACEN-01",
  ]);
  igual("…la conexión es para este equipo y con el mismo nombre", dbx.uso.accion?.tipo === "conectar_nube" ? [dbx.uso.accion.equipo.nombre, dbx.uso.accion.nube] : null, ["SERVIDOR-APPS", "Dropbox Oficina"]);
  igual("se propone el almacén", opcionInicial(l), `almacen:${almacen.id}`);
  cierto("el almacén no se ofrece a sí mismo sin «almacen_propio»", !opcionesRepoNuevo(almacen, [almacen, e]).some((o) => o.valor === `almacen:${almacen.id}`));
}

console.log("\n· Ya conectada en el equipo");
{
  const e = apps(NUEVO, [{ nombre: "Dropbox Oficina", tipo: "dropbox" }]);
  const dbx = opcionesRepoNuevo(e, [almacen, e]).find((o) => o.nombre === "Dropbox Oficina")!;
  igual("se elige; dice que no es inmutable", [dbx.valor, dbx.uso.ok, dbx.uso.motivo, dbx.que], ["nube:Dropbox Oficina", true, "No es inmutable", { tipo: "nube", nube: "Dropbox Oficina", tipoNube: "dropbox" }]);
  igual("…una sola vez (aunque esté en el almacén y en el equipo)", opcionesRepoNuevo(e, [almacen, e]).filter((o) => o.nombre === "Dropbox Oficina").length, 1);
  // Ya tiene un repositorio en ella: el destino del equipo, sin repetirla como nube nueva.
  const conDestino = apps(NUEVO, [{ nombre: "Dropbox Oficina", tipo: "dropbox" }], [{ id: "nube-dropbox-oficina-1a2b", nombre: "Dropbox Oficina", tipo: "nube", nube: "Dropbox Oficina", donde: "Resguardo" }]);
  const l = opcionesRepoNuevo(conDestino, [almacen, conDestino]);
  igual("con un destino en ella: ese destino, y no otra vez como nube", l.filter((o) => o.nombre === "Dropbox Oficina").map((o) => [o.valor, o.uso.ok, o.clase]), [["nube-dropbox-oficina-1a2b", true, "nube"]]);
}

console.log("\n· Un agente anterior (sin «repo_en_nube»)");
{
  const e = apps(["cadenas", "derivadas", "nube_equipo"], [{ nombre: "Dropbox Oficina", tipo: "dropbox" }], [{ id: "nube-x", nombre: "Dropbox Oficina", tipo: "nube", nube: "Dropbox Oficina", donde: "Resguardo" }]);
  const l = opcionesRepoNuevo(e, [almacen, e]);
  igual("su nube, desactivada: «Actualiza el agente»", l.filter((o) => o.clase === "nube").map((o) => [o.valor, o.uso.ok, o.uso.motivo]), [["no:nube-x", false, "Actualiza el agente de SERVIDOR-APPS para copiar directo a una nube"]]);
  cierto("…lo demás sigue igual", l.some((o) => o.valor === `almacen:${almacen.id}` && o.uso.ok));
}

console.log("\n· Ninguna Dropbox en el cliente");
{
  const sinNubes: Equipo = { ...almacen, resumen: { ...almacen.resumen, guarda_copias: { ...almacen.resumen!.guarda_copias!, nubes: [] } } };
  const e = apps();
  const x = opcionesRepoNuevo(e, [sinNubes, e]).find((o) => o.valor === "conectar:dropbox")!;
  igual("«Conectar Dropbox» en este equipo", [x.nombre, x.uso.ok, x.uso.accion?.tipo, x.uso.accion && "texto" in x.uso.accion ? x.uso.accion.texto : null], ["Dropbox", false, "conectar_nube", "Conectar Dropbox en SERVIDOR-APPS"]);
  const viejo = apps([]);
  igual("…con un agente anterior: actualizarlo", opcionesRepoNuevo(viejo, [sinNubes, viejo]).find((o) => o.valor === "conectar:dropbox")?.uso.accion?.tipo, "actualizar");
}

console.log("\n· Otros tipos de nube conectados en el almacén");
{
  const conB2: Equipo = { ...almacen, resumen: { ...almacen.resumen, guarda_copias: { ...almacen.resumen!.guarda_copias!, nubes: [{ nombre: "B2 Espejo", tipo: "b2" }, { nombre: "NAS", tipo: "smb" }] } } };
  const e = apps();
  const l = opcionesRepoNuevo(e, [conB2, e]);
  igual(
    "B2 por rclone: mejor un destino de siempre; SMB: aún no desde la consola",
    l.filter((o) => o.clase === "nube" && o.nombre !== "Dropbox").map((o) => [o.nombre, o.uso.ok, o.uso.motivo]),
    [
      ["B2 Espejo", false, "Desde SERVIDOR-APPS: «Un destino nuevo…» de tipo Backblaze B2, con sus datos"],
      ["NAS", false, "En SERVIDOR-APPS, todavía no se conecta desde la consola"],
    ],
  );
  cierto("…y sin Dropbox en el cliente, también «Conectar Dropbox»", l.some((o) => o.valor === "conectar:dropbox"));
}

console.log("\n· El destino de la orden y la carpeta");
{
  const e = apps(NUEVO, [{ nombre: "Dropbox Oficina", tipo: "dropbox" }], [{ id: "nube-dropbox-oficina-1a2b", nombre: "Dropbox Oficina", tipo: "nube", nube: "Dropbox Oficina", donde: "Resguardo" }]);
  igual("la misma nube y carpeta: el destino que ya tiene", destinoNubeCuerpo(e, "Dropbox Oficina", "/Resguardo/"), { id: "nube-dropbox-oficina-1a2b" });
  igual("otra carpeta: uno nuevo", destinoNubeCuerpo(e, "Dropbox Oficina", "Copias/Sur", "9z9z"), { id: "nube-dropbox-oficina-9z9z", nombre: "Dropbox Oficina", tipo: "nube", nube: "Dropbox Oficina", donde: "Copias/Sur" });
  igual("id con acentos y espacios", idDestinoNube("Dropbox Café Ñandú", "0000"), "nube-dropbox-cafe-nandu-0000");
  igual("carpetas que valen", ["Resguardo", "Copias/Sur", "/Resguardo/"].map(errorCarpetaNube), [null, null, null]);
  cierto("…y las que no", ["", "../x", "a//b", "C:/x", "a\\b", "a/./b"].every((c) => !!errorCarpetaNube(c)));
  cierto("Dropbox no es inmutable; B2 sí", nubeNoInmutable("dropbox") && !nubeNoInmutable("b2") && !nubeNoInmutable(undefined));
}

console.log(`\n${total - fallos}/${total} bien`);
if (fallos) process.exit(1);
