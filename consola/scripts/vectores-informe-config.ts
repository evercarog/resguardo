// Pruebas del informe de la configuración de copias (0.8.1, H5) y de lo que comparte
// con el paquete para auditoría (H2): el PDF de texto (lib/pdfTexto.ts), el CSV
// (lib/csv.ts) y el informe (lib/informeConfig.ts). `npm run test:vectores`.
import { anchoTexto, byteWin, maquetar, partir, pdfDe } from "../src/lib/pdfTexto";
import { celdaCsv, csv, nombreArchivo } from "../src/lib/csv";
import { copiasInforme, csvConfig, documentoConfig, separarGrupos } from "../src/lib/informeConfig";
import { aB64url, deB64url, esIp, soporte } from "../src/lib/passkeys";
import { csvInventario, errorFicha, leerInventario, valorFicha } from "../src/lib/inventario";
import { csvPaquete, documentoPaquete, filasInventario, incidentes } from "../src/lib/paqueteAuditoria";
import type { Configuracion, EntradaHistorial, Equipo } from "../src/lib/tipos";

let fallos = 0;
let total = 0;
function igual(nombre: string, obtenido: unknown, esperado: unknown) {
  total++;
  const ok = JSON.stringify(obtenido) === JSON.stringify(esperado);
  if (!ok) fallos++;
  console.log(`${ok ? "ok  " : "MAL "} ${nombre}${ok ? "" : `\n       obtenido: ${JSON.stringify(obtenido)}\n       esperado: ${JSON.stringify(esperado)}`}`);
}
const cierto = (nombre: string, x: boolean) => igual(nombre, x, true);

console.log("\n· PDF de texto");
{
  igual("WinAnsi: tildes, ñ, «», €, raya y lo que no cabe", ["á", "ñ", "«", "€", "—", "→", "漢"].map(byteWin), [0xe1, 0xf1, 0xab, 0x80, 0x97, 0x3e, 0x3f]);
  cierto("ancho con negrita, mayor", anchoTexto("Hola", 10, true) > anchoTexto("Hola", 10));
  const ls = partir("una frase bastante larga que no cabe en tan poco espacio", 80, 10);
  cierto("parte en líneas que caben", ls.length > 2 && ls.every((l) => anchoTexto(l, 10) <= 80));
  igual("respeta los saltos de línea", partir("a\nb", 500, 10), ["a", "b"]);
  cierto("una palabra larguísima se parte por letras", partir("x".repeat(400), 100, 10).every((l) => anchoTexto(l, 10) <= 100));
  const muchos = Array.from({ length: 200 }, (_, i) => ({ t: "p" as const, texto: `Párrafo ${i} con (paréntesis) y \\barras\\` }));
  cierto("varias páginas", maquetar(muchos).length > 3);
  const pdf = pdfDe({ titulo: "Prueba «ñ»", pie: "Pie", bloques: [{ t: "titulo", texto: "Título" }, ...muchos, { t: "kv", filas: [["Clave", "Valor\ncon dos líneas"]] }] });
  const texto = Array.from(pdf, (b) => String.fromCharCode(b)).join("");
  cierto("empieza como un PDF", texto.startsWith("%PDF-1.4\n%"));
  cierto("termina con %%EOF", texto.trimEnd().endsWith("%%EOF"));
  // Cada entrada de la tabla xref apunta al principio de su objeto.
  const xref = Number(/startxref\n(\d+)/.exec(texto)?.[1]);
  const entradas = texto.slice(xref).split("\n").slice(3).filter((l) => / n $/.test(l));
  cierto("xref con todos los objetos", entradas.length >= 7);
  cierto(
    "xref apunta a cada objeto",
    entradas.every((l, i) => texto.startsWith(`${i + 1} 0 obj`, Number(l.slice(0, 10)))),
  );
  // Las longitudes de los flujos son las de verdad.
  const flujos = [...texto.matchAll(/<< \/Length (\d+) >>\nstream\n/g)];
  cierto("longitud de cada flujo", flujos.length > 0 && flujos.every((m) => texto.slice(m.index! + m[0].length + Number(m[1]), m.index! + m[0].length + Number(m[1]) + 9) === "endstream"));
  cierto("los paréntesis y las barras, escapados", texto.includes("\\(paréntesis\\)".replace("é", "\\351")) && texto.includes("\\\\barras\\\\"));
  cierto("«Página 1 de N» en el pie", /Página 1 de \d+/.test(texto.replace(/\\341/g, "á")));
}

console.log("\n· CSV");
{
  igual("celdas", [celdaCsv("a;b"), celdaCsv('di "hola"'), celdaCsv(3), celdaCsv(null), celdaCsv(true), celdaCsv("dos\nlíneas")], ['"a;b"', '"di ""hola"""', "3", "", "sí", '"dos\nlíneas"']);
  igual("nada de fórmulas", [celdaCsv("=HYPERLINK(1)"), celdaCsv("+1"), celdaCsv("-2"), celdaCsv("@x"), celdaCsv(-2)], ["'=HYPERLINK(1)", "'+1", "'-2", "'@x", "-2"]);
  const c = csv(["A", "B"], [["1", "2"]]);
  igual("con BOM, «;» y CRLF", [c.charCodeAt(0), c.slice(1)], [0xfeff, "A;B\r\n1;2\r\n"]);
  igual("nombre de archivo limpio", nombreArchivo(["Resguardo", "Oficina: Sur/Norte", "2026-10-20"], "csv"), "Resguardo - Oficina Sur Norte - 2026-10-20.csv");
}

console.log("\n· Grupos de exclusiones reconocidos");
{
  const g = [{ nombre: "Música", reglas: ["*.mp3", "*.flac"] }, { nombre: "Vídeo", reglas: ["*.mp4", "*.mkv"] }];
  igual("un grupo entero y reglas sueltas", separarGrupos(["*.mp3", "*.flac", "*.mp4", "*.tmp"], g), { grupos: ["Música"], sueltas: ["*.mp4", "*.tmp"] });
}

console.log("\n· El informe de un equipo de ejemplo");
{
  const eq = {
    id: "eq-1",
    nombre: "PC-EJEMPLO",
    so: "Windows 11",
    version_agente: "0.8.1",
    box_pub: "",
    sign_pub: "",
    sal_equipo: "",
    etiqueta: null,
    rol: "agente",
    modo: "gestionado",
    confirmado: true,
    conectado: true,
    ultimo_contacto: null,
    estado_servicio: "en_marcha",
    siguiente_seq: 1,
    resumen: {
      copias: [{ id: "k1", nombre: "Documentos", repo: "r1", horario: { dias: [1, 2, 3, 4, 5], horas: ["13:00"] }, carpetas: 2, activa: true }],
      repositorios: [{ id: "r1", nombre: "Documentos", destino: "d1", retencion_regla: { diarias: 7, semanales: 4, mensuales: 12, anuales: 2 }, verificacion_auto: { cada_dias: 7, porcentaje: 10 } }],
      destinos: [{ id: "d1", nombre: "Disco D", tipo: "local", carpeta: "D:\\Copias" }],
    },
  } as unknown as Equipo;
  const base = { equipos: [eq], todos: [eq], informes: {}, catalogo: [], organizacion: null, funcionales: [], infra: null, grupos: [{ nombre: "Música", reglas: ["*.mp3"] }], ahora: Date.parse("2026-10-20T10:00:00Z") };
  const sin = copiasInforme({ ...base, configs: {} });
  igual("una copia", sin.length, 1);
  igual("sin la clave: cuántas carpetas, no cuáles", [sin[0].carpetas, sin[0].nCarpetas, sin[0].exclusiones], [null, 2, null]);
  cierto("el horario en palabras", /13:00/.test(sin[0].cuando));
  cierto("la retención del equipo en palabras", sin[0].sitios[0].retencion.startsWith("En el equipo: Conserva"));
  const cfg: Configuracion = {
    v: 1,
    copias: [{ id: "k1", nombre: "Documentos", repo: "r1", carpetas: ["C:\\Datos\\Ventas", "C:\\Datos\\Compras"], exclusiones: ["*.mp3", "*.tmp"], horario: { dias: [1], horas: ["13:00"] }, activa: true, gancho: { tipo: "sqlserver", bases: [], carpeta: "C:\\Volcados", todas: true } }],
    repositorios: [],
    destinos: [],
  };
  const con = copiasInforme({ ...base, configs: { "eq-1": cfg } });
  igual("con la clave: las carpetas y las exclusiones", [con[0].carpetas, con[0].grupos, con[0].exclusiones], [["C:\\Datos\\Ventas", "C:\\Datos\\Compras"], ["Música"], ["*.tmp"]]);
  cierto("antes de copiar: el volcado", (con[0].antes ?? []).some((t) => /SQL Server/.test(t)));
  const doc = documentoConfig(con, { cliente: "Ejemplo", generado: "Hoy", alcance: "Todos los equipos", conClave: true });
  cierto("el PDF dice el equipo y la copia", doc.bloques.some((b) => b.t === "h1" && b.texto === "PC-EJEMPLO") && doc.bloques.some((b) => b.t === "h2" && b.texto.includes("Documentos")));
  const lineas = csvConfig(con).trim().split("\r\n");
  igual("CSV: cabecera y una fila por sitio", lineas.length, 1 + con[0].sitios.length);
  cierto("CSV: nada que parezca una contraseña", !/contrase|clave|secret/i.test(csvConfig(con).split("\r\n").slice(1).join("")));
}

console.log("\n· Inventario y paquete para auditoría (H2)");
{
  igual("ficha vacía: se quita", valorFicha({ responsable: " ", criticidad: null }), null);
  igual("ficha limpia", valorFicha({ responsable: " Ana ", criticidad: "alta", datos_personales: false, rto_min: 240 }), { responsable: "Ana", criticidad: "alta", datos_personales: false, rto_min: 240 });
  cierto("el identificador externo es de un equipo", errorFicha({ id_externo: "INV-1" }, "repositorio") !== null && errorFicha({ id_externo: "INV-1" }, "equipo") === null);
  cierto("RTO mayor que cero", errorFicha({ rto_min: 0 }, "equipo") !== null);
  igual("del servidor, solo lo que vale", leerInventario({ "equipo:e1": { responsable: "Ana", criticidad: "maxima", datos_personales: true }, otra: {} }), { "equipo:e1": { responsable: "Ana", datos_personales: true } });
  const eq = {
    id: "e1",
    nombre: "SERVIDOR-EJ",
    so: "Debian 12",
    confirmado: true,
    modo: "gestionado",
    resumen: {
      copias: [{ id: "k1", nombre: "Bases", repo: "r1" }],
      repositorios: [{ id: "r1", nombre: "Bases", destino: "d1", versiones: 30, bytes: 1000 }],
    },
  } as unknown as Equipo;
  const org = { sedes: [], equipos: {}, destinos: {}, categorias: [], orden: [], exclusiones: [], exclusiones_quitadas: [], exclusionesAdmitidas: true, inventario: { "equipo:e1": { id_externo: "INV-7", datos_personales: true }, "repositorio:e1/r1": { criticidad: "critica" as const } } };
  const fs = filasInventario([eq], org);
  igual("filas del inventario", fs.map((f) => [f.tipo, f.idExterno, f.ficha.criticidad ?? null]), [["equipo", "INV-7", null], ["repositorio", "INV-7", "critica"]]);
  const lineasInv = csvInventario(fs).trim().split("\r\n");
  cierto("CSV del inventario: cabecera y dos filas", lineasInv.length === 3 && lineasInv[0].includes("Identificador externo"));
  const ahora = Date.parse("2026-10-20T10:00:00Z");
  const h = (hora: string, resultado: "ok" | "fallo", tipo: "copia" | "verificacion" = "copia") => ({ id: hora + tipo, hora, tipo, copia: "k1", repo: "r1", resultado, mensaje: resultado === "fallo" ? "Sin conexión con el almacén" : "" }) as EntradaHistorial;
  const d = {
    cliente: "Cliente de ejemplo",
    desde: new Date("2026-10-01T00:00:00Z"),
    hasta: new Date("2026-10-21T00:00:00Z"),
    generado: "Hoy",
    equipos: [eq],
    organizacion: org,
    copias: [],
    conClave: false,
    historial: { e1: [h("2026-10-02T10:00:00Z", "fallo"), h("2026-10-03T10:00:00Z", "ok"), h("2026-10-05T10:00:00Z", "fallo"), h("2026-10-04T03:00:00Z", "ok", "verificacion")] },
    restauraciones: [],
    funcionales: [],
    politica: null,
    miembros: [{ cuenta: "c1", correo: "ana@ejemplo.com", nombre: "Ana", rol: "administrador" as const }],
    nombreRol: (r: string) => (r === "administrador" ? "Administrador" : r),
    ahora,
  };
  const inc = incidentes(d);
  igual("incidentes: uno resuelto y otro no", inc.map((i) => !!i.resuelta), [false, true]);
  const pdf = documentoPaquete(d);
  const textos = pdf.bloques.map((b) => ("texto" in b ? b.texto : "")).join("\n");
  cierto("dice que es evidencia, no certificación", textos.includes("No es una certificación"));
  cierto("cita los controles", ["A.5.9", "A.8.13", "A.5.30", "A.8.14", "CIS Controls v8"].every((c) => textos.includes(c)));
  cierto("sin el correo de las personas", !JSON.stringify(pdf).includes("ana@ejemplo.com") && !csvPaquete(d).includes("ana@ejemplo.com"));
  cierto("sin la ley de ningún país", !/RGPD|GDPR|LOPD|LGPD|Ley \d/.test(JSON.stringify(pdf)));
  cierto("el CSV tiene la parte y sus controles", csvPaquete(d).includes("Incidentes: copias fallidas y cómo se resolvieron;A.8.13 CIS 11"));
}

console.log("\n· Llaves de acceso (H3): base64url y cuándo se pueden usar");
{
  igual("base64url ida y vuelta", aB64url(deB64url("AQID_-8")), "AQID_-8");
  igual("IP o nombre", [esIp("192.0.2.10"), esIp("[2001:db8::1]"), esIp("consola.ejemplo.com"), esIp("localhost")], [true, true, false, false]);
  igual("por IP, no", soporte({ hostname: "192.0.2.10", seguro: true, api: true }).ok, false);
  igual("sin conexión segura, no", soporte({ hostname: "consola.ejemplo.com", seguro: false, api: true }).ok, false);
  igual("por nombre y seguro, sí", soporte({ hostname: "consola.ejemplo.com", seguro: true, api: true }).ok, true);
}

console.log(`\n${total - fallos}/${total} bien`);
if (fallos) process.exit(1);
