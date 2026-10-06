// Pruebas de «Retención en detalle» (src/lib/retencionDetalle.ts): las vueltas
// que anota el equipo, por qué se va cada versión (lo mismo que `seQuedan` y que
// el agente) y la previsión de lo que quitará (la próxima vuelta y los días que
// vienen, con las copias que llegan según su horario). `npm run test:vectores`.
import { seQuedan, SIEMPRE } from "../src/lib/retencion";
import { claveDia, explicar, leerVuelta, marcasPorDia, prever, textoSeQueda, textoSeVa, totales, vueltasDelRepo, type EntradaRetencion } from "../src/lib/retencionDetalle";
import type { Regla } from "../src/lib/tipos";

let fallos = 0;
let total = 0;
function igual(nombre: string, obtenido: unknown, esperado: unknown) {
  total++;
  const ok = JSON.stringify(obtenido) === JSON.stringify(esperado);
  if (!ok) fallos++;
  console.log(`${ok ? "ok  " : "MAL "} ${nombre}${ok ? "" : `\n       obtenido: ${JSON.stringify(obtenido)}\n       esperado: ${JSON.stringify(esperado)}`}`);
}

// Un azar con semilla (las pruebas salen siempre igual).
let semilla = 12345;
const azar = () => ((semilla = (semilla * 1103515245 + 12345) % 2 ** 31) / 2 ** 31);

console.log("\n· explicar: lo mismo que seQuedan (las reglas de restic forget)");
{
  const reglas: Regla[] = [
    { diarias: 7, semanales: 4, mensuales: 12, anuales: 2 },
    { horarias: 5, diarias: 3, semanales: 0, mensuales: 0, anuales: 0 },
    { diarias: 0, semanales: 0, mensuales: SIEMPRE, anuales: 0, plazos: { horarias: "2d", diarias: "15d" } },
    { diarias: 0, semanales: 2, mensuales: 0, anuales: 0, plazos: { diarias: "10d" } },
  ];
  for (const [i, r] of reglas.entries()) {
    let t = new Date(2026, 9, 5, 18, 0).getTime();
    const horas: Date[] = [];
    for (let n = 0; n < 300; n++) {
      horas.push(new Date(t));
      t -= Math.floor(azar() * 30 * 3600_000);
    }
    const q = seQuedan(horas, r);
    const ex = explicar(horas, r);
    igual(`regla ${i + 1}: mismas que se quedan`, ex.map((x) => !!x.queda), q);
    igual(`regla ${i + 1}: cada una con su porqué`, ex.every((x) => (x.queda ? !x.motivo : !!x.motivo)), true);
  }
}

console.log("\n· explicar: por qué se va (como el agente, retencion_registro::motivos)");
{
  const t0 = new Date(2026, 9, 1, 9, 0).getTime();
  const H = 3600_000;
  // Del 1 de octubre (9:00, 9:30 y 15:00) y una al día del 2 al 5; de la más reciente a la más antigua.
  const horas = [4 * 24 * H, 3 * 24 * H, 2 * 24 * H, 24 * H, 6 * H, 0.5 * H, 0].map((d) => new Date(t0 + d));
  const corto = (r: Regla) => explicar(horas, r).map((x) => x.queda ?? `${x.motivo!.tipo}:${x.motivo!.periodo}`);
  igual("3 diarias", corto({ diarias: 3, semanales: 0, mensuales: 0, anuales: 0 }), ["diarias", "diarias", "diarias", "cupo:diarias", "cupo:diarias", "repe:diarias", "repe:diarias"]);
  igual("plazo de 2 días de diarias", corto({ diarias: 0, semanales: 0, mensuales: 0, anuales: 0, plazos: { diarias: "2d" } }), ["diarias", "diarias", "plazo:diarias", "plazo:diarias", "plazo:diarias", "repe:diarias", "repe:diarias"]);
  igual("1 horaria y 1 diaria: el periodo más largo", corto({ horarias: 1, diarias: 1, semanales: 0, mensuales: 0, anuales: 0 }), ["diarias", "cupo:diarias", "cupo:diarias", "cupo:diarias", "cupo:diarias", "cupo:horarias", "repe:horarias"]);
  igual("se queda por el periodo más largo que la guarda", corto({ diarias: 7, semanales: 0, mensuales: 2, anuales: 0 }).slice(0, 2), ["mensuales", "diarias"]);
  const r: Regla = { diarias: 7, semanales: 0, mensuales: 0, anuales: 0, plazos: { horarias: "15d" } };
  igual("en palabras: cupo", textoSeVa({ tipo: "cupo", periodo: "diarias" }, new Date(2026, 9, 3, 10), r), "Era la diaria del 3 oct, pero ya había 7 diarias más recientes.");
  igual("en palabras: plazo", textoSeVa({ tipo: "plazo", periodo: "horarias" }, new Date(2026, 9, 3, 10, 20), r), "Era la horaria de las 10:00 del 3 oct, pero quedaba fuera del plazo de 15 días de las horarias.");
  igual("en palabras: repetida", textoSeVa({ tipo: "repe", periodo: "horarias" }, null, r), "Ya había otra versión más reciente en esa hora.");
  igual("en palabras: se queda", textoSeQueda("semanales", new Date(2026, 9, 3)), "Se queda: la semanal de la semana del 28 sept.");
  igual("en palabras: mensual", textoSeQueda("mensuales", new Date(2026, 9, 3)), "Se queda: la mensual de octubre de 2026.");
}

console.log("\n· Las vueltas que anota el equipo");
{
  const t = Date.UTC(2026, 9, 3, 1, 0) / 1000;
  const delAlmacen: EntradaRetencion = {
    id: "v-alm",
    hora: "2026-10-04T03:00:05+02:00",
    tipo: "retencion",
    origen: "almacen",
    por: "automatica",
    repo: "caja",
    usuario: "caja-1",
    regla: { diarias: 3, semanales: 0, mensuales: 0, anuales: 0 },
    resultado: "ok",
    antes: 9,
    quedan: 5,
    quitadas: 4,
    liberado: 2048,
    sospechosas: 2,
    grupos: [{ copia: null, refs: ["ffff0001", "aaaa0001"], quedan: 5 }],
    motivos: ["cupo:diarias", "repe:diarias"],
    versiones: [
      ["bbbb0001", t, 0, 1000, 0],
      ["bbbb0002", t - 3600, 0, null, 1],
    ],
    mas: 2,
  };
  const delEquipo: EntradaRetencion = { id: "v-eq", hora: "2026-10-05T10:00:00+02:00", tipo: "retencion", origen: "equipo", por: "orden", repo: "r1", resultado: "fallo", mensaje: "Sin red.", antes: 5 };
  const otroRepo: EntradaRetencion = { ...delEquipo, id: "v-otro", repo: "r2" };
  const otroUsuario: EntradaRetencion = { ...delAlmacen, id: "v-otro-alm", usuario: "otro" };
  const vs = vueltasDelRepo({ propias: [delEquipo, otroRepo], delAlmacen: [delAlmacen, otroUsuario], repo: "r1", enAlmacen: { usuario: "caja-1", carpeta: "caja" }, copiaDe: (id) => (id === "aaaa0001" ? "docs" : null) });
  igual("las del repositorio y las de su almacén, la más reciente primero", vs.map((v) => v.id), ["v-eq", "v-alm"]);
  const a = vs[1];
  igual(
    "las versiones quitadas, con su copia (por la versión que queda en su grupo) y su motivo",
    a.versiones.map((v) => [v.id, v.hora, v.copia, v.bytes, v.motivo && `${v.motivo.tipo}:${v.motivo.periodo}`]),
    [
      ["bbbb0001", "2026-10-03T01:00:00.000Z", "docs", 1000, "cupo:diarias"],
      ["bbbb0002", "2026-10-03T00:00:00.000Z", "docs", null, "repe:diarias"],
    ],
  );
  igual("cifras", [a.origen, a.por, a.quitadas, a.quedan, a.liberado, a.sospechosas, a.mas, a.ok], ["almacen", "automatica", 4, 5, 2048, 2, 2, true]);
  igual("una fallida sin lista", [vs[0].ok, vs[0].versiones.length, vs[0].quitadas, vs[0].mensaje], [false, 0, null, "Sin red."]);
  igual("una antigua (compactada): las que quitó, en «más»", leerVuelta({ ...delAlmacen, versiones: undefined, grupos: undefined, motivos: undefined, compactada: true, mas: undefined })!.mas, 4);
  igual("totales", totales(vs), { vueltas: 2, quitadas: 4, liberado: 2048, liberadoIncompleto: false, fallidas: 1 });
  igual("una entrada que no es de la retención", leerVuelta({ id: "x", hora: "2026-10-01T00:00:00Z", tipo: "copia" } as unknown as EntradaRetencion), null);
}

console.log("\n· prever: la próxima vuelta y lo que irá saliendo");
{
  const D = 86_400_000;
  const ahora = new Date(2026, 9, 5, 12, 0).getTime();
  const dia = (n: number, h = 10) => new Date(2026, 9, 5 + n, h, 0).toISOString();
  const cincoDias = [0, -1, -2, -3, -4].map((n) => ({ id: `d${-n}`, hora: dia(n), copia: "k1" }));
  const cadaDia10 = () => () => ["10:00"];
  const diarias3: Regla = { diarias: 3, semanales: 0, mensuales: 0, anuales: 0 };
  const p = prever({ versiones: cincoDias, regla: diarias3, ahora, dias: 7, horasDe: (k) => (k === "k1" ? cadaDia10() : null) });
  igual("al aplicarla (sin horario de retención): las dos más antiguas", [p.proxima.cuando, p.proxima.ids], [null, ["d3", "d4"]]);
  igual(
    "y cada día, con la copia nueva de las 10:00, se va la diaria más antigua",
    p.porDia.map((x) => [x.dia, x.ids]),
    [
      [claveDia(ahora + D), ["d2"]],
      [claveDia(ahora + 2 * D), ["d1"]],
      [claveDia(ahora + 3 * D), ["d0"]],
    ],
  );
  igual("copias supuestas en 7 días (la de hoy ya pasó)", p.supuestas, 6);
  const d2 = p.porVersion.get("d2")!;
  igual("la del 3 oct: hoy se queda (diaria) y se irá mañana por cupo", [d2.ahora.queda, claveDia(d2.seVa!), d2.motivoSeVa], ["diarias", claveDia(ahora + D), { tipo: "cupo", periodo: "diarias" }]);

  const sin = prever({ versiones: cincoDias, regla: diarias3, ahora, dias: 30 });
  igual("sin horario de copias: nada envejece (no llegan versiones nuevas)", [sin.porDia.length, sin.sinHorario, sin.supuestas], [0, ["k1"], 0]);

  // Con plazo: 3 días de diarias desde la más reciente; los mismos días que con 3 diarias.
  const plazo = prever({ versiones: cincoDias, regla: { diarias: 0, semanales: 0, mensuales: 0, anuales: 0, plazos: { diarias: "3d" } }, ahora, dias: 7, horasDe: cadaDia10 });
  igual("plazo de 3 días: igual que 3 diarias", [plazo.proxima.ids, plazo.porDia.map((x) => x.ids)], [["d3", "d4"], [["d2"], ["d1"], ["d0"]]]);

  // Por copia: cada una guarda las suyas (restic agrupa por equipo y carpetas).
  const dos = [...cincoDias, { id: "e0", hora: dia(0, 11), copia: "k2" }, { id: "e1", hora: dia(-1, 11), copia: "k2" }];
  const p2 = prever({ versiones: dos, regla: { diarias: 1, semanales: 0, mensuales: 0, anuales: 0 }, ahora, dias: 2 });
  igual("dos copias, 1 diaria: la más reciente de cada una se queda", p2.proxima.ids, ["e1", "d1", "d2", "d3", "d4"]);

  // Con la retención a su hora (el almacén): la próxima vuelta cuenta las copias que lleguen antes.
  const proxima = new Date(2026, 9, 7, 3, 0).getTime();
  const p3 = prever({ versiones: cincoDias, regla: diarias3, ahora, proxima, dias: 7, horasDe: cadaDia10 });
  igual("a su hora (pasado mañana a las 03:00): con la copia de mañana, una más", [p3.proxima.cuando, p3.proxima.ids, p3.porDia.map((x) => x.ids)], [proxima, ["d2", "d3", "d4"], [["d1"], ["d0"]]]);

  // La consola no conoce todas las versiones (solo las de 60 días): la más antigua conocida no es «la última de todas».
  const mismaDia = [{ id: "x1", hora: dia(0, 10), copia: "k1" }, { id: "x2", hora: dia(0, 9), copia: "k1" }];
  const r5: Regla = { diarias: 5, semanales: 0, mensuales: 0, anuales: 0 };
  igual("todas conocidas: restic guarda la más antigua", prever({ versiones: mismaDia, regla: r5, ahora, dias: 1 }).proxima.ids, []);
  igual("con más de las que se conocen: no", prever({ versiones: mismaDia, total: 40, regla: r5, ahora, dias: 1 }).proxima.ids, ["x2"]);

  // El calendario: lo que se quitó (por el día de la vuelta) y lo que dejará de entrar.
  const vuelta = leerVuelta({ id: "v", hora: new Date(ahora - 2 * D).toISOString(), tipo: "retencion", resultado: "ok", quitadas: 3 })!;
  const m = marcasPorDia([vuelta], p, ahora);
  igual("marcas del calendario", [m.get(claveDia(ahora - 2 * D)), m.get(claveDia(ahora)), m.get(claveDia(ahora + D))], [
    { quitadas: 3, vueltas: 1, fallos: 0, previstas: 0 },
    { quitadas: 0, vueltas: 0, fallos: 0, previstas: 2 },
    { quitadas: 0, vueltas: 0, fallos: 0, previstas: 1 },
  ]);
}

console.log(`\n${total - fallos}/${total} bien`);
if (fallos) process.exit(1);
