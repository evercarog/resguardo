// Pruebas de la lógica de «Buscar archivos» (src/lib/buscarArchivos.ts):
// comprobar lo que manda el equipo (`buscar_todas`), unir sus páginas, resumir
// cada archivo (cuándo cambió, tamaños), la línea de tiempo, el orden, los
// filtros de la URL (sin el texto ni nombres) y las fechas. También el
// «agente» del simulador (mock/buscar.ts): valida como el de verdad, sus
// páginas suman el total y cuadra con `historial_archivo`.
//
//   npm run test:vectores
import {
  borradoDespues,
  conFiltros,
  diaDeHace,
  errorTexto,
  filtrarEncontrados,
  fraseRecorte,
  leerFiltros,
  lineaArchivo,
  ordenarArchivos,
  rangoIso,
  resumirArchivo,
  trozosNombre,
  unirBusqueda,
  type PaginaBusqueda,
} from "../src/lib/buscarArchivos";

let fallos = 0;
let total = 0;
function igual(nombre: string, obtenido: unknown, esperado: unknown) {
  total++;
  const ok = JSON.stringify(obtenido) === JSON.stringify(esperado);
  if (!ok) fallos++;
  console.log(`${ok ? "ok  " : "MAL "} ${nombre}${ok ? "" : `\n       obtenido: ${JSON.stringify(obtenido)}\n       esperado: ${JSON.stringify(esperado)}`}`);
}
const cierto = (nombre: string, x: boolean) => igual(nombre, x, true);

const V = (id: string, cuando: string, bytes: number | null, modificado: string | null = null) => ({ version: id.padEnd(8, "0"), cuando, bytes, modificado });

console.log("· El texto que se busca");
{
  igual("vacío", errorTexto("  "), "Escribe qué buscar (de 2 a 100 caracteres).");
  igual("una letra", errorTexto("a"), "Escribe qué buscar (de 2 a 100 caracteres).");
  igual("dos letras", errorTexto(" fa "), null);
  igual("100 caracteres con tildes", errorTexto("ñ".repeat(100)), null);
  igual("101", errorTexto("x".repeat(101)) != null, true);
  igual("con barra", errorTexto("Facturas/2026"), "Busca por el nombre del archivo (sin / ni \\).");
  igual("con barra invertida", errorTexto("a\\b") != null, true);
  igual("con control", errorTexto("fac\u0001tura") != null, true);
  igual("con corchetes y asteriscos (los cambia el agente)", errorTexto("Factura [1] *.pdf"), null);
}

console.log("\n· Unir las páginas y resumir cada archivo");
{
  const p1: PaginaBusqueda = {
    texto: "factura",
    total_archivos: 4,
    coincidencias: 7,
    versiones_buscadas: 5,
    versiones_en_rango: 9,
    recortado: true,
    motivo: "limite",
    indice: 0,
    siguiente: 2,
    archivos: [
      {
        ruta: "/C/Datos/Factura 1.pdf",
        versiones: [V("cc", "2026-10-03T10:00:00Z", 30, "2026-10-03T09:00:00Z"), V("bb", "2026-10-02T10:00:00Z", 20, "2026-10-02T09:00:00Z"), V("aa", "2026-10-01T10:00:00Z", 20, "2026-10-01T09:00:00Z")],
      },
      { ruta: "relativa/mal.pdf", versiones: [V("aa", "2026-10-01T10:00:00Z", 1)] },
    ],
  };
  const p2: PaginaBusqueda = {
    indice: 2,
    siguiente: null,
    archivos: [
      { ruta: "/C/Datos/factura-vieja.pdf", versiones: [V("aa", "2026-10-01T10:00:00Z", 5)], recortado: true },
      { ruta: "/C/Datos/Factura 1.pdf", versiones: [V("aa", "2026-10-01T10:00:00Z", 1)] },
      { ruta: "/C/Datos/mala-version.pdf", versiones: [{ version: "--help", cuando: "2026-10-01T10:00:00Z", bytes: 1 }, { version: "dddddddd", cuando: "ayer", bytes: 1 }] },
      { ruta: "/C/Datos/tam-raro.pdf", versiones: [{ version: "eeeeeeee", cuando: "2026-10-02T10:00:00Z", bytes: -5 }] },
    ],
  };
  const r = unirBusqueda("docs", [p2, p1]);
  igual("rutas válidas, sin repetir, en orden de páginas", r.archivos.map((a) => a.ruta), ["/C/Datos/Factura 1.pdf", "/C/Datos/factura-vieja.pdf", "/C/Datos/tam-raro.pdf"]);
  igual("las cifras salen de la primera página", [r.totalArchivos, r.coincidencias, r.versionesBuscadas, r.versionesEnRango, r.recortado, r.motivo], [4, 7, 5, 9, true, "limite"]);
  const f = r.archivos[0];
  igual("nombre y carpeta", [f.nombre, f.carpeta, f.repo], ["Factura 1.pdf", "/C/Datos", "docs"]);
  igual("la más reciente y la primera", [f.ultima.version, f.primera.version], ["cc000000", "aa000000"]);
  igual("cambió por última vez en la más reciente", f.ultimoCambio?.version, "cc000000");
  igual("bb: otra fecha de modificación, mismo tamaño → cambió", f.versiones.map((v) => v.cambio), [true, true, true]);
  igual("cambios (sin contar cuando aparece)", f.cambios, 2);
  igual("tamaños", [f.bytes, f.bytesMin, f.bytesMax], [30, 20, 30]);
  igual("recortado por archivo", r.archivos[1].recortado, true);
  igual("tamaño negativo: sin tamaño", r.archivos[2].bytes, null);
  const igualSiempre = resumirArchivo("r", "/x/a.txt", [V("bb", "2026-10-02T10:00:00Z", 5, "2026-09-01T00:00:00Z"), V("aa", "2026-10-01T10:00:00Z", 5, "2026-09-01T00:00:00Z")])!;
  igual("igual desde que apareció: sin último cambio", [igualSiempre.ultimoCambio, igualSiempre.cambios], [null, 0]);
  igual("sin versiones: nada", resumirArchivo("r", "/x/a.txt", []), null);
  igual("página sin nada", unirBusqueda("r", []).archivos, []);
  igual("motivo desconocido", unirBusqueda("r", [{ motivo: "otro" as never }]).motivo, null);
}

console.log("\n· Ordenar, filtrar y resaltar");
{
  const a = resumirArchivo("r1", "/C/A/Zeta.pdf", [V("bb", "2026-10-05T10:00:00Z", 10), V("aa", "2026-10-01T10:00:00Z", 1)])!;
  const b = resumirArchivo("r2", "/C/B/alfa.pdf", [V("cc", "2026-10-03T10:00:00Z", 99)])!;
  const c = resumirArchivo("r1", "/C/Árbol/Beta.pdf", [V("dd", "2026-10-04T10:00:00Z", 50), V("cc", "2026-10-03T10:00:00Z", 40), V("bb", "2026-10-02T10:00:00Z", 30)])!;
  const xs = [a, b, c];
  igual("recientes primero", ordenarArchivos(xs).map((x) => x.nombre), ["Zeta.pdf", "Beta.pdf", "alfa.pdf"]);
  igual("por nombre (sin mayúsculas)", ordenarArchivos(xs, "nombre").map((x) => x.nombre), ["alfa.pdf", "Beta.pdf", "Zeta.pdf"]);
  igual("por tamaño", ordenarArchivos(xs, "tamano").map((x) => x.nombre), ["alfa.pdf", "Beta.pdf", "Zeta.pdf"]);
  igual("los que más cambiaron", ordenarArchivos(xs, "cambios").map((x) => x.nombre), ["Beta.pdf", "Zeta.pdf", "alfa.pdf"]);
  igual("filtrar sin tildes por carpeta", filtrarEncontrados(xs, "arbol").map((x) => x.nombre), ["Beta.pdf"]);
  igual("filtro vacío: todos", filtrarEncontrados(xs, " ").length, 3);
  igual("resaltar sin mayúsculas", trozosNombre("Factura Octubre.PDF", "octubre"), [
    { texto: "Factura ", marca: false },
    { texto: "Octubre", marca: true },
    { texto: ".PDF", marca: false },
  ]);
  igual("sin coincidencia: entero", trozosNombre("x.pdf", "zz"), [{ texto: "x.pdf", marca: false }]);
  igual("al principio", trozosNombre("FV-1.pdf", "fv"), [
    { texto: "FV", marca: true },
    { texto: "-1.pdf", marca: false },
  ]);
}

console.log("\n· Línea de tiempo y «ya no está»");
{
  const repo = [
    { id: "aa000000", cuando: "2026-10-01T00:00:00Z" },
    { id: "bb000000", cuando: "2026-10-02T00:00:00Z" },
    { id: "cc000000", cuando: "2026-10-03T00:00:00Z" },
    { id: "dd000000", cuando: "2026-10-05T00:00:00Z" },
  ];
  const a = resumirArchivo("r", "/x/a.txt", [V("cc", "2026-10-03T00:00:00Z", 9), V("bb", "2026-10-02T00:00:00Z", 5)])!;
  const l = lineaArchivo(a, repo);
  igual("una marca por versión del repositorio, por fecha", l.map((m) => [m.version, m.estado, Math.round(m.x * 100)]), [
    ["aa000000", "falta", 0],
    ["bb000000", "aparece", 25],
    ["cc000000", "cambio", 50],
    ["dd000000", "falta", 100],
  ]);
  cierto("ya no está en la versión más reciente", borradoDespues(a, repo));
  const b = resumirArchivo("r", "/x/b.txt", [V("dd", "2026-10-05T00:00:00Z", 1)])!;
  cierto("está en la más reciente", !borradoDespues(b, repo));
  igual("sin las versiones del repositorio: solo las del archivo", lineaArchivo(a, []).map((m) => m.estado), ["aparece", "cambio"]);
  igual("una sola versión: en el centro", lineaArchivo(b, []).map((m) => m.x), [0.5]);
  igual("ids cortos y largos se reconocen", lineaArchivo(resumirArchivo("r", "/x/c", [V("bb", "2026-10-02T00:00:00Z", 1)])!, [{ id: "bb000000ffff", cuando: "2026-10-02T00:00:00Z" }])[0].estado, "aparece");
}

console.log("\n· La URL: equipo, repositorio y fechas (nunca el texto)");
{
  const f = leerFiltros(new URLSearchParams("equipo=abc-1&repo=todos&desde=2026-10-05&hasta=2026-10-01&texto=secreto&v=aaaaaaaa"));
  igual("lee y ordena las fechas", f, { equipo: "abc-1", repo: "todos", desde: "2026-10-01", hasta: "2026-10-05" });
  igual("lo que no vale se ignora", leerFiltros(new URLSearchParams("equipo=<x>&repo=a%2Fb&desde=ayer&hasta=2026-13-45")), { equipo: null, repo: null, desde: null, hasta: null });
  const q = conFiltros(new URLSearchParams("v=aaaaaaaa&vista=cambios"), { equipo: "e1", repo: "todos", desde: "2026-10-01", hasta: null });
  igual("pone los filtros sin tocar el panel", q, "?v=aaaaaaaa&vista=cambios&equipo=e1&repo=todos&desde=2026-10-01");
  igual("quitar", conFiltros(new URLSearchParams("equipo=e1&desde=2026-10-01"), { desde: null }), "?equipo=e1");
  cierto("nunca lleva un texto", !/texto|factura/i.test(q));
  const r = rangoIso("2026-10-01", "2026-10-02");
  const d = new Date(r.desde!);
  const h = new Date(r.hasta!);
  igual("desde: las 00:00 locales", [d.getFullYear(), d.getMonth(), d.getDate(), d.getHours(), d.getMinutes()], [2026, 9, 1, 0, 0]);
  igual("hasta: las 23:59:59 locales", [h.getDate(), h.getHours(), h.getMinutes(), h.getSeconds()], [2, 23, 59, 59]);
  igual("sin fechas: nada", rangoIso(null, null), {});
  igual("solo desde", Object.keys(rangoIso("2026-10-01", null)), ["desde"]);
  igual("hace 7 días", diaDeHace(7, new Date(2026, 9, 10, 12).getTime()), "2026-10-03");
}

console.log("\n· Por qué no está todo");
{
  igual("completo", fraseRecorte({ recortado: false, motivo: null, versionesBuscadas: 5, versionesEnRango: 5 }), null);
  cierto("límite", /más recientes/.test(fraseRecorte({ recortado: true, motivo: "limite", versionesBuscadas: 5, versionesEnRango: 5 }) ?? ""));
  cierto("tiempo", /5 minutos/.test(fraseRecorte({ recortado: true, motivo: "tiempo", versionesBuscadas: 5, versionesEnRango: 5 })!));
  cierto("solo las más recientes", /1000|1\.000/.test(fraseRecorte({ recortado: false, motivo: null, versionesBuscadas: 1000, versionesEnRango: 1500 })!));
  cierto("un archivo recortado", /todas sus versiones/.test(fraseRecorte({ recortado: true, motivo: null, versionesBuscadas: 5, versionesEnRango: 5 })!));
}

console.log("\n· Simulador: «buscar_todas»");
{
  const mock = await import("../src/mock/estado");
  await mock.sembrar();
  const { buscarTodas } = await import("../src/mock/buscar");
  const { operarDetalle } = await import("../src/mock/detalle");
  const eq = mock.estado.equipos.find((x) => x.nombre === "RECEPCION");
  const inf = eq?.informes[0]?.datos.repos?.[0];
  if (!eq || !inf?.versiones.length) {
    console.log("(sin datos del simulador: se omite)");
  } else {
    const s = { equipo: eq.id, repo: inf.id };
    let trabajando = 0;
    const todo = async (args: Record<string, unknown>) => {
      const paginas: PaginaBusqueda[] = [];
      let indice: number | null = 0;
      while (indice != null) {
        const p = (await buscarTodas({ ...args, indice }, s, () => trabajando++)) as PaginaBusqueda & { error?: string };
        if (p.error) throw new Error(p.error);
        paginas.push(p);
        indice = p.siguiente ?? null;
      }
      return { paginas, r: unirBusqueda(inf.id, paginas) };
    };
    const { paginas, r } = await todo({ texto: "FV-" });
    cierto("dice que trabaja", trabajando >= 1);
    cierto("varias páginas", paginas.length > 1);
    igual("las páginas suman el total", r.archivos.length, paginas[0].total_archivos);
    cierto("todo lo encontrado lleva «fv-» en el nombre", r.archivos.every((a) => a.nombre.toLowerCase().includes("fv-")));
    cierto("los más recientes primero", r.archivos.every((a, i) => i === 0 || Date.parse(r.archivos[i - 1].ultima.cuando) >= Date.parse(a.ultima.cuando)));
    const a = r.archivos.find((x) => !x.ruta.includes("anulada"))!;
    const h = (await operarDetalle("historial_archivo", { ruta: a.ruta }, s, () => {})) as { versiones: { version: string }[] };
    igual("cuadra con «historial_archivo»", a.versiones.map((v) => v.version), h.versiones.map((v) => v.version));
    const borrado = r.archivos.find((x) => x.ruta.includes("anulada"));
    const delRepo = inf.versiones.map((v: { id: string; hora: string }) => ({ id: v.id, cuando: v.hora }));
    cierto("un archivo borrado ya no está en la más reciente", !!borrado && borradoDespues(borrado, delRepo));
    const hb = (await operarDetalle("historial_archivo", { ruta: borrado!.ruta }, s, () => {})) as { versiones: { version: string }[] };
    igual("el borrado también cuadra con «historial_archivo»", borrado!.versiones.map((v) => v.version), hb.versiones.map((v) => v.version));
    const semana = rangoIso(diaDeHace(7), null);
    const { r: r7 } = await todo({ texto: "FV-", ...semana });
    cierto("con fechas, en menos versiones", r7.versionesBuscadas < r.versionesBuscadas && r7.archivos.every((x) => x.versiones.every((v) => Date.parse(v.cuando) >= Date.parse(semana.desde!))));
    const { r: rc } = await todo({ texto: "[borrador]" });
    igual("con corchetes en el texto", rc.archivos.map((x) => x.nombre), ["Precios nuevos [borrador].xlsx"]);
    const { r: rl } = await todo({ texto: "FV-", max: 3 });
    igual("con un máximo", [rl.coincidencias <= 3, rl.recortado, rl.motivo], [true, true, "limite"]);
    const { r: nada } = await todo({ texto: "no existe esto" });
    igual("nada", nada.archivos.length, 0);
    for (const [args, error] of [
      [{ texto: "a" }, "Escribe qué buscar (de 2 a 100 caracteres)."],
      [{ texto: "x/y" }, "Busca por el nombre del archivo (sin / ni \\ ni caracteres de control)."],
      [{ texto: "fv", desde: "ayer" }, "Fecha no válida."],
      [{ texto: "fv", desde: "2026-10-03T00:00:00Z", hasta: "2026-10-01T00:00:00Z" }, "La fecha «desde» es posterior a «hasta»."],
      [{ texto: "fv", max: 0 }, "«max» no válido (de 1 a 2000)."],
      [{ texto: "fv", indice: -1 }, "«indice» no válido."],
    ] as [Record<string, unknown>, string][])
      igual(`error: ${JSON.stringify(args)}`, ((await buscarTodas(args, s, () => {})) as { error?: string }).error, error);
  }
}

console.log(`\n${total - fallos} de ${total} comprobaciones correctas (buscar).`);
if (fallos) process.exit(1);
