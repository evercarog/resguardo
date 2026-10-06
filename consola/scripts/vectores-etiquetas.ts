// Pruebas de las etiquetas con color elegido y que sirven para algo (tarea 6,
// src/lib/etiquetasGrupos.ts y src/lib/configEnvio.ts). `npm run test:vectores`.
import type { AjusteEtiqueta, Configuracion, Equipo } from "../src/lib/tipos";
import type { Plantilla } from "../src/lib/plantillas";
import { ajusteDe, colorDe, colorPorNombre, cuerpoAjuste, etiquetasDe, gruposPorEtiqueta, N_COLORES, NOMBRES_COLOR, pasaFiltro, plantillasPropuestas } from "../src/lib/etiquetasGrupos";
import { admiteDe, configParaEnviar, paraEditar, planPlantilla, type Admite } from "../src/lib/configEnvio";

let fallos = 0;
let total = 0;
function igual(nombre: string, obtenido: unknown, esperado: unknown) {
  total++;
  const ok = JSON.stringify(obtenido) === JSON.stringify(esperado);
  if (!ok) fallos++;
  console.log(`${ok ? "ok  " : "MAL "} ${nombre}${ok ? "" : `\n       obtenido: ${JSON.stringify(obtenido)}\n       esperado: ${JSON.stringify(esperado)}`}`);
}

const base = { so: "Windows 11", version_agente: "0.7.21", box_pub: "", sign_pub: "", sal_equipo: "", etiqueta: null, modo: "gestionado" as const, confirmado: true, conectado: true, ultimo_contacto: null, estado_servicio: "en_marcha" as const, siguiente_seq: 1, rol: "agente" as const };
const equipo = (id: string, etiquetas: string[], extra: Partial<Equipo> = {}): Equipo => ({ ...base, id, nombre: id.toUpperCase(), etiquetas, resumen: { copias: [] }, ...extra }) as Equipo;
const ids = (es: { id: string }[]) => es.map((e) => e.id);

console.log("\n· Colores (lib/etiquetasGrupos.ts)");
{
  // El de siempre: el mismo hash que antes (h·31 + código, sobre el nombre en minúsculas).
  const viejo = (n: string) => {
    let h = 0;
    for (const c of n.trim().toLowerCase()) h = (h * 31 + c.codePointAt(0)!) >>> 0;
    return h % 7;
  };
  igual("sin elegir, el mismo color que antes", ["Contabilidad", "Servidores", "Sede norte", "Recepción"].map(colorPorNombre), ["Contabilidad", "Servidores", "Sede norte", "Recepción"].map(viejo));
  igual("sin distinguir mayúsculas ni espacios", colorPorNombre(" SERVIDORES "), colorPorNombre("servidores"));
  const ajustes: AjusteEtiqueta[] = [{ nombre: "Servidores", color: 4 }, { nombre: "Raro", color: 9 }, { nombre: "Medio", color: 2.5 }];
  igual("elegido, con otras mayúsculas", colorDe("servidores", ajustes), 4);
  igual("fuera de la paleta: el de su nombre", [colorDe("Raro", ajustes), colorDe("Medio", ajustes)], [colorPorNombre("Raro"), colorPorNombre("Medio")]);
  igual("sin ajustes: el de su nombre", colorDe("Contabilidad"), colorPorNombre("Contabilidad"));
  igual("cada color tiene nombre", NOMBRES_COLOR.length, N_COLORES);
  igual("ajusteDe sin distinguir mayúsculas", ajusteDe("SERVIDORES", ajustes)?.color, 4);
}

console.log("\n· Grupos por etiqueta");
{
  const es = [equipo("a", ["Servidores"]), equipo("b", ["Sede norte", "contabilidad"]), equipo("c", []), equipo("d", ["Contabilidad"])];
  igual("etiquetas con cuántos (la primera forma escrita manda)", etiquetasDe(es), [
    { nombre: "contabilidad", n: 2 },
    { nombre: "Sede norte", n: 1 },
    { nombre: "Servidores", n: 1 },
  ]);
  const g = gruposPorEtiqueta(es);
  igual(
    "alfabético, uno con dos etiquetas en los dos, «sin etiqueta» al final",
    g.map((x) => [x.etiqueta, ids(x.equipos)]),
    [
      ["contabilidad", ["b", "d"]],
      ["Sede norte", ["b"]],
      ["Servidores", ["a"]],
      [null, ["c"]],
    ],
  );
  igual("sin equipos sin etiqueta, sin ese grupo", gruposPorEtiqueta([es[0]]).map((x) => x.etiqueta), ["Servidores"]);
  igual("filtro sin distinguir mayúsculas", ids(es.filter((e) => pasaFiltro(e, "CONTABILIDAD"))), ["b", "d"]);
}

console.log("\n· Plantilla propuesta a un equipo nuevo");
{
  const ajustes: AjusteEtiqueta[] = [{ nombre: "Servidores", plantilla: "pla-srv" }, { nombre: "Sede norte", plantilla: "pla-srv" }, { nombre: "Contabilidad", plantilla: "pla-conta" }, { nombre: "Gerencia", color: 1 }];
  igual("una por plantilla, con sus etiquetas", plantillasPropuestas(equipo("a", ["Servidores", "Sede norte", "Gerencia"]), ajustes), [{ plantilla: "pla-srv", etiquetas: ["Servidores", "Sede norte"] }]);
  igual("con copias ya: nada", plantillasPropuestas(equipo("a", ["Servidores"], { resumen: { copias: [{ id: "k", nombre: "Docs", repo: "r", activa: true } as never] } }), ajustes), []);
  igual("sin confirmar o trasladado: nada", [plantillasPropuestas(equipo("a", ["Servidores"], { confirmado: false }), ajustes), plantillasPropuestas(equipo("a", ["Servidores"], { modo: "trasladado" }), ajustes)], [[], []]);
  igual("sin ajustes: nada", plantillasPropuestas(equipo("a", ["Servidores"]), undefined), []);
}

console.log("\n· Lo que se envía al guardar los ajustes");
{
  igual("todo vacío: vuelve a lo de siempre", cuerpoAjuste(" Servidores ", null, "", { importancia: null, canales: [] }), { nombre: "Servidores", color: null, plantilla: null, avisos: null });
  igual("con avisos: solo lo que hay", cuerpoAjuste("S", 3, "pla-1", { importancia: "critico", canales: [] }), { nombre: "S", color: 3, plantilla: "pla-1", avisos: { importancia: "critico" } });
  igual("color 0 (azul) no es «sin color»", cuerpoAjuste("S", 0, null, null).color, 0);
}

console.log("\n· Aplicar una plantilla a varios (lib/configEnvio.ts)");
const TODO: Admite = { reglas: true, ganchos: true, soloCambios: true, verif: true, verifHorario: true, escritorio: true };
const plantilla: Plantilla = {
  v: 1,
  id: "pla-srv",
  nombre: "Copia de servidores",
  copia: { carpetas: ["D:\\Datos"], exclusiones: ["*.tmp"], horario: { dias: [1, 2, 3, 4, 5], horas: ["22:00"] }, solo_si_cambios: true, gancho: [] },
  creada: "2026-10-01T00:00:00Z",
};
const cfg = (extra: Partial<Configuracion> = {}): Configuracion => ({
  v: 1,
  copias: [{ id: "k1", nombre: "Documentos", repo: "r1", carpetas: ["C:\\Docs"], exclusiones: [], horario: { dias: [1], horas: ["13:00"] }, activa: true }],
  repositorios: [
    { id: "r0", nombre: "Importado", destino: "d", solo_lectura: true },
    { id: "r1", nombre: "Almacén", destino: "d" },
    { id: "r2", nombre: "Disco USB", destino: "u" },
  ],
  destinos: [],
  ...extra,
});
{
  const p = planPlantilla(paraEditar(cfg()), plantilla, TODO, "copia-nueva");
  igual("copia nueva en el primer repositorio que admite escritura (y se puede elegir otro)", p.ok ? [p.copia.repo, p.repos.map((r) => r.id), p.copia.nombre, p.copia.carpetas] : p, ["r1", ["r1", "r2"], "Copia de servidores", ["D:\\Datos"]]);
  igual("sin repositorio: se salta", planPlantilla(cfg({ repositorios: [] }), plantilla, TODO, "x").ok, false);
  const ya = planPlantilla(cfg({ copias: [{ ...cfg().copias[0], nombre: " copia de SERVIDORES " }] }), plantilla, TODO, "x");
  igual("ya tiene una copia con ese nombre: se salta y lo dice", ya.ok ? "" : ya.motivo, "Ya tiene una copia «Copia de servidores»: se salta.");
  const conGancho: Plantilla = { ...plantilla, copia: { ...plantilla.copia, gancho: [{ tipo: "carpeta_reciente", carpeta: "D:\\Volcados", horas: 24 } as never] } };
  igual("agente anterior a los ganchos: se salta", planPlantilla(cfg(), conGancho, { ...TODO, ganchos: false }, "x").ok, false);
  const sinHorario: Plantilla = { ...plantilla, copia: { ...plantilla.copia, horario: { dias: [], horas: [] } } };
  igual("horario vacío: se salta", planPlantilla(cfg(), sinHorario, TODO, "x").ok, false);
  // Lo que se envía: la copia nueva junto a las que ya tenía, sin tocarlas.
  if (p.ok) {
    const c = paraEditar(cfg());
    const enviada = configParaEnviar({ ...c, copias: [...c.copias, p.copia] }, TODO);
    igual("las que tenía siguen igual y la nueva al final", enviada.copias.map((k) => [k.id, k.repo, k.activa]), [["k1", "r1", true], ["copia-nueva", "r1", true]]);
    igual("ganchos vacíos → null; «solo si hay cambios» a un agente que lo entiende", [enviada.copias[1].gancho, enviada.copias[1].solo_si_cambios], [null, true]);
    const vieja = configParaEnviar({ ...c, copias: [...c.copias, p.copia] }, { ...TODO, soloCambios: false, ganchos: false });
    igual("a un agente anterior, sin los campos que no entiende", ["solo_si_cambios" in vieja.copias[1], vieja.copias[1].gancho], [false, null]);
    igual("horas ordenadas y sin repetir", configParaEnviar({ ...c, copias: [{ ...c.copias[0], horario: { dias: [3, 1, 1], horas: ["13:00", "08:00", "13:00"] } }] }, TODO).copias[0].horario, { dias: [1, 3], horas: ["08:00", "13:00"] });
  }
  igual("admiteDe por la versión del agente", [admiteDe(equipo("a", [], { version_agente: "0.7.1" })).ganchos, admiteDe(equipo("a", [], { version_agente: "0.7.21" })).reglas], [false, true]);
}

console.log(`\n${total - fallos} de ${total} comprobaciones correctas.`);
if (fallos) process.exit(1);
