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
  estadoParte,
  globoParte,
  lineaFalta,
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
import { atributosDe, clasificar, estadoConexion, textoClasificacion, tipoDeLugar, ultimaConexion } from "../src/lib/tipoDestino";

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
igual("en frase", fraseRegla(mal.regla), "Dejó de cumplir: 1 fuera del sitio (algo no está al día).");

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
igual("lo deducido se recuerda (para «volver a lo deducido»)", marcada.pasos[0].porDefecto, { lugar: "oficina", inmutable: "solo_anadir", bloqueoDias: null });
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

console.log("\n· La tira: estado corto de cada parte y la línea de lo que falta");
{
  const parte = (id: "copias" | "errores", valor: number, meta: number, cumple: boolean, cumple_config: boolean) => ({ id, meta, valor, valor_config: valor, cumple, cumple_config, accion: "", detalle: "" });
  igual("cumple", estadoParte(parte("copias", 3, 3, true, true)), { tono: "ok", icono: "cumple", texto: "Cumple" });
  igual("la configuración cumple, algo no está al día", estadoParte(parte("copias", 2, 3, false, true)), { tono: "warn", icono: "atrasado", texto: "Atrasado" });
  igual("falta uno / faltan dos", [estadoParte(parte("copias", 2, 3, false, false)).texto, estadoParte(parte("copias", 1, 3, false, false)).texto], ["Falta 1", "Faltan 2"]);
  igual("en el «0», lo que hay por resolver", estadoParte(parte("errores", 2, 0, false, false)).texto, "Faltan 2");
  igual("la línea corta: lo atrasado (el espejo en la nube lleva 70 h fallando)", lineaFalta(mal.regla), "No está al día: 1 fuera del sitio.");
  igual(
    "la línea corta: lo que falta",
    lineaFalta({ ...mal.regla, partes: mal.regla.partes.map((p) => (p.id === "inmutable" ? { ...p, cumple: false, cumple_config: false } : p)) }),
    "Falta: 1 inmutable o aislado. No está al día: 1 fuera del sitio.",
  );
  igual("si cumple, nada", lineaFalta({ ...mal.regla, cumple: true }), null);
  const pm = mal.regla.partes.find((p) => !p.cumple)!;
  cierto("el globo de una parte dice qué pide, cómo está y qué hacer", globoParte(pm, mal, "c1", AHORA).startsWith("1 fuera del sitio: ") && globoParte(pm, mal, "c1", AHORA).includes("Qué hacer"));
}

console.log("\n· 0.7.26: tipo y marcas de un destino (lib/tipoDestino.ts)");
{
  const zona = { lugar: "oficina", inmutable: "solo_anadir" } as const;
  const b2l = { lugar: "nube", inmutable: "object_lock", bloqueoDias: 30 } as const;
  igual("lugares de antes → tipo", (["este_equipo", "oficina", "otra_sede", "nube"] as const).map(tipoDeLugar), ["local", "local", "fuera", "nube"]);
  igual("zona de un almacén: Local + Inmutable (deducido)", clasificar(zona, null), {
    tipo: "local",
    inmutable: true,
    como: "solo_anadir",
    aislado: false,
    bloqueoDias: null,
    aisladoDias: 30,
    tipoPorPersona: false,
    marcasPorPersona: false,
  });
  igual("B2 con bloqueo de la copia externa: Nube + Inmutable 30 días", [clasificar(b2l, null).tipo, clasificar(b2l, null).inmutable, clasificar(b2l, null).bloqueoDias], ["nube", true, 30]);
  igual("«desconectado» de antes se lee como Aislado (no Inmutable)", [clasificar(zona, { inmutable: "desconectado" }).aislado, clasificar(zona, { inmutable: "desconectado" }).inmutable], [true, false]);
  igual("el lugar de antes marcado por una persona da el tipo", [clasificar(zona, { lugar: "otra_sede" }).tipo, clasificar(zona, { lugar: "otra_sede" }).tipoPorPersona], ["fuera", true]);
  igual("el tipo nuevo manda sobre el lugar", clasificar(zona, { lugar: "otra_sede", tipo: "nube" }).tipo, "nube");
  const base2 = { tipo: "local" as const, inmutable: true, como: "solo_anadir" as const, aislado: false, bloqueoDias: null, aisladoDias: 30, soporte: "" };
  igual("sin cambios: nada que guardar", atributosDe(zona, base2), {});
  igual("Aislado sin Inmutable: también «desconectado» para una consola anterior", atributosDe(zona, { ...base2, inmutable: false, aislado: true }), { inmutable: "desconectado", aislado: true });
  igual("Aislado e Inmutable a la vez", atributosDe(zona, { ...base2, aislado: true, aisladoDias: 14 }), { aislado: true, aislado_dias: 14 });
  igual("Fuera del sitio: también el lugar de antes", atributosDe(zona, { ...base2, tipo: "fuera" }), { tipo: "fuera", lugar: "otra_sede" });
  igual("Local desde una nube: «oficina» para una consola anterior", atributosDe(b2l, { ...base2, tipo: "local", como: "object_lock", bloqueoDias: 30 }), { tipo: "local", lugar: "oficina" });
  igual("otro bloqueo de días", atributosDe(b2l, { ...base2, tipo: "nube", como: "object_lock", bloqueoDias: 90 }), { bloqueo_dias: 90 });
  for (const [n, e] of [
    ["ida y vuelta: Aislado", { ...base2, inmutable: false, aislado: true }],
    ["ida y vuelta: Nube inmutable y aislada", { ...base2, tipo: "nube" as const, aislado: true, aisladoDias: 10 }],
    ["ida y vuelta: Fuera sin marcas", { ...base2, tipo: "fuera" as const, inmutable: false }],
  ] as const) {
    const c = clasificar(zona, atributosDe(zona, e));
    igual(n, [c.tipo, c.inmutable, c.aislado, c.aisladoDias], [e.tipo, e.inmutable, e.aislado, e.aisladoDias]);
  }
  igual("el nombre accesible", textoClasificacion({ tipo: "nube", inmutable: true, aislado: false, bloqueoDias: 30 }), "Nube, inmutable (bloqueo de 30 días)");

  console.log("\n· 0.7.26: la conexión de un medio aislado (la ve el agente)");
  igual("un agente anterior: sin datos (no es un fallo)", estadoConexion(null, 30, AHORA), { sinDatos: true, tarde: false, texto: "Sin datos de conexión", discos: [] });
  igual("visto hace 3 días", estadoConexion({ ultima_conexion: iso(72), volumenes: [{ id: "a1", visto: iso(72) }] }, 30, AHORA).texto, "Conectado por última vez hace 3 días");
  const rot = estadoConexion({ ultima_conexion: iso(24 * 40), volumenes: [{ id: "a1", visto: iso(24 * 47) }, { id: "b2", visto: iso(24 * 40) }] }, 30, AHORA);
  igual("40 días sin conectarse: pide conectarlo", [rot.tarde, rot.texto], [true, "Conecta el medio aislado para comprobar la rotación (última vez visto hace 40 días)"]);
  igual("con dos discos, uno por disco (el más reciente primero)", rot.discos.map((d) => d.texto), ["Disco 1: visto hace 40 días", "Disco 2: visto hace 47 días"]);
  igual("la última conexión, de la lista si falta", ultimaConexion({ volumenes: [{ id: "a", visto: iso(5) }, { id: "b", visto: iso(2) }] }), iso(2));

  // Un USB del equipo marcado Aislado: la regla usa la conexión que vio el agente.
  const usbD: DestinoResumen = { id: "usb-1", nombre: "USB", tipo: "local", unidad: "E:", extraible: true, aislado: { ultima_conexion: iso(24 * 40), volumenes: [{ id: "a1", visto: iso(24 * 40) }] } };
  const conUsb: Equipo = { ...recepcion, resumen: { ...recepcion.resumen!, destinos: [usbD], repositorios: [{ id: "r", nombre: "R", destino: "usb-1" }], copias: [{ id: "k", nombre: "K", repo: "r", horario: diario, ultima: { cuando: iso(24 * 40), estado: "ok" } }] } };
  const cat: DestinoCatalogo[] = [{ id: "usb-1", nombre: "", tipo: "local", atributos: { inmutable: "desconectado", aislado: true } }];
  const ru = reglaDeCopia(conUsb, conUsb.resumen!.copias![0], [conUsb], null, cat, AHORA)!;
  const p0 = ru.pasos[0];
  igual("el paso: Local, Aislado, conectado hace 40 días", [p0.tipo_destino, p0.aislado, p0.conectado, p0.clasificacion.marcasPorPersona], ["local", true, iso(24 * 40), true]);
  igual("avisa: lleva más de 30 días sin conectarse", ru.regla.avisos.includes("aislado_sin_conectar"), true);
  const ruSin = reglaDeCopia({ ...conUsb, resumen: { ...conUsb.resumen!, destinos: [{ ...usbD, aislado: undefined }] } }, conUsb.resumen!.copias![0], [conUsb], null, cat, AHORA)!;
  igual("sin datos de conexión (agente anterior): no avisa", ruSin.regla.avisos.includes("aislado_sin_conectar"), false);
}

console.log(`\n${total - fallos}/${total} correctos`);
if (fallos) process.exit(1);
