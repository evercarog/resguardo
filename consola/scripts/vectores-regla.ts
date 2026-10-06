// Pruebas de la regla 3-2-1-1-0 (tarea 8, src/lib/regla321.ts, docs/regla-3-2-1.md).
// 1. Los vectores compartidos con Rust (crates/protocolo/vectors/regla-321.json):
//    la misma regla que `protection::regla_321`.
// 2. La entrada de cada copia con lo que ve la consola (zona, espejo, copia
//    externa, catálogo) y los textos.
// `npm run test:vectores` (con las demás).
import { readFileSync } from "node:fs";
import type { DestinoCatalogo, DestinoResumen, Equipo, Informe, RepoInforme } from "../src/lib/tipos";
import {
  claveEspejoCarpeta,
  cuentaRegla,
  fraseRegla,
  horasEntre,
  margenHoras,
  queHacer,
  regla321,
  reglaDeCopia,
  reglasDelCliente,
  resumenCorto,
  textoEntorno,
  type EntradaRegla,
} from "../src/lib/regla321";

let fallos = 0;
let total = 0;
function igual(nombre: string, obtenido: unknown, esperado: unknown) {
  total++;
  const ok = JSON.stringify(obtenido) === JSON.stringify(esperado);
  if (!ok) fallos++;
  console.log(`${ok ? "ok  " : "MAL "} ${nombre}${ok ? "" : `\n       obtenido: ${JSON.stringify(obtenido)}\n       esperado: ${JSON.stringify(esperado)}`}`);
}
const cierto = (nombre: string, v: boolean) => igual(nombre, v, true);

/** Ordena las claves (los vectores de Rust no tienen el mismo orden). */
const ordenado = (v: unknown): unknown =>
  Array.isArray(v) ? v.map(ordenado) : v && typeof v === "object" ? Object.fromEntries(Object.entries(v).sort(([a], [b]) => a.localeCompare(b)).map(([k, x]) => [k, ordenado(x)])) : v;

console.log("\n· Vectores compartidos con Rust (crates/protocolo/vectors/regla-321.json)");
const doc = JSON.parse(readFileSync(new URL("../../crates/protocolo/vectors/regla-321.json", import.meta.url), "utf8")) as {
  ahora: string;
  vectores: { nombre: string; que: string; entrada: EntradaRegla; esperado: unknown }[];
};
const AHORA = Date.parse(doc.ahora);
cierto("hay vectores", doc.vectores.length >= 10);
for (const v of doc.vectores) {
  const r = regla321(v.entrada, AHORA);
  const sinDetalle = { ...r, partes: r.partes.map(({ detalle: _d, ...p }) => p) };
  igual(`${v.nombre}: ${v.que}`, ordenado(sinDetalle), ordenado(v.esperado));
}
igual("margen: cada día 48 h, cada hora 13,5 h, sin horario 48 h", [margenHoras(24), margenHoras(1), margenHoras(null), margenHoras(NaN)], [48, 13.5, 48, 48]);

console.log("\n· La entrada de una copia (lo que ve la consola)");
const iso = (h: number) => new Date(AHORA - h * 3600_000).toISOString();
const base = { so: "Windows 11", version_agente: "0.7.23", box_pub: "", sign_pub: "", sal_equipo: "", etiqueta: null, modo: "gestionado" as const, confirmado: true, conectado: true, ultimo_contacto: null, estado_servicio: "en_marcha" as const, siguiente_seq: 1 };
const almacen: Equipo = {
  ...base,
  id: "0b5c1f8e-1d2a-4c3b-9e8f-7a6b5c4d3e2f",
  nombre: "ALMACEN-01",
  rol: "almacenamiento",
  resumen: {
    admite: ["zonas_almacen", "espejo_flexible"],
    entorno: { virtual: "kvm", contenedor: "lxc" },
    guarda_copias: {
      activo: true,
      puerto: 8000,
      carpeta: "D:\\Resguardo",
      sistema_archivos: "NTFS",
      usuarios: 1,
      repositorios: [{ usuario: "recepcion", repos: ["documentos"] }],
      zonas: [{ id: "z1a2b3c", nombre: "Disco E", carpeta: "E:\\Resguardo", puerto: 8002, usuarios: 0, sistema_archivos: "ReFS" }],
      nubes: [{ nombre: "Dropbox Oficina", tipo: "dropbox" }],
      espejo: {
        hora: "02:00",
        destinos: [
          { tipo: "carpeta", carpeta: "F:\\Espejo", ultima: iso(9), resultado: "Copiados 3 archivos.", sistema_archivos: "NTFS", verificacion: { ultima: iso(9), archivos: 10, mal: 0 } },
          { tipo: "nube", nube: "Dropbox Oficina", carpeta: "Resguardo", ultima: iso(80), resultado: "ERROR: sin conexión", repos: ["recepcion/documentos"] },
          { tipo: "carpeta", carpeta: "G:\\Otros", ultima: iso(9), resultado: "ok", repos: ["caja/otro"] },
        ],
      },
    },
  },
};
const enD: DestinoResumen = { id: "almacen-0b5c1f8e", nombre: "ALMACEN-01", tipo: "rest", donde: "https://192.168.1.20:8000/recepcion/", equipo_almacen: almacen.id };
const b2: DestinoResumen = { id: "destino-1a2b3c4d", nombre: "B2 de la oficina", tipo: "b2", donde: "copias-sur" };
const diario = { dias: [1, 2, 3, 4, 5, 6, 7], horas: ["13:00"] };
const recepcion: Equipo = {
  ...base,
  id: "e-recepcion",
  nombre: "RECEPCION",
  rol: "agente",
  resumen: {
    admite: ["prueba_auto"],
    destinos: [enD, b2],
    repositorios: [
      { id: "documentos", nombre: "Documentos", destino: "almacen-0b5c1f8e", verificacion_auto: { cada_dias: 7, porcentaje: 10 }, prueba_auto: { cada_dias: 30 }, externa: { destino: "B2 de la oficina", destino_id: "destino-1a2b3c4d", hora: "23:00", bloqueo_dias: 30 } },
      { id: "importado", nombre: "Importado", destino: "almacen-0b5c1f8e", solo_lectura: true },
    ],
    copias: [
      { id: "docs", nombre: "Documentos cada día", repo: "documentos", horario: diario, ultima: { cuando: iso(23), estado: "ok" } },
      { id: "apagada", nombre: "Apagada", repo: "documentos", horario: diario, activa: false },
      { id: "vieja", nombre: "Del importado", repo: "importado", horario: diario },
    ],
  },
};
const repoInf = (extra: Partial<RepoInforme>): RepoInforme => ({
  id: "documentos",
  nombre: "Documentos",
  versiones: [],
  versiones_leidas: null,
  ejecuciones: [],
  espacio: null,
  verificacion: { ultima: iso(72), resultado: "ok", mensaje_corto: null },
  prueba_restauracion: { ultima: iso(24 * 10), resultado: "ok", mensaje_corto: null },
  externa: { ultima: iso(13), resultado: "ok", mensaje_corto: null },
  proteccion: null,
  ...extra,
});
const informe: Informe = { recibido: iso(1), datos: { repos: [repoInf({})] } };
const equipos = [almacen, recepcion];
const k = recepcion.resumen!.copias![0];

igual("horas entre dos copias de un horario diario", horasEntre(diario, AHORA), 24);
const rc = reglaDeCopia(recepcion, k, equipos, informe, [], AHORA)!;
igual(
  "pasos: la zona D, el espejo que la incluye (no el de otro repositorio) y la copia externa",
  rc.pasos.map((p) => [p.id, p.tipo, p.lugar, p.inmutable, p.soporte, p.equipo ?? null]),
  [
    ["destino", "copia", "oficina", "solo_anadir", `equipo:${almacen.id}:D:`, almacen.id],
    ["espejo-1", "espejo", "oficina", "no", `equipo:${almacen.id}:F:`, almacen.id],
    ["espejo-2", "espejo", "nube", "no", `nube:nube:${almacen.id}:dropbox-oficina`, null],
    ["externa", "externa", "nube", "object_lock", "nube:b2:copias-sur", null],
  ],
);
igual("el espejo de Dropbox lleva días fallando: atrasado", rc.regla.atrasados, ["espejo-2"]);
igual("cumple (la copia externa a B2 con bloqueo basta para fuera e inmutable)", [rc.regla.cumple, rc.regla.avisos], [true, ["mismo_equipo"]]);
igual("el sistema de archivos, solo como dato", rc.pasos.map((p) => p.sistemaArchivos), ["NTFS", "NTFS", null, null]);
igual("una copia apagada o de un repositorio importado no cuenta", [reglaDeCopia(recepcion, recepcion.resumen!.copias![1], equipos, informe, [], AHORA), reglaDeCopia(recepcion, recepcion.resumen!.copias![2], equipos, informe, [], AHORA)], [null, null]);

// La copia externa lleva 3 días fallando: dejó de cumplir (sin guardar historia).
const informeMal: Informe = { recibido: iso(1), datos: { repos: [repoInf({ externa: { ultima: iso(70), resultado: "fallo", mensaje_corto: "sin red" } })] } };
const mal = reglaDeCopia(recepcion, k, equipos, informeMal, [], AHORA)!;
igual("la copia externa falla: deja de cumplir «fuera»", [mal.regla.cumple, mal.regla.dejo_de_cumplir, mal.regla.partes.find((p) => p.id === "fuera")!.accion], [false, true, "poner_al_dia"]);
igual("qué hacer: decirlo con su nombre", queHacer(mal.regla.partes.find((p) => p.id === "fuera")!, mal, "c1", AHORA)?.texto, "«Dropbox Oficina» no está al día (la última vez falló o aún no se ha hecho) (y 1 paso más): revisa por qué.");
igual("en frase", fraseRegla(mal.regla), "Dejó de cumplir: 1 fuera de la oficina (algo no está al día).");

// Lo que dice la persona en el catálogo manda sobre lo deducido.
const catalogo: DestinoCatalogo[] = [
  { id: `zona:${almacen.id}:principal`, nombre: "", tipo: "zona", atributos: { inmutable: "instantaneas" } },
  { id: "destino-1a2b3c4d", nombre: "Nube Sur", tipo: "b2", donde: "copias-sur", atributos: { lugar: "otra_sede", inmutable: "no", soporte: "Cinta" } },
  { id: claveEspejoCarpeta(almacen.id, "F:\\Espejo"), nombre: "Disco del espejo", tipo: "local", atributos: { soporte: "cinta" } },
];
const marcada = reglaDeCopia(recepcion, k, equipos, informe, catalogo, AHORA)!;
igual(
  "marcados: inmutable de la zona, lugar y soporte de B2, soporte del espejo (sin distinguir mayúsculas)",
  marcada.pasos.map((p) => [p.id, p.lugar, p.inmutable, p.soporte, p.nombre]),
  [
    ["destino", "oficina", "instantaneas", `equipo:${almacen.id}:D:`, "Almacén ALMACEN-01 · Disco D"],
    ["espejo-1", "oficina", "no", "marcado:cinta", "Disco del espejo"],
    ["espejo-2", "nube", "no", `nube:nube:${almacen.id}:dropbox-oficina`, "Dropbox Oficina"],
    ["externa", "otra_sede", "no", "marcado:cinta", "Nube Sur"],
  ],
);
igual("lo deducido se recuerda (para «volver a lo deducido»)", marcada.pasos[0].porDefecto, { lugar: "oficina", inmutable: "solo_anadir" });
igual("el mismo soporte marcado cuenta una vez", marcada.regla.partes.find((p) => p.id === "soportes")!.valor, 3);
cierto("la clave de una carpeta del espejo no lleva la ruta", !claveEspejoCarpeta(almacen.id, "F:\\Espejo").includes("Espejo") && /^espejo:[0-9a-f-]+:[0-9a-f]{8}$/.test(claveEspejoCarpeta(almacen.id, "F:\\Espejo")));
igual("…y no distingue mayúsculas ni la barra final", claveEspejoCarpeta("a", "F:\\Espejo\\"), claveEspejoCarpeta("a", "f:\\espejo"));
igual("resumenCorto estable (FNV-1a)", [resumenCorto(""), resumenCorto("a")], ["811c9dc5", "e40c292c"]);

console.log("\n· Lo deducido de cada tipo de destino");
const solo = (d: DestinoResumen, extra: Partial<Equipo["resumen"]> = {}) => {
  const e: Equipo = { ...recepcion, resumen: { ...recepcion.resumen!, ...extra, destinos: [d], repositorios: [{ id: "r", nombre: "R", destino: d.id }], copias: [{ id: "k", nombre: "K", repo: "r", horario: diario, ultima: { cuando: iso(2), estado: "ok" } }] } };
  const p = reglaDeCopia(e, e.resumen!.copias![0], [almacen, e], null, [], AHORA)!.pasos[0];
  return [p.lugar, p.inmutable, p.soporte];
};
igual("carpeta del equipo: el mismo soporte que los originales", solo({ id: "d", nombre: "D", tipo: "local", unidad: "D:", extraible: false }), ["este_equipo", "no", "equipo:e-recepcion:origen"]);
igual("disco USB: otro soporte en el mismo equipo", solo({ id: "d", nombre: "USB", tipo: "local", unidad: "E:", extraible: true }), ["este_equipo", "no", "equipo:e-recepcion:usb:E:"]);
igual("carpeta de la red: otro equipo de la oficina", solo({ id: "d", nombre: "NAS", tipo: "local", red: true }), ["oficina", "no", "red:d"]);
igual("servidor de copias de fuera, sin comprobar", solo({ id: "d", nombre: "Sede", tipo: "rest", donde: "https://copias.ejemplo.com:8000/x/" }), ["otra_sede", "no", "servidor:copias.ejemplo.com:8000"]);
igual("…de solo añadir", solo({ id: "d", nombre: "Sede", tipo: "rest", donde: "https://copias.ejemplo.com:8000/x/", inmutable: true }), ["otra_sede", "solo_anadir", "servidor:copias.ejemplo.com:8000"]);
igual("S3 sin bloqueo", solo({ id: "d", nombre: "S3", tipo: "s3", donde: "cubo" }), ["nube", "no", "nube:s3:cubo"]);
igual("SFTP", solo({ id: "d", nombre: "SFTP", tipo: "sftp", donde: "copias@nas.ejemplo.com:/c" }), ["otra_sede", "no", "servidor:nas.ejemplo.com"]);
const propio: Equipo = { ...almacen, resumen: { ...almacen.resumen!, destinos: [{ ...enD, donde: "https://localhost:8000/almacen/" }], repositorios: [{ id: "r", nombre: "R", destino: enD.id }], copias: [{ id: "k", nombre: "K", repo: "r", horario: diario, ultima: { cuando: iso(2), estado: "ok" } }] } };
const rp = reglaDeCopia(propio, propio.resumen!.copias![0], [propio], null, [], AHORA)!;
igual("su propio almacén: en este mismo equipo, otro disco", [rp.pasos[0].lugar, rp.pasos[0].soporte], ["este_equipo", `equipo:${almacen.id}:D:`]);

console.log("\n· Prueba de restauración y verificación");
const sinPrueba = reglaDeCopia({ ...recepcion, resumen: { ...recepcion.resumen!, admite: [], repositorios: [{ ...recepcion.resumen!.repositorios![0], prueba_auto: null, verificacion_auto: null }] } }, k, equipos, { recibido: iso(1), datos: { repos: [repoInf({ prueba_restauracion: null, verificacion: null })] } }, [], AHORA)!;
const errores = sinPrueba.regla.partes.find((p) => p.id === "errores")!;
igual("sin verificación ni prueba: primero programar la verificación", [errores.valor, errores.accion], [2, "programar_verificacion"]);
igual("con un agente anterior: probarla a mano cada mes", queHacer({ ...errores, accion: "programar_prueba" }, sinPrueba, "c1", AHORA)?.texto.startsWith("Prueba la restauración cada mes"), true);
igual("con uno que la admite: programarla", queHacer({ ...errores, accion: "programar_prueba" }, rc, "c1", AHORA)?.enlace?.href, "/c/c1/equipos/e-recepcion/copias?prueba=documentos");

console.log("\n· Por cliente y entorno");
const todas = reglasDelCliente(equipos, { [recepcion.id]: informeMal }, [], AHORA);
igual("las copias que cuentan (activas, no importadas)", todas.map((x) => x.copia.id), ["docs"]);
igual("la cuenta", cuentaRegla(todas), { total: 1, cumplen: 0, dejaron: 1, faltan: { copias: 0, soportes: 0, fuera: 1, inmutable: 0, errores: 0 } });
igual("el entorno, en palabras", [textoEntorno(almacen), textoEntorno(recepcion), textoEntorno({ resumen: { entorno: { virtual: "hyperv" } } })], [
  "En un contenedor LXC dentro de una máquina virtual KVM (Proxmox, QEMU)",
  null,
  "En una máquina virtual Hyper-V",
]);

console.log(`\n${total - fallos}/${total} correctos`);
if (fallos) process.exit(1);
