// Pruebas de la rama `ia/082-consola` (plan 0.8.2): B4 (la copia derivada «de cada día» ya
// quitada no se enseña como viva), C2 (grupos de «No copiar» en varios equipos y en las
// plantillas) y C3 (la copia que se cortó porque el equipo dejó de responder).
// `npm run test:vectores`.
import type { CopiaConfig, Equipo, RepoInforme, RepositorioResumen, TareaEnMarcha } from "../src/lib/tipos";
import { externaViva } from "../src/lib/repo";
import { gruposAlDia, gruposEnCopias } from "../src/lib/exclusionesCopia";
import { planPlantilla, type Admite } from "../src/lib/configEnvio";
import type { Plantilla } from "../src/lib/plantillas";
import { aLas, cortadaDeCopia, cortadasVigentes, equipoDesconectado, sigueCortada, textoCortada } from "../src/lib/interrumpidas";

let fallos = 0;
let total = 0;
function igual(nombre: string, obtenido: unknown, esperado: unknown) {
  total++;
  const ok = JSON.stringify(obtenido) === JSON.stringify(esperado);
  if (!ok) fallos++;
  console.log(`${ok ? "ok  " : "MAL "} ${nombre}${ok ? "" : `\n       obtenido: ${JSON.stringify(obtenido)}\n       esperado: ${JSON.stringify(esperado)}`}`);
}

console.log("\n· B4: la copia derivada «de cada día» ya quitada");
{
  const inf = { id: "r1", externa: { ultima: "2026-10-06T21:00:00Z", resultado: "fallo", mensaje_corto: "no aparece" } } as unknown as RepoInforme;
  const con = { id: "r1", externa: { destino: "Disco E", hora: "21:00" } } as unknown as RepositorioResumen;
  const sin = { id: "r1", externa: null } as unknown as RepositorioResumen;
  igual("con la copia: su última vuelta", externaViva(con, inf)?.resultado, "fallo");
  igual("quitada: nada (aunque el informe aún la traiga)", externaViva(sin, inf), null);
  igual("sin informe: nada", externaViva(con, null), null);
}

console.log("\n· C2: grupos de «No copiar» en varias copias");
{
  const musica = { id: "musica", nombre: "Música", reglas: ["*.mp3", "*.flac"] };
  const video = { id: "video", nombre: "Vídeo", reglas: ["*.mp4"] };
  const k = (extra: Partial<CopiaConfig> = {}) => ({ nombre: "Docs", exclusiones: ["*.tmp"], ...extra }) as CopiaConfig;
  const a = k();
  const b = k({ exclusiones: ["*.tmp", "*.mp3", "*.flac"], grupos_exclusiones: [{ id: "musica", nombre: "Música", reglas: ["*.mp3", "*.flac"] }] });
  const c = k({ exclusiones: ["*.mp3"], grupos_exclusiones: [{ id: "musica", nombre: "Música", reglas: ["*.mp3"] }] });
  const cambian = gruposEnCopias([a, b, c], [musica]);
  igual("poner: cambian la que no lo tenía y la de reglas de antes", cambian.map((x) => x === a ? "a" : x === b ? "b" : "c"), ["a", "c"]);
  igual("la nueva lleva sus reglas y el grupo", [a.exclusiones, a.grupos_exclusiones?.map((g) => g.id)], [["*.tmp", "*.mp3", "*.flac"], ["musica"]]);
  igual("la de antes, al día", c.exclusiones, ["*.mp3", "*.flac"]);
  igual("repetir no cambia nada", gruposEnCopias([a, b, c], [musica]).length, 0);
  const q = gruposEnCopias([a, b], [musica, video], true);
  igual("quitar: salen las suyas, las sueltas se quedan", [q.length, a.exclusiones, a.grupos_exclusiones], [2, ["*.tmp"], []]);
  igual("quitar un grupo que no usa ninguna: nada", gruposEnCopias([a], [video], true).length, 0);

  // La plantilla (también la de una etiqueta) con las reglas de ahora de cada grupo.
  const conViejas = k({ exclusiones: ["*.mp3", "*.bak"], grupos_exclusiones: [{ id: "musica", nombre: "Música", reglas: ["*.mp3"] }, { id: "borrado", nombre: "Ya no existe", reglas: ["*.bak"] }] });
  igual("plantilla: pone al día los que existen", gruposAlDia(conViejas, [musica]), ["Música"]);
  igual("… y deja los que ya no existen", [conViejas.exclusiones.sort(), conViejas.grupos_exclusiones?.map((g) => g.id).sort()], [["*.bak", "*.flac", "*.mp3"], ["borrado", "musica"]]);
  const p: Plantilla = {
    v: 1,
    id: "pla-1",
    nombre: "Puestos",
    copia: { carpetas: ["C:\\Datos"], exclusiones: ["*.mp3"], horario: { reglas: [{ tipo: "dias", dias: [1, 2, 3, 4, 5], horas: ["13:00"] }] } as unknown as CopiaConfig["horario"], gancho: [], grupos_exclusiones: [{ id: "musica", nombre: "Música", reglas: ["*.mp3"] }] },
    creada: "2026-10-01T00:00:00Z",
  };
  const admite = { reglas: true, ganchos: true, grupos: true } as unknown as Admite;
  const plan = planPlantilla({ v: 1, copias: [], repositorios: [{ id: "r1", nombre: "Datos", destino: "d" }], destinos: [] } as never, p, admite, "copia-1", [musica]);
  igual("plantilla por etiqueta: el equipo nuevo recibe el grupo con sus reglas de ahora", plan.ok ? plan.copia.exclusiones : plan.motivo, ["*.mp3", "*.flac"]);
}

console.log("\n· C3: la copia cortada porque el equipo dejó de responder");
{
  const ahora = Date.parse("2026-10-21T15:00:00");
  const desde = new Date("2026-10-21T12:40:00").toISOString();
  const empezo = new Date("2026-10-21T12:10:00").toISOString();
  const tarea = { tipo: "copia", fase: "subiendo", repo: "r1", copia: "k1", nombre: "Documentos", empezo } as TareaEnMarcha;
  const eq = (ultima: string | null, conectado = false, informe: string | null = null) =>
    ({ id: "e1", nombre: "PC-02", conectado, ultimo_contacto: desde, resumen: { copias: [{ id: "k1", repo: "r1", nombre: "Documentos", ultima: ultima ? { cuando: ultima, estado: "ok" } : null }] }, ultimo_informe: informe ? { recibido: informe, datos: {} } : null }) as unknown as Equipo;
  const cortadas = { e1: { desde, tareas: [tarea] } };
  igual("sin vuelta más nueva: sigue interrumpida", sigueCortada(eq(new Date("2026-10-20T13:00:00").toISOString()), tarea, desde), true);
  igual("el equipo dice que terminó: manda el equipo", sigueCortada(eq(new Date("2026-10-21T12:10:00").toISOString()), tarea, desde), false);
  igual("la copia ya no existe: no se enseña", cortadasVigentes(cortadas, [{ ...eq(null), resumen: { copias: [] } } as unknown as Equipo]).length, 0);
  igual("texto, desconectado", textoCortada({ equipo: eq(null), desde }, ahora), "Interrumpida: PC-02 se desconectó a las 12:40");
  igual("texto, ya volvió", textoCortada({ equipo: eq(null, true), desde }, ahora), "Interrumpida: PC-02 dejó de responder a las 12:40");
  igual("la de la copia", cortadaDeCopia(cortadas, eq(null), "k1")?.tarea.copia, "k1");
  const verif = { tipo: "verificar", fase: "en_marcha", repo: "r1" } as TareaEnMarcha;
  igual("otra tarea: hasta que el equipo vuelve a informar", [sigueCortada(eq(null, true, new Date("2026-10-21T12:41:00").toISOString()), verif, desde), sigueCortada(eq(null, true, new Date("2026-10-21T13:00:00").toISOString()), verif, desde)], [true, false]);
  igual("ayer", aLas(new Date("2026-10-20T18:05:00").toISOString(), ahora), "ayer a las 18:05");
  igual("desconectado desde", equipoDesconectado({ nombre: "PC-02", conectado: false, ultimo_contacto: new Date("2026-10-20T18:05:00").toISOString() }, ahora), "PC-02 desconectado desde ayer a las 18:05");
  igual("hoy", equipoDesconectado({ nombre: "PC-02", conectado: false, ultimo_contacto: desde }, ahora), "PC-02 desconectado desde las 12:40");
  igual("conectado: nada", equipoDesconectado({ nombre: "PC-02", conectado: true, ultimo_contacto: desde }, ahora), null);
}

console.log(`\n${total - fallos} de ${total} bien`);
if (fallos) process.exit(1);
