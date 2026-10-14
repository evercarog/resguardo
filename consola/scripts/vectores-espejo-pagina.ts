// Pruebas de la página de cada espejo (plan 0.8.0, C2; plan-mejoras 14i), del
// aviso de su primera subida (14s), de lo que no espera (14c) y de cómo se dice
// el progreso en vivo de lo largo (C3, 14p). `npm run test:vectores` (con las demás).
import type { DestinoResumen, Equipo, TareaEnMarcha, TrabajoEspejo, TrabajoEspejoResumen } from "../src/lib/tipos";
import { empiezaDeCero, filasSubidas, madrugadaSiguiente, ocupaPorRepo, primeraSubida, resumenSubidas } from "../src/lib/espejoPagina";
import { reglaDeCopia } from "../src/lib/regla321";
import { caminoDeCopia } from "../src/lib/camino";
import { cuerpoAlmacen, hrefEspejo, paraOrden, reduceTrabajos, sinTrabajo } from "../src/lib/espejoTrabajos";
import { esDestructiva } from "../src/lib/cripto/ordenes";
import { cifrasTarea, quienEmpezo, textoCorto, textoFase } from "../src/lib/textoProgreso";

let fallos = 0;
let total = 0;
const sinFinos = (v: unknown) => JSON.stringify(v).replace(/[  ]/g, " ");
function igual(nombre: string, obtenido: unknown, esperado: unknown) {
  total++;
  const ok = sinFinos(obtenido) === sinFinos(esperado);
  if (!ok) fallos++;
  console.log(`${ok ? "ok  " : "MAL "} ${nombre}${ok ? "" : `\n       obtenido: ${JSON.stringify(obtenido)}\n       esperado: ${JSON.stringify(esperado)}`}`);
}
const cierto = (nombre: string, v: boolean) => igual(nombre, v, true);

const AHORA = Date.parse("2026-10-14T12:00:00-05:00");
const iso = (h: number) => new Date(AHORA - h * 3600_000).toISOString();
const base = { so: "Windows 11", version_agente: "0.8.0", box_pub: "", sign_pub: "", sal_equipo: "", etiqueta: null, modo: "gestionado" as const, confirmado: true, conectado: true, ultimo_contacto: iso(0), estado_servicio: "en_marcha" as const, siguiente_seq: 1 };
const FRENO = { pct: 10, min_archivos: 100, min_faltan: 20, accion: "confirmar" as const };

const nube: TrabajoEspejoResumen = {
  id: "t1",
  nombre: "Todo a la nube",
  activo: true,
  quien: "almacen",
  que: { tipo: "repos", repos: ["pc01/docs", "pc01/caja"] },
  adonde: { tipo: "nube", nube: "Backblaze Oficina", carpeta: "Resguardo" },
  cuando: { tras_copia: true },
  retencion: { modo: "retraso", dias: 30 },
  freno: FRENO,
  orden: 0,
  ultima: iso(2),
  resultado: "Espejo «Todo a la nube» hecho en Backblaze Oficina:Resguardo: 3 archivos nuevos (1 MB).",
  verificacion: { ultima: iso(20), archivos: 40, mal: 0 },
  bien: iso(2),
  nunca_bien: false,
  subidas: [
    { inicio: iso(50), fin: iso(49.5), resultado: "ok", motivo: "horario", archivos: 300, bytes: 9_000_000_000, ocupa: 9_000_000_000 },
    { inicio: iso(26), fin: iso(25.9), resultado: "error", motivo: "tras_copia", archivos: 0, bytes: 0, error: "La nube no respondió." },
    { inicio: iso(2.25), fin: iso(2), resultado: "ok", motivo: "tras_copia", archivos: 3, bytes: 1_000_000, borrados: 2, borrados_bytes: 500_000, verificados: 40, ocupa: 9_000_500_000 },
  ],
  ocupa: { bytes: 9_000_500_000, repos: { "pc01/docs": 8_000_000_000, "pc01/caja": 1_000_500_000 }, cuando: iso(2) },
};

console.log("\n· 14i: el registro de subidas de un espejo (lib/espejoPagina.ts)");
{
  const f = filasSubidas(nube);
  igual("de la más reciente a la más antigua, con su duración", f.map((x) => [x.resultado, x.duracion_s]), [
    ["ok", 900],
    ["error", 360],
    ["ok", 1800],
  ]);
  igual("el resumen: vueltas, fallidas, lo subido, lo borrado y la duración media de las buenas", resumenSubidas(f), {
    vueltas: 3,
    fallidas: 1,
    bytes: 9_001_000_000,
    archivos: 303,
    borrados: 2,
    borradosBytes: 500_000,
    mediaDuracion: 1350,
    desde: iso(50),
  });
  igual("lo que ocupa cada repositorio en el destino, de mayor a menor", ocupaPorRepo(nube).map((x) => x.clave), ["pc01/docs", "pc01/caja"]);
  igual("sin registro (agente anterior): nada", [filasSubidas({ subidas: null }).length, ocupaPorRepo({ ocupa: null }).length], [0, 0]);
}

// Un almacén con el repositorio «docs» de PC-01 y el espejo «t1» a la nube.
const almacen: Equipo = {
  ...base,
  id: "a1",
  nombre: "ALMACEN-01",
  rol: "almacenamiento",
  resumen: {
    admite: ["zonas_almacen", "espejo_flexible", "espejo_trabajos", "espejo_primera"],
    guarda_copias: {
      activo: true,
      puerto: 8000,
      carpeta: "D:\\Resguardo",
      usuarios: 1,
      repositorios: [{ usuario: "pc01", repos: ["docs", "caja"] }],
      nubes: [{ nombre: "Backblaze Oficina", tipo: "b2" }],
      espejo: {
        hora: "02:00",
        destinos: [{ tipo: "nube", nube: "Backblaze Oficina", carpeta: "Resguardo", ultima: iso(2), resultado: "ok", repos: ["pc01/docs", "pc01/caja"], tras_copia: true, verificacion: nube.verificacion, trabajo: "t1" }],
        trabajos: [nube],
      },
    },
  },
};
const enAlmacen: DestinoResumen = { id: "dest-alm", nombre: "ALMACEN-01", tipo: "rest", donde: "https://almacen.ejemplo:8000/pc01/", equipo_almacen: "a1" };
const diario = { dias: [1, 2, 3, 4, 5, 6, 7], horas: ["10:00"] };
const pc: Equipo = {
  ...base,
  id: "e1",
  nombre: "PC-01",
  rol: "agente",
  resumen: {
    destinos: [enAlmacen],
    repositorios: [
      { id: "docs", nombre: "Documentos", destino: "dest-alm", bytes: 70_000_000_000, versiones: 12 },
      { id: "caja", nombre: "Caja", destino: "dest-alm", bytes: 5_000_000_000, versiones: 3 },
    ],
    copias: [{ id: "k1", nombre: "Documentos", repo: "docs", horario: diario, ultima: { cuando: iso(3), estado: "ok" } }],
  },
};
const equipos = [almacen, pc];

console.log("\n· 14h: «su copia en <destino>», con su tamaño allí y su página (lib/camino.ts)");
{
  const rc = reglaDeCopia(pc, pc.resumen!.copias![0], equipos, null, [], AHORA);
  const c = caminoDeCopia(rc, { cliente: "c1", ahora: AHORA, equipos });
  const e = c.find((x) => x.clase === "espejo")!;
  igual("el enlace va a la página del espejo", e.href, hrefEspejo("c1", "a1", "t1"));
  igual("lo que ocupa este repositorio allí y su comprobación", [e.ocupa, e.verificacion?.archivos], [8_000_000_000, 40]);
  const vivo = caminoDeCopia(rc, { cliente: "c1", ahora: AHORA, equipos, enVivo: (eq, r, t) => (t === "espejo" && eq === "a1" && r === "t1" ? "Subiendo 30 %" : null) });
  igual("subiendo ahora (14p): en vivo", [vivo.find((x) => x.clase === "espejo")!.estado, vivo.find((x) => x.clase === "espejo")!.vivo], ["Subiendo 30 %", true]);
}

console.log("\n· 14s: la primera subida, avisada (lib/espejoPagina.ts)");
{
  const nuevo: TrabajoEspejo = { ...paraOrden(nube), id: "t9", nombre: "Docs a otra nube", que: { tipo: "repos", repos: ["pc01/docs"] } };
  igual("lo que subirá: lo que ocupa cada repositorio que entra", primeraSubida(nuevo, almacen, equipos), { bytes: 70_000_000_000, repos: 1, sinTamano: 0 });
  igual("todos: los dos", primeraSubida({ ...nuevo, que: { tipo: "todos" } }, almacen, equipos), { bytes: 75_000_000_000, repos: 2, sinTamano: 0 });
  igual("un repositorio de un equipo que no está aquí: sin su tamaño", primeraSubida({ ...nuevo, que: { tipo: "repos", repos: ["pc01/docs", "otro/x"] } }, { ...almacen, resumen: { ...almacen.resumen!, guarda_copias: { ...almacen.resumen!.guarda_copias!, repositorios: [{ usuario: "pc01", repos: ["docs"] }, { usuario: "otro", repos: ["x"] }] } } }, equipos), { bytes: 70_000_000_000, repos: 2, sinTamano: 1 });
  cierto("uno nuevo empieza de cero", empiezaDeCero(nuevo, null, true));
  cierto("cambiar su destino, también", empiezaDeCero({ ...nube, adonde: { tipo: "nube", nube: "Dropbox", carpeta: "R" } }, nube, false));
  igual("cambiar solo la hora, no", empiezaDeCero({ ...nube, cuando: { horario: diario } }, nube, false), false);
  const m = new Date(madrugadaSiguiente(Date.parse("2026-10-14T15:30:00")));
  igual("la próxima madrugada: mañana a las 2:00 (hora de la consola)", [m.getDate(), m.getHours(), m.getMinutes()], [15, 2, 0]);
  const m2 = new Date(madrugadaSiguiente(new Date(2026, 9, 14, 1, 10).getTime()));
  igual("de madrugada antes de las 2: hoy a las 2:00", [m2.getDate(), m2.getHours()], [14, 2]);
  igual("se manda con la orden", paraOrden({ ...nuevo, cuando: { tras_copia: true, primera_desde: "2026-10-15T07:00:00.000Z" } }).cuando.primera_desde, "2026-10-15T07:00:00.000Z");
}

console.log("\n· 14c: un espejo que nunca terminó bien se cambia o se quita sin espera (cripto/ordenes.ts)");
{
  const nunca: TrabajoEspejoResumen = { ...nube, id: "t2", nombre: "Recién puesto", nunca_bien: true, bien: null, subidas: [] };
  const ctx = { espejo: { trabajos: [nube, nunca] } };
  igual("quitar el que nunca terminó: sin espera", esDestructiva("guarda_copias", cuerpoAlmacen(sinTrabajo([nube, nunca].map(paraOrden), "t2")), undefined, ctx), false);
  igual("quitar el que ya terminó: espera", esDestructiva("guarda_copias", cuerpoAlmacen(sinTrabajo([nube, nunca].map(paraOrden), "t1")), undefined, ctx), true);
  igual("pausar el que nunca terminó: sin espera", reduceTrabajos([nunca], [{ ...paraOrden(nunca), activo: false }]), false);
  igual("quitar el espejo entero con uno que ya terminó: espera", esDestructiva("guarda_copias", { espejo: null }, undefined, ctx), true);
  igual("quitar el espejo entero si ninguno terminó: sin espera", esDestructiva("guarda_copias", { espejo: null }, undefined, { espejo: { trabajos: [nunca] } }), false);
  igual("un espejo de antes (sin trabajos): espera, como siempre", esDestructiva("guarda_copias", { espejo: null }, undefined, { espejo: { destinos: [] } }), true);
}

console.log("\n· C3/14p: cómo se dice lo largo en marcha (lib/textoProgreso.ts)");
{
  const t = (x: Partial<TareaEnMarcha>): TareaEnMarcha => ({ tipo: "verificar", repo: "docs", fase: "en_marcha", ...x });
  igual("en cola, pedida desde otra consola", textoFase(t({ fase: "en_cola", por: "Ana", otra_consola: true, consola: "Oficina" })), "En cola: verificar (pedida por Ana desde «Oficina»)");
  igual("en cola: el chip", textoCorto(t({ fase: "en_cola" })), "En cola…");
  igual("una verificación pedida desde otra consola", textoFase(t({ etapa: "Leyendo el 5 % de los datos…", otra_consola: true, consola: "Oficina" })), "Leyendo el 5 % de los datos (desde «Oficina»)");
  igual("por su horario, sin quién", textoFase(t({ etapa: "Leyendo el 5 % de los datos…" })), "Leyendo el 5 % de los datos");
  const e = t({ tipo: "espejo", repo: "t1", espejo: "t1", nombre: "Todo a la nube", etapa: "Primera subida", fase: "subiendo", porcentaje: 0.42, bytes: 30_000_000_000, bytes_total: 70_000_000_000, velocidad: 12_000_000, quedan_s: 3300 });
  igual("la primera subida de un espejo", textoFase(e), "Primera subida de «Todo a la nube»");
  igual("su chip", textoCorto(e), "Subiendo… 42 %");
  igual("sus cifras", cifrasTarea(e), ["30 GB de 70 GB", "12 MB/s", "quedan ~55 min"]);
  igual("la actualización del agente", textoFase(t({ tipo: "actualizacion", repo: "", etapa: "Bajando la versión nueva" })), "Bajando la versión nueva");
  igual("quién: desde la ventana del equipo", quienEmpezo(t({ en_el_equipo: true })), "desde la ventana del equipo");
  igual("quién: esta consola, sin nombre", quienEmpezo(t({ otra_consola: false })), null);
}

console.log(`\n${total - fallos} de ${total} comprobaciones correctas.`);
if (fallos) process.exit(1);
