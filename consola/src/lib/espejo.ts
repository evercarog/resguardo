// El espejo del almacén por destino (docs/espejo.md): horario, «después de
// cada copia nueva» y lo que se manda al equipo. Sin dependencias de Svelte,
// para las pruebas (scripts/vectores-espejo.ts).
import { horarioEnFrase } from "./formato";
import { usuarioEnAlmacen } from "./direccion";
import type { Equipo, Horario } from "./tipos";

/** El agente entiende el espejo por destino (horario, selección, retención y verificación). */
export const ADMITE_FLEXIBLE = "espejo_flexible";

export const admiteEspejoFlexible = (e: Pick<Equipo, "resumen"> | null | undefined) => !!e?.resumen?.admite?.includes(ADMITE_FLEXIBLE);

/** Un destino del espejo como lo da el resumen del equipo. */
export interface DestinoEspejoResumen {
  tipo: "carpeta" | "nube";
  carpeta?: string | null;
  nube?: string | null;
  ultima?: string | null;
  resultado?: string | null;
  /** Su horario propio (sin él, cada día a `espejo.hora`). */
  horario?: Horario | null;
  /** También después de cada copia nueva. */
  tras_copia?: boolean | null;
  /** La próxima vuelta por horario. */
  proxima?: string | null;
  /** §3f: solo estos repositorios (`<usuario>` o `<usuario>/<repo>`); sin ellos, todos. */
  repos?: string[] | null;
  /** §3f: los repositorios que había al elegir la selección (los demás son nuevos). */
  vistos?: string[] | null;
  /** §3d: % de lo que hay en el destino que se comprueba cada día. */
  verificar_pct?: number | null;
  /** §3d: la última comprobación del destino. */
  verificacion?: { ultima: string; archivos: number; mal: number } | null;
  /** §3d: archivos dañados del almacén que no se copiaron en la última vuelta. */
  danados_origen?: number | null;
}

/** Lo que se manda de un destino en `guarda_copias.espejo.destinos` (sin sus resultados). */
export interface DestinoEspejoOrden {
  tipo: "carpeta" | "nube";
  carpeta: string;
  nube?: string;
  horario?: Horario;
  tras_copia?: boolean;
  repos?: string[];
  vistos?: string[];
  verificar_pct?: number;
}

/** Un destino del resumen en la forma de la orden: lo que ya tiene, para reenviarlo sin cambios. */
export function destinoParaOrden(d: DestinoEspejoResumen): DestinoEspejoOrden {
  const o: DestinoEspejoOrden = d.tipo === "nube" ? { tipo: "nube", nube: d.nube ?? "", carpeta: d.carpeta ?? "" } : { tipo: "carpeta", carpeta: d.carpeta ?? "" };
  if (d.horario && (d.horario.reglas?.length || d.horario.horas?.length)) o.horario = d.horario;
  if (d.tras_copia) o.tras_copia = true;
  if (Array.isArray(d.repos)) {
    o.repos = [...d.repos];
    o.vistos = [...(d.vistos ?? [])];
  }
  if (typeof d.verificar_pct === "number") o.verificar_pct = d.verificar_pct;
  return o;
}

/** §3d: «Comprueba el 5 % cada día · 120 archivos bien» (o null si no comprueba nada). */
export function textoVerificacion(d: Pick<DestinoEspejoResumen, "verificar_pct" | "verificacion">): string | null {
  const pct = d.verificar_pct ?? 0;
  if (!pct) return null;
  const base = pct === 100 ? "Lo comprueba todo cada día" : `Comprueba el ${pct} % cada día`;
  const v = d.verificacion;
  if (!v) return base;
  return `${base} · ${v.archivos} ${v.archivos === 1 ? "archivo" : "archivos"} la última vez${v.mal ? `, ${v.mal} mal` : ", bien"}`;
}

/** Los repositorios que guarda un almacén, con el nombre que usa el espejo: `<usuario>` o `<usuario>/<repo>`. */
export function nombresRepos(repositorios: { usuario: string; repos: string[] }[] | null | undefined): string[] {
  const v = (repositorios ?? []).flatMap((u) => u.repos.map((r) => (r === "." || r === "" ? u.usuario : `${u.usuario}/${r}`)));
  return [...new Set(v.filter((n) => n.split("/").length <= 2))].sort();
}

/** Repositorios nuevos del almacén que no entran en un destino con selección (ni se vieron al elegirla). */
export function nuevosEn(d: Pick<DestinoEspejoResumen, "repos" | "vistos">, todos: string[]): string[] {
  if (!Array.isArray(d.repos)) return [];
  const conocidos = new Set([...(d.vistos ?? []), ...d.repos]);
  return todos.filter((r) => !conocidos.has(r));
}

/** «Todos los repositorios» o «2 repositorios: a, b». */
export function textoRepos(d: Pick<DestinoEspejoResumen, "repos">, nombre: (r: string) => string = (r) => r): string {
  if (!Array.isArray(d.repos)) return "Todos los repositorios";
  const n = d.repos.map(nombre);
  return n.length === 1 ? `Solo ${n[0]}` : `${n.length} repositorios: ${n.join(", ")}`;
}

/** El nombre de un repositorio en el espejo de su almacén (`<usuario>/<repo>`), o null. */
export function nombreEnAlmacen(donde: string | null | undefined, repo: { id: string; ruta?: string | null }): string | null {
  const u = usuarioEnAlmacen(donde);
  if (!u) return null;
  const r = repo.ruta || repo.id;
  return r === "." || !r ? u : `${u}/${r}`;
}

/** El espejo de un almacén visto desde uno de sus repositorios: solo los destinos a los que va (o null). */
export function espejoDelRepo<E extends { destinos?: Pick<DestinoEspejoResumen, "repos">[] | null }>(espejo: E | null, nombre: string | null): E | null {
  if (!espejo) return null;
  if (!espejo.destinos?.length) return espejo;
  const destinos = espejo.destinos.filter((d) => !Array.isArray(d.repos) || (!!nombre && d.repos.includes(nombre)));
  return destinos.length ? { ...espejo, destinos } : null;
}

/** El destino con estos repositorios añadidos a su selección (y lo de ahora como visto). */
export function conRepos(d: DestinoEspejoOrden, anadir: string[], todos: string[]): DestinoEspejoOrden {
  if (!d.repos) return d;
  return { ...d, repos: [...new Set([...d.repos, ...anadir])], vistos: [...todos] };
}

/** El horario «cada día a esa hora» de antes, como horario de las copias. */
export const horarioDiario = (hora: string): Horario => ({ dias: [1, 2, 3, 4, 5, 6, 7], horas: [hora] });

/** «Cada día a las 02:00», «Cada hora de 8:00 a 18:00, … y después de cada copia nueva». */
export function cuandoEspejo(d: Pick<DestinoEspejoResumen, "horario" | "tras_copia">, horaGlobal: string): string {
  const h = d.horario && (d.horario.reglas?.length || d.horario.horas?.length) ? horarioEnFrase(d.horario) : `Cada día a las ${horaGlobal}`;
  return d.tras_copia ? `${h} y después de cada copia nueva` : h;
}

const tieneHorario = (d: Pick<DestinoEspejoResumen, "horario">) => !!(d.horario && (d.horario.reglas?.length || d.horario.horas?.length));

/** Corto, para el mapa y las flechas: «cada noche a las 02:00» (como antes) o «con su horario y tras cada copia». */
export function cuandoCorto(d: Pick<DestinoEspejoResumen, "horario" | "tras_copia">, horaGlobal: string): string {
  const base = tieneHorario(d) ? "con su horario" : `cada noche a las ${horaGlobal}`;
  return d.tras_copia ? `${base} y tras cada copia` : base;
}

/** Lo mismo para todo el espejo: el de sus destinos si todos coinciden; si no, «con el horario de cada destino». */
export function cuandoCortoEspejo(e: { hora: string; destinos?: Pick<DestinoEspejoResumen, "horario" | "tras_copia">[] | null }): string {
  const textos = [...new Set((e.destinos?.length ? e.destinos : [{}]).map((d) => cuandoCorto(d, e.hora)))];
  return textos.length === 1 ? textos[0] : "con el horario de cada destino";
}

/** La hora que se manda en `espejo.hora` (para consolas anteriores): la primera hora del primer destino. */
export function horaParaConsolasAnteriores(destinos: DestinoEspejoOrden[], porDefecto: string): string {
  for (const d of destinos) {
    const h = d.horario?.horas?.[0] ?? d.horario?.reglas?.map((r) => ("horas" in r ? r.horas[0] : "desde" in r ? r.desde : r.hora)).find(Boolean);
    if (h && /^([01]\d|2[0-3]):[0-5]\d$/.test(h)) return h;
  }
  return porDefecto;
}
