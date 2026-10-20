// Pruebas del camino de cada copia (plan 0.8.0, C1; src/lib/camino.ts) y de lo
// que la página «Copias» cuenta con él (src/lib/copiasCliente.ts).
// `npm run test:vectores` (con las demás).
import type { DestinoResumen, Equipo, Informe, RepoInforme } from "../src/lib/tipos";
import { reglaDeCopia } from "../src/lib/regla321";
import { caminoDeCopia, caminoEnTexto, eslabonMal, lineaEslabon, resumenCamino, textoEslabon } from "../src/lib/camino";
import { cuentaCopias, filasCopias, filtrarCopias } from "../src/lib/copiasCliente";
import { saludCompleta } from "../src/lib/saludCompleta";

let fallos = 0;
let total = 0;
/** Los espacios finos de las fechas («hace 2 h»), como espacios normales. */
const sinFinos = (v: unknown) => JSON.stringify(v).replace(/[  ]/g, " ");
function igual(nombre: string, obtenido: unknown, esperado: unknown) {
  total++;
  const ok = sinFinos(obtenido) === sinFinos(esperado);
  if (!ok) fallos++;
  console.log(`${ok ? "ok  " : "MAL "} ${nombre}${ok ? "" : `\n       obtenido: ${JSON.stringify(obtenido)}\n       esperado: ${JSON.stringify(esperado)}`}`);
}
const cierto = (nombre: string, v: boolean) => igual(nombre, v, true);

const AHORA = Date.parse("2026-10-01T12:00:00-05:00");
const iso = (h: number) => new Date(AHORA - h * 3600_000).toISOString();
const base = { so: "Windows 11", version_agente: "0.7.26", box_pub: "", sign_pub: "", sal_equipo: "", etiqueta: null, modo: "gestionado" as const, confirmado: true, conectado: true, ultimo_contacto: iso(0), estado_servicio: "en_marcha" as const, siguiente_seq: 1 };

const almacen: Equipo = {
  ...base,
  id: "a1",
  nombre: "ALMACEN-01",
  rol: "almacenamiento",
  resumen: {
    admite: ["zonas_almacen", "espejo_flexible"],
    guarda_copias: {
      activo: true,
      puerto: 8000,
      carpeta: "D:\\Resguardo",
      usuarios: 1,
      repositorios: [{ usuario: "pc01", repos: ["docs"] }],
      nubes: [{ nombre: "Backblaze Oficina", tipo: "b2" }],
      espejo: {
        hora: "02:00",
        destinos: [
          // A la nube, solo «pc01/docs» (la selección de cada destino: como en el mapa).
          { tipo: "nube", nube: "Backblaze Oficina", carpeta: "Resguardo", ultima: iso(2), resultado: "Copiados 12 archivos.", repos: ["pc01/docs"], tras_copia: true },
          // A otra carpeta, solo otro repositorio: no es de esta copia.
          { tipo: "carpeta", carpeta: "E:\\Espejo", ultima: iso(3), resultado: "ok", repos: ["otro/caja"] },
        ],
      },
    },
  },
};
const enAlmacen: DestinoResumen = { id: "dest-alm", nombre: "ALMACEN-01", tipo: "rest", donde: "https://almacen.ejemplo:8000/pc01/", equipo_almacen: "a1" };
const disco: DestinoResumen = { id: "dest-usb", nombre: "Disco USB", tipo: "local", unidad: "F:", extraible: true };
const diario = { dias: [1, 2, 3, 4, 5, 6, 7], horas: ["10:00"] };
const pc: Equipo = {
  ...base,
  id: "e1",
  nombre: "PC-01",
  rol: "agente",
  resumen: {
    destinos: [enAlmacen, disco],
    repositorios: [
      { id: "docs", nombre: "Documentos", destino: "dest-alm", derivadas: [{ id: "d1", destino: "Disco USB", destino_id: "dest-usb", cuando: { tras_copia: true } }] },
      { id: "solo", nombre: "Solo", destino: "dest-alm" },
    ],
    copias: [
      { id: "k1", nombre: "Documentos", repo: "docs", horario: diario, ultima: { cuando: iso(2), estado: "ok" } },
      { id: "k2", nombre: "Sin pasos", repo: "solo", horario: diario, ultima: { cuando: iso(2), estado: "ok" } },
    ],
  },
};
const repoInf = (id: string, extra: Partial<RepoInforme> = {}): RepoInforme => ({
  id,
  nombre: id,
  versiones: [],
  versiones_leidas: null,
  ejecuciones: [],
  espacio: null,
  verificacion: null,
  prueba_restauracion: null,
  externa: null,
  proteccion: null,
  ...extra,
});
const informeBien: Informe = { recibido: iso(1), datos: { repos: [repoInf("docs", { derivadas: [{ id: "d1", ultima: iso(1), resultado: "ok", mensaje_corto: null }] }), repoInf("solo")] } };
const informeMal: Informe = { recibido: iso(1), datos: { repos: [repoInf("docs", { derivadas: [{ id: "d1", ultima: iso(3), resultado: "fallo", mensaje_corto: "sin espacio" }] }), repoInf("solo")] } };
const equipos = [almacen, pc];
const k1 = pc.resumen!.copias![0];
const opciones = (informe: Informe) => ({ cliente: "c1", ahora: AHORA, equipos, informe });

console.log("\n· El camino de una copia (lib/camino.ts)");
{
  const rc = reglaDeCopia(pc, k1, equipos, informeBien, [], AHORA);
  const c = caminoDeCopia(rc, opciones(informeBien));
  igual("los eslabones: el repositorio, el espejo del almacén que lo incluye y la copia derivada", c.map((x) => [x.clase, x.nombre]), [
    ["copia", "Almacén ALMACEN-01 · Disco D"],
    ["espejo", "Backblaze Oficina"],
    ["derivada", "Disco USB"],
  ]);
  igual("el espejo de otro repositorio no sale (la misma lógica que el mapa)", c.some((x) => x.nombre.includes("E:")), false);
  igual("estado de cada uno", c.map((x) => [x.tono, x.estado]), [
    ["ok", "Al día"],
    ["ok", "Al día"],
    ["ok", "Al día"],
  ]);
  igual("quién hace el espejo y que va después de cada copia", [c[1].quien, c[1].despues, c[2].despues], ["ALMACEN-01", true, true]);
  igual("tipo del destino (icono): Local + Solo añadir, Nube, Local", c.map((x) => [x.tipo.tipo, !!x.tipo.soloAnadir]), [
    ["local", true],
    ["nube", false],
    ["local", false],
  ]);
  igual("enlaces: la zona del almacén, la nube del almacén y (15g) la página de la derivada", c.map((x) => x.href), ["/c/c1/destinos/zona%3Aa1%3Aprincipal", "/c/c1/destinos/nube%3Aa1%3Abackblaze-oficina", "/c/c1/equipos/e1/repositorios/docs/derivadas/d1"]);
  igual("en una línea de «Copias»", lineaEslabon(c[1], AHORA), "→ Backblaze Oficina · al día, hace 2 h");
  cierto("el camino en texto (lectores de pantalla)", caminoEnTexto(c, AHORA).replace(/[  ]/g, " ").startsWith("Almacén ALMACEN-01 · Disco D (al día, hace 2 h) → después, Backblaze Oficina (espejo, al día"));
  igual("resumen: dos pasos, nada mal", resumenCamino(c), { pasos: 2, espejos: 1, derivadas: 1, mal: 0, tono: "ok" });
}
{
  const rc = reglaDeCopia(pc, k1, equipos, informeMal, [], AHORA);
  const c = caminoDeCopia(rc, opciones(informeMal));
  igual("la derivada falló: lo dice con cuándo", [c[2].tono, c[2].estado, textoEslabon(c[2], AHORA)], ["bad", "Falló", "falló hace 3 h"]);
  igual("y cuenta como paso mal", resumenCamino(c).mal, 1);
  const vivo = caminoDeCopia(rc, { ...opciones(informeMal), enVivo: (_e, _r, t) => (t === "copia_externa" ? "Subiendo 45 %" : null) });
  igual("subiendo: en marcha (no cuenta como mal)", [vivo[2].estado, vivo[2].vivo, eslabonMal(vivo[2]), lineaEslabon(vivo[2], AHORA)], ["Subiendo 45 %", true, false, "→ Disco USB · subiendo 45 %"]);
  const copiando = caminoDeCopia(rc, { ...opciones(informeMal), enVivo: (_e, _r, t) => (t === "copia" ? "Copiando 10 %" : null) });
  igual("la copia en marcha", [copiando[0].tono, copiando[0].estado], ["info", "Copiando 10 %"]);
}
{
  // El almacén no se conecta y su espejo va atrasado: «Sin conexión», no «Falló».
  const lejos: Equipo = { ...almacen, conectado: false, resumen: { ...almacen.resumen!, guarda_copias: { ...almacen.resumen!.guarda_copias!, espejo: { hora: "02:00", destinos: [{ tipo: "nube", nube: "Backblaze Oficina", carpeta: "Resguardo", ultima: iso(80), resultado: "ok", repos: ["pc01"] }] } } } };
  const eqs = [lejos, pc];
  const c = caminoDeCopia(reglaDeCopia(pc, k1, eqs, informeBien, [], AHORA), { ...opciones(informeBien), equipos: eqs });
  igual("el almacén sin conexión: su espejo, «Sin conexión»", [c[1].tono, c[1].estado, textoEslabon(c[1], AHORA)], ["warn", "Sin conexión", "sin conexión · la última, hace 3 días"]);
  const conectado = [{ ...lejos, conectado: true }, pc];
  const c2 = caminoDeCopia(reglaDeCopia(pc, k1, conectado, informeBien, [], AHORA), { ...opciones(informeBien), equipos: conectado });
  igual("conectado pero sin subir desde hace 80 h: «Atrasado»", [c2[1].tono, c2[1].estado], ["warn", "Atrasado"]);
}
igual("una copia sin regla (desactivada): sin camino", caminoDeCopia(null, opciones(informeBien)), []);

console.log("\n· La página «Copias» con el camino (lib/copiasCliente.ts)");
{
  const filas = filasCopias(equipos, { e1: informeMal, a1: null }, AHORA, []);
  const f1 = filas.find((f) => f.copia.id === "k1")!;
  igual("cada fila trae su camino (sin el repositorio: los pasos)", f1.pasos.map((x) => x.nombre), ["Backblaze Oficina", "Disco USB"]);
  igual("la copia está al día pero un paso falla: pide atención", [f1.estado.tono, f1.atencion], ["ok", true]);
  igual("«Necesitan atención» la cuenta", filtrarCopias(filas, { estado: "atencion" }).map((f) => f.copia.id), ["k1"]);
  igual("«Al día» no (no está del todo al día)", filtrarCopias(filas, { estado: "al_dia" }).map((f) => f.copia.id), ["k2"]);
  igual("la selección por equipo (`pc01`) incluye todos sus repositorios, como en el mapa", caminoDeCopia(reglaDeCopia(pc, pc.resumen!.copias![1], [{ ...almacen, resumen: { ...almacen.resumen!, guarda_copias: { ...almacen.resumen!.guarda_copias!, espejo: { hora: "02:00", destinos: [{ tipo: "nube", nube: "Backblaze Oficina", ultima: iso(2), resultado: "ok", repos: ["pc01"] }] } } } }, pc], informeBien, [], AHORA), opciones(informeBien)).map((x) => x.clase), ["copia", "espejo"]);
  igual("filtro: con espejo", filtrarCopias(filas, { espejo: "con" }).map((f) => f.copia.id), ["k1"]);
  igual("filtro: sin espejo", filtrarCopias(filas, { espejo: "sin" }).map((f) => f.copia.id), ["k2"]);
  igual("la cifra: «N al día · M con un paso fallando»", cuentaCopias(filas), { total: 2, alDia: 1, conPasoMal: 1, atencion: 1 });
  const bien = filasCopias(equipos, { e1: informeBien, a1: null }, AHORA, []);
  igual("con todo bien: las dos al día", cuentaCopias(bien), { total: 2, alDia: 2, conPasoMal: 0, atencion: 0 });
}

console.log("\n· 14w: la salud de la protección, completada por la consola (lib/saludCompleta.ts)");
{
  const delAgente = repoInf("docs", {
    proteccion: {
      puntuacion: 5,
      total: 7,
      items: [
        { id: "copias", estado: "ok", etiqueta: "Copias automáticas", detalle: "" },
        { id: "externa", estado: "fallo", etiqueta: "Copia externa", detalle: "Todas las versiones están en un solo sitio." },
        { id: "retencion", estado: "aviso", etiqueta: "Retención", detalle: "Sin política: el repositorio crece sin fin." },
      ],
    },
  });
  const rc = reglaDeCopia(pc, k1, equipos, informeBien, [], AHORA);
  const p = saludCompleta(delAgente, { rc, retencionAlmacen: "ALMACEN-01", ahora: AHORA })!;
  const item = (id: string) => p.items.find((i) => i.id === id)!;
  igual("el espejo del almacén a la nube cuenta como fuera del sitio", [item("externa").estado, item("externa").etiqueta], ["ok", "Fuera del sitio"]);
  cierto("…y lo dice con su nombre", item("externa").detalle.includes("«Backblaze Oficina»"));
  igual("la retención la aplica el almacén", [item("retencion").estado, item("retencion").detalle], ["ok", "La aplica el almacén «ALMACEN-01»."]);
  igual("la cuenta, rehecha", [p.puntuacion, p.total, p.pendientes, p.tono], [3, 3, 0, "ok"]);
  const sinRegla = saludCompleta(delAgente, { ahora: AHORA })!;
  igual("sin copia activa: lo del agente, con el nombre nuevo", [sinRegla.items[1].estado, sinRegla.items[1].etiqueta], ["fallo", "Fuera del sitio"]);
}

console.log(`\n${total - fallos} de ${total} comprobaciones correctas.`);
if (fallos) process.exit(1);
