// Pruebas de los espejos de un repositorio (15m; src/lib/espejosRepo.ts): los que lo
// incluyen, los que lo incluyeron (su copia allí ya no se actualiza) y lo que se dice al
// cambiar un espejo. `npm run test:vectores` (con las demás).
import type { DestinoResumen, Equipo, TrabajoEspejoResumen } from "../src/lib/tipos";
import { espejosDelRepo, salenDelEspejo, sinActualizar, textoSalen, textoSinActualizar } from "../src/lib/espejosRepo";

let fallos = 0;
let total = 0;
function igual(nombre: string, obtenido: unknown, esperado: unknown) {
  total++;
  const ok = JSON.stringify(obtenido) === JSON.stringify(esperado);
  if (!ok) fallos++;
  console.log(`${ok ? "ok  " : "MAL "} ${nombre}${ok ? "" : `\n       obtenido: ${JSON.stringify(obtenido)}\n       esperado: ${JSON.stringify(esperado)}`}`);
}
const cierto = (nombre: string, v: boolean) => igual(nombre, v, true);

const AHORA = Date.parse("2026-10-20T12:00:00-05:00");
const iso = (dias: number) => new Date(AHORA - dias * 86_400_000).toISOString();
const base = { so: "Windows 11", version_agente: "0.8.1", box_pub: "", sign_pub: "", sal_equipo: "", etiqueta: null, modo: "gestionado" as const, confirmado: true, conectado: true, ultimo_contacto: iso(0), estado_servicio: "en_marcha" as const, siguiente_seq: 1 };
const trabajo = (id: string, repos: string[], extra: Partial<TrabajoEspejoResumen> = {}) =>
  ({
    id,
    nombre: `Espejo ${id}`,
    activo: true,
    quien: "almacen",
    que: { tipo: "repos", repos },
    adonde: { tipo: "nube", nube: "Nube Oficina", carpeta: "Resguardo" },
    cuando: { tras_copia: true },
    retencion: { modo: "nunca" },
    freno: { pct: 10, min_archivos: 100, min_faltan: 20, accion: "avisar" },
    orden: 0,
    ...extra,
  }) as TrabajoEspejoResumen;

const almacen: Equipo = {
  ...base,
  id: "a1",
  nombre: "ALMACEN-01",
  rol: "almacenamiento",
  resumen: {
    admite: ["espejo_trabajos"],
    guarda_copias: {
      activo: true,
      puerto: 8000,
      carpeta: "D:\\Resguardo",
      usuarios: 1,
      repositorios: [{ usuario: "pc01", repos: ["docs", "fotos"] }],
      espejo: {
        trabajos: [
          // Incluye «docs»; «fotos» estuvo hasta hace 12 días.
          trabajo("t1", ["pc01/docs"], { ultima_por_repo: { "pc01/docs": iso(0), "pc01/fotos": iso(12) } }),
          // Uno de un agente anterior, sin `ultima_por_repo`: lo de siempre (su última vez bien).
          trabajo("t2", ["pc01/docs"], { orden: 1, bien: iso(1) }),
          // Uno que no tiene que ver.
          trabajo("t3", ["otro/caja"], { orden: 2 }),
        ],
      },
    },
  },
};
const destino: DestinoResumen = { id: "dest-alm", nombre: "ALMACEN-01", tipo: "rest", donde: "https://almacen.ejemplo:8000/pc01/", equipo_almacen: "a1" };
const pc: Equipo = {
  ...base,
  id: "e1",
  nombre: "PC-01",
  resumen: {
    destinos: [destino],
    repositorios: [
      { id: "docs", nombre: "Documentos", destino: "dest-alm" },
      { id: "fotos", nombre: "Fotos", destino: "dest-alm" },
    ],
  },
};
const equipos = [almacen, pc];
const [docs, fotos] = pc.resumen!.repositorios!;

console.log("\n· Los espejos de un repositorio");
{
  const l = espejosDelRepo(pc, docs, equipos);
  igual("los que incluyen «docs» (y nada del que no tiene que ver)", l.map((x) => [x.trabajo.id, x.incluye, x.ultima]), [
    ["t1", true, iso(0)],
    ["t2", true, iso(1)],
  ]);
  igual("ninguno sin actualizar", sinActualizar(l).length, 0);
}
{
  const l = espejosDelRepo(pc, fotos, equipos);
  igual("«fotos» ya no está en t1, pero lo estuvo", l.map((x) => [x.trabajo.id, x.incluye, x.ultima]), [["t1", false, iso(12)]]);
  const s = sinActualizar(l);
  igual("su copia allí no se actualiza", s.length, 1);
  cierto("lo dice con los días y el espejo", textoSinActualizar(s[0], AHORA).startsWith("Tiene una copia en «Nube Oficina» que ya no se actualiza desde hace 12 días (ya no entra en el espejo «Espejo t1»)"));
}

console.log("\n· Al cambiar un espejo");
{
  const antes = trabajo("t1", ["pc01/docs", "pc01/fotos"]);
  const despues = trabajo("t1", ["pc01/docs"]);
  igual("sale «fotos»", salenDelEspejo(antes, despues, ["pc01/docs", "pc01/fotos"]), ["pc01/fotos"]);
  igual("de todos a algunos", salenDelEspejo({ ...antes, que: { tipo: "todos" } }, despues, ["pc01/docs", "pc01/fotos", "otro/caja"]), ["pc01/fotos", "otro/caja"]);
  igual("añadir no saca a nadie", salenDelEspejo(despues, antes, ["pc01/docs", "pc01/fotos"]), []);
  igual("de otra zona: salen todos", salenDelEspejo(antes, { ...despues, zona: "z2" }, ["pc01/docs", "pc01/fotos"]), ["pc01/docs", "pc01/fotos"]);
  cierto("la frase dice que lo subido se queda", textoSalen(["Fotos", "Caja"], "Nube Oficina").startsWith("«Fotos» y «Caja» dejan de estar en este espejo. Lo ya subido a Nube Oficina se queda allí"));
  igual("sin nadie, nada", textoSalen([], "x"), "");
}

console.log(`\n${total - fallos} de ${total} comprobaciones correctas.`);
if (fallos) process.exit(1);
