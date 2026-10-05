// Pruebas de la lógica de «pulsar para ver más» (src/lib/detalle.ts): qué
// cambió entre dos versiones (unir páginas, filtrar, ordenar, agrupar), la
// selección en la URL (sin nombres de archivo), las vueltas de una versión y
// a qué lleva cada aviso. También el «agente» del simulador (mock/detalle.ts):
// sus recuentos cuadran con el informe y sus páginas con el total.
//
//   npm run test:vectores
import {
  agruparPorCarpeta,
  anteriorDeLaCopia,
  conSeleccion,
  cuentaFiltro,
  deltaDe,
  destinoAviso,
  filtrarCambios,
  ganchosDeVuelta,
  leerSeleccion,
  marcarCambiosArchivo,
  migasDe,
  partesRuta,
  prefijoComun,
  problemasRecientes,
  rutaLegible,
  unirPaginas,
  versionPorId,
  vueltaPorHora,
  vueltasDeVersion,
  type Cambio,
  type PaginaCambios,
} from "../src/lib/detalle";
import type { EjecucionInforme, EntradaHistorial, VersionInforme } from "../src/lib/tipos";

let fallos = 0;
let total = 0;
function igual(nombre: string, obtenido: unknown, esperado: unknown) {
  total++;
  const ok = JSON.stringify(obtenido) === JSON.stringify(esperado);
  if (!ok) fallos++;
  console.log(`${ok ? "ok  " : "MAL "} ${nombre}${ok ? "" : `\n       obtenido: ${JSON.stringify(obtenido)}\n       esperado: ${JSON.stringify(esperado)}`}`);
}
const cierto = (nombre: string, v: boolean) => igual(nombre, v, true);

console.log("\n· Qué cambió: páginas, filtros, orden y carpetas");
{
  const resumen = { nuevos: 2, cambiados: 2, borrados: 1, metadatos: 1, otros: 0, carpetas_nuevas: 1, carpetas_borradas: 0, bytes_anadidos: 900, bytes_quitados: 50 };
  const p1: PaginaCambios = {
    desde: "aaaaaaaa",
    hasta: "bbbbbbbb",
    resumen,
    total: 6,
    con_tamanos: true,
    indice: 0,
    siguiente: 3,
    cambios: [
      { ruta: "/C/Datos/Facturas/FV-1.pdf", tipo: "nuevo", bytes: 300 },
      { ruta: "/C/Datos/Facturas/FV-2.pdf", tipo: "nuevo", bytes: 100 },
      { ruta: "/C/Datos/Notas.txt", tipo: "cambiado", bytes: 50, bytes_antes: 80 },
    ],
  };
  const p2: PaginaCambios = {
    ...p1,
    indice: 3,
    siguiente: null,
    cambios: [
      { ruta: "/C/Datos/Viejo.xlsx", tipo: "borrado", bytes_antes: 700 },
      { ruta: "/C/Datos/Precios.xlsx", tipo: "cambiado", bytes: 1000, bytes_antes: 400 },
      { ruta: "/C/Datos/Fecha.txt", tipo: "metadatos", bytes: 5, bytes_antes: 5 },
      // Lo que no tiene la forma esperada se descarta.
      { ruta: "relativa.txt", tipo: "nuevo" },
      { ruta: "/C/Datos/x", tipo: "raro" as never },
      { ruta: "/C/Datos/Notas.txt", tipo: "cambiado", bytes: 1 },
      { ruta: "/C/Datos/neg.txt", tipo: "nuevo", bytes: -5 },
    ],
  };
  // Las páginas pueden llegar en cualquier orden.
  const d = unirPaginas([p2, p1]);
  igual("une las páginas en orden y sin repetidos", d.cambios.map((c) => c.ruta), [
    "/C/Datos/Facturas/FV-1.pdf",
    "/C/Datos/Facturas/FV-2.pdf",
    "/C/Datos/Notas.txt",
    "/C/Datos/Viejo.xlsx",
    "/C/Datos/Precios.xlsx",
    "/C/Datos/Fecha.txt",
    "/C/Datos/neg.txt",
  ]);
  igual("un tamaño negativo no se pinta", d.cambios.at(-1)!.bytes, undefined);
  igual("resumen", [d.resumen.nuevos, d.resumen.bytes_anadidos, d.desde, d.conTamanos], [2, 900, "aaaaaaaa", true]);
  const c = (ruta: string) => d.cambios.find((x) => x.ruta === ruta)!;
  igual("delta de un cambiado, un nuevo y un borrado", [deltaDe(c("/C/Datos/Precios.xlsx")), deltaDe(c("/C/Datos/Facturas/FV-1.pdf")), deltaDe(c("/C/Datos/Viejo.xlsx"))], [600, 300, -700]);
  igual("«cambiados» incluye solo metadatos", filtrarCambios(d.cambios, "cambiados", "").map((x) => partesRuta(x.ruta).nombre), ["Fecha.txt", "Notas.txt", "Precios.xlsx"]);
  igual("recuentos por filtro (del resumen, no de lo cargado)", [cuentaFiltro(d.resumen, "todos"), cuentaFiltro(d.resumen, "cambiados"), cuentaFiltro(d.resumen, "borrados")], [6, 3, 1]);
  igual("buscar sin acentos ni mayúsculas", filtrarCambios(d.cambios, "todos", "PRECIOS").map((x) => x.ruta), ["/C/Datos/Precios.xlsx"]);
  igual("ordenar por tamaño", filtrarCambios(d.cambios, "todos", "", "tamano").slice(0, 3).map((x) => partesRuta(x.ruta).nombre), ["Precios.xlsx", "Viejo.xlsx", "FV-1.pdf"]);
  igual("ordenar por cuánto cambió", filtrarCambios(d.cambios, "todos", "", "delta").slice(0, 2).map((x) => partesRuta(x.ruta).nombre), ["Viejo.xlsx", "Precios.xlsx"]);
  const g = agruparPorCarpeta(filtrarCambios(d.cambios, "todos", ""));
  igual("agrupa por carpeta", g.map((x) => [x.carpeta, x.cambios.length, x.nuevos, x.borrados]), [
    ["/C/Datos", 5, 1, 1],
    ["/C/Datos/Facturas", 2, 2, 0],
  ]);
  igual("carpetas por tamaño", agruparPorCarpeta(d.cambios, "tamano").map((x) => x.carpeta), ["/C/Datos", "/C/Datos/Facturas"]);
  igual("carpeta común", prefijoComun(d.cambios.map((x) => x.ruta)), "/C/Datos");
  igual("sin nada en común", prefijoComun(["/C/a/x", "/D/b/y"]), "/");
  igual("ruta de Windows", rutaLegible("/C/Users/Ana/Informe.docx"), "C:\\Users\\Ana\\Informe.docx");
  igual("raíz de una unidad", rutaLegible("/D"), "D:\\");
  igual("ruta de Linux", rutaLegible("/home/ana/x"), "/home/ana/x");
  igual("partes de una ruta", partesRuta("/C/a/b.txt"), { carpeta: "/C/a", nombre: "b.txt" });
  const primera = unirPaginas([{ hasta: "bbbbbbbb", desde: null, primera: true }]);
  cierto("primera versión: sin cambios ni errores", primera.primera && primera.cambios.length === 0 && primera.resumen.nuevos === 0);
}

console.log("\n· Selección en la URL (sin nombres de archivo)");
{
  const q = (s: string) => new URLSearchParams(s);
  igual("versión sola: su detalle", leerSeleccion(q("v=5437A6B7")).vista, "version");
  igual("qué cambió con filtro", leerSeleccion(q("v=5437a6b7&vista=cambios&filtro=nuevos&con=216ef38e")), { vista: "cambios", version: "5437a6b7", con: "216ef38e", filtro: "nuevos", vuelta: null, dia: null, desde: null, hasta: null });
  igual("sin versión, «cambios» no se abre", leerSeleccion(q("vista=cambios")).vista, null);
  igual("un id que no es hexadecimal se ignora", leerSeleccion(q("v=--help")).version, null);
  igual("filtro desconocido: todos", leerSeleccion(q("v=5437a6b7&vista=cambios&filtro=raro")).filtro, "todos");
  igual("vuelta por su hora", leerSeleccion(q("vuelta=2026-10-03T00:04:19.412Z")).vista, "vuelta");
  igual("una hora mal formada no abre nada", leerSeleccion(q("vuelta=ayer")).vista, null);
  igual("día", leerSeleccion(q("dia=2026-10-01")).dia, "2026-10-01");
  igual("día mal formado", leerSeleccion(q("dia=1/10/2026")).dia, null);
  igual("intervalo de días", [leerSeleccion(q("desde=2026-09-01&hasta=2026-09-30")).desde, leerSeleccion(q("desde=2026-09-01&hasta=2026-09-30")).hasta], ["2026-09-01", "2026-09-30"]);
  igual("intervalo al revés: se ordena", leerSeleccion(q("desde=2026-09-30&hasta=2026-09-01")).desde, "2026-09-01");
  igual("intervalo a medias o mal formado: nada", [leerSeleccion(q("desde=2026-09-01")).desde, leerSeleccion(q("desde=ayer&hasta=2026-09-01")).hasta], [null, null]);
  igual("con día, el día manda", leerSeleccion(q("dia=2026-10-01&desde=2026-09-01&hasta=2026-09-30")).desde, null);
  igual("cambiar la selección conserva lo demás", conSeleccion(q("retencion=1&v=aaaaaaaa"), { vista: "cambios", version: "bbbbbbbb", filtro: "cambiados" }), "?retencion=1&v=bbbbbbbb&vista=cambios&filtro=cambiados");
  igual("cerrar quita solo lo del panel", conSeleccion(q("dia=2026-10-01&v=aaaaaaaa&vista=ocupa"), { cerrar: true }), "?dia=2026-10-01");
  igual("«todos» y «version» no se escriben", conSeleccion(q(""), { vista: "version", version: "aaaaaaaa", filtro: "todos" }), "?v=aaaaaaaa");
  const migas = migasDe(leerSeleccion(q("v=aaaaaaaa&vista=cambios&filtro=borrados")), (id) => `V ${id}`);
  igual("migas de qué cambió", migas.map((m) => m.texto), ["V aaaaaaaa", "Qué cambió", "Borrados"]);
  igual("migas de comparar con otra", migasDe(leerSeleccion(q("v=aaaaaaaa&vista=cambios&con=cccccccc")), (id) => `V ${id}`).map((m) => m.texto), ["V aaaaaaaa", "Comparada con V cccccccc"]);
}

console.log("\n· Versiones, vueltas y avisos");
{
  const v = (id: string, hora: string, copia = "docs", extra: Partial<VersionInforme> = {}): VersionInforme => ({
    id,
    hora,
    copia,
    total_bytes: 1000,
    anadido: 10,
    anadido_empaquetado: 5,
    archivos_nuevos: 1,
    archivos_cambiados: 2,
    archivos_sin_cambios: 3,
    duracion_s: 60,
    etiquetas: [],
    ...extra,
  });
  const versiones = [v("dddddddd", "2026-10-02T18:00:00Z"), v("cccccccc", "2026-10-02T12:00:00Z", "sql"), v("bbbbbbbb", "2026-10-01T18:00:00Z"), v("aaaaaaaa", "2026-10-01T12:00:00Z")];
  igual("anterior de la misma copia (salta la otra copia)", anteriorDeLaCopia(versiones, versiones[0])?.id, "bbbbbbbb");
  igual("la primera no tiene anterior", anteriorDeLaCopia(versiones, versiones[3]), null);
  igual("versión por id corto o largo", [versionPorId(versiones, "dddddddd")?.id, versionPorId(versiones, "dddddddd0123")?.id, versionPorId(versiones, null)], ["dddddddd", "dddddddd", null]);
  const e = (hora: string, resultado: EjecucionInforme["resultado"], copia = "docs", extra: Partial<EjecucionInforme> = {}): EjecucionInforme => ({ hora, copia, resultado, mensaje_corto: resultado === "ok" ? null : "No se pudo conectar", duracion_s: 120, ...extra });
  const ejecuciones = [e("2026-10-02T18:02:00Z", "ok", "docs", { reintento: true }), e("2026-10-02T17:00:00Z", "fallo"), e("2026-10-02T16:00:00Z", "fallo"), e("2026-10-02T12:02:00Z", "ok", "sql"), e("2026-10-01T18:02:00Z", "ok")];
  igual("vueltas de una versión: la que la guardó y los fallos de antes", vueltasDeVersion(ejecuciones, versiones, versiones[0]).map((x) => [x.hora.slice(11, 16), x.resultado]), [
    ["18:02", "ok"],
    ["17:00", "fallo"],
    ["16:00", "fallo"],
  ]);
  igual("vuelta por su hora (con un minuto de margen)", vueltaPorHora(ejecuciones, "2026-10-02T17:00:30Z")?.hora, "2026-10-02T17:00:00Z");
  igual("ninguna cerca", vueltaPorHora(ejecuciones, "2026-10-05T10:00:00Z"), null);
  igual("problemas recientes", problemasRecientes(ejecuciones).length, 2);
  const historial: EntradaHistorial[] = [{ id: "h1", hora: "2026-10-02T18:02:10Z", tipo: "copia", repo: "r", copia: "docs", resultado: "ok", ganchos: [{ tipo: "sqlserver", estado: "ok", mensaje: "2 bases" }] }];
  igual("ganchos de esa vuelta, del historial", ganchosDeVuelta(historial, ejecuciones[0], "r").map((g) => g.mensaje), ["2 bases"]);
  igual("de otro repositorio, no", ganchosDeVuelta(historial, ejecuciones[0], "otro"), []);
  const repos = [{ id: "r", ejecuciones, verificacion: { resultado: "fallo", ultima: "2026-10-02T00:00:00Z" } }];
  igual("aviso de copia fallida → su vuelta", destinoAviso({ tipo: "copia_fallida", equipo: "e", creado: "2026-10-02T17:01:00Z" }, repos), { repo: "r", vuelta: "2026-10-02T17:00:00Z" });
  igual("verificación fallida → el repositorio", destinoAviso({ tipo: "verificacion_fallida", equipo: "e", creado: "2026-10-02T17:01:00Z" }, repos), { repo: "r", vuelta: null });
  igual("sin equipo, nada", destinoAviso({ tipo: "copia_fallida", equipo: null, creado: "2026-10-02T17:01:00Z" }, repos), null);
  igual("demasiado lejos, nada", destinoAviso({ tipo: "copia_fallida", equipo: "e", creado: "2026-10-09T17:01:00Z" }, repos), null);
  const archivo = marcarCambiosArchivo([
    { version: "a", cuando: "2026-10-01T10:00:00Z", bytes: 5, modificado: "x" },
    { version: "c", cuando: "2026-10-03T10:00:00Z", bytes: 6, modificado: "y" },
    { version: "b", cuando: "2026-10-02T10:00:00Z", bytes: 5, modificado: "x" },
  ]);
  igual("versiones de un archivo: dónde cambió", archivo.map((x) => [x.version, x.cambio]), [
    ["c", true],
    ["b", false],
    ["a", true],
  ]);
}

console.log("\n· Simulador: «diferencias», «ocupa» e «historial_archivo»");
{
  const mock = await import("../src/mock/estado");
  await mock.sembrar();
  const { operarDetalle } = await import("../src/mock/detalle");
  const eq = mock.estado.equipos.find((x) => x.nombre === "RECEPCION");
  const inf = eq?.informes[0]?.datos.repos?.[0];
  if (!eq || !inf?.versiones.length) {
    console.log("(sin datos del simulador: se omite)");
  } else {
    const s = { equipo: eq.id, repo: inf.id };
    const v = inf.versiones.find((x: VersionInforme, i: number) => i > 0 && (x.archivos_nuevos ?? 0) + (x.archivos_cambiados ?? 0) > 0)!;
    let n = 0;
    const paginas: PaginaCambios[] = [];
    let indice: number | null = 0;
    while (indice != null) {
      const p = (await operarDetalle("diferencias", { hasta: v.id, indice }, s, () => n++)) as unknown as PaginaCambios;
      paginas.push(p);
      indice = p.siguiente ?? null;
    }
    const d = unirPaginas(paginas);
    cierto("dice que trabaja en la primera página", n >= 1);
    igual("los nuevos cuadran con el informe", d.resumen.nuevos, v.archivos_nuevos);
    igual("los cambiados (con solo metadatos) cuadran con el informe", d.resumen.cambiados + d.resumen.metadatos, v.archivos_cambiados);
    igual("las páginas suman el total", d.cambios.length, paginas[0].total);
    const cambios: Cambio[] = d.cambios;
    cierto("todas las rutas son de versión", cambios.every((c) => c.ruta.startsWith("/")));
    igual("versión inválida", ((await operarDetalle("diferencias", { hasta: "--help" }, s, () => {})) as { error?: string }).error, "Versión no válida.");
    const ocupa = (await operarDetalle("ocupa", { version: v.id }, s, () => {})) as { carpetas: { bytes: number }[] };
    cierto("ocupa: carpetas de mayor a menor", ocupa.carpetas.every((c, i) => i === 0 || ocupa.carpetas[i - 1].bytes >= c.bytes));
    const h = (await operarDetalle("historial_archivo", { ruta: cambios[0].ruta }, s, () => {})) as { versiones: { cuando: string }[] };
    cierto("historial de un archivo, más reciente primero", h.versiones.length > 0 && h.versiones.every((x, i) => i === 0 || Date.parse(h.versiones[i - 1].cuando) >= Date.parse(x.cuando)));
    igual("ruta con ..", ((await operarDetalle("historial_archivo", { ruta: "/C/../x" }, s, () => {})) as { error?: string }).error, "Ruta no válida dentro de la versión.");
  }
}

console.log(`\n${total - fallos} de ${total} comprobaciones correctas (detalle).`);
if (fallos) process.exit(1);
