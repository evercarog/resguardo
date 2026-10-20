// 15m: los espejos de un repositorio, también los que lo incluyeron y ya no, con su última
// subida. Un repositorio que sale de un espejo deja su copia en el destino sin actualizar
// (la retención del espejo solo mira lo que incluye: lo de antes se queda allí); hay que
// decirlo al cambiar el espejo, en la página del repositorio y en Estado. Sin runas (lo
// prueban scripts/vectores-espejos-repo.ts).
import { repoEnAlmacen } from "./cadenas";
import { nombrePorDefecto, trabajoIncluye, trabajosDelAlmacen, trabajosDelEquipo } from "./espejoTrabajos";
import { nombreZonaPorDefecto, zonasDe } from "./destinos";
import type { Equipo, RepositorioResumen, TrabajoEspejo, TrabajoEspejoResumen } from "./tipos";

const PRINCIPAL = "principal";
const DIA = 86_400_000;

/** Un espejo de un repositorio: lo incluye ahora o lo incluyó (y cuándo subió bien por última vez). */
export interface EspejoDeRepo {
  trabajo: TrabajoEspejoResumen;
  /** Quién lo hace (el almacén donde está o el propio equipo). */
  hace: Equipo;
  /** ¿Lo incluye ahora? */
  incluye: boolean;
  /** La última subida buena de este repositorio en él (RFC 3339), si se sabe. */
  ultima: string | null;
}

/**
 * Los espejos que incluyen el repositorio (los del almacén donde está y los del propio equipo)
 * y los que lo incluyeron (lo dicen por su `ultima_por_repo`). Primero los que lo incluyen.
 */
export function espejosDelRepo(equipo: Equipo, repo: RepositorioResumen, equipos: Equipo[]): EspejoDeRepo[] {
  const out: EspejoDeRepo[] = [];
  const en = repoEnAlmacen(equipo, repo, equipos);
  if (en) {
    const almacen = equipos.find((e) => e.id === en.almacen.id) ?? en.almacen;
    for (const t of trabajosDelAlmacen(almacen)) {
      const incluye = trabajoIncluye(t, en.nombre, en.zona);
      const ultima = t.ultima_por_repo?.[en.nombre] ?? null;
      if (incluye || ultima) out.push({ trabajo: t, hace: almacen, incluye, ultima: ultima ?? (incluye && !t.ultima_por_repo ? (t.bien ?? null) : null) });
    }
  }
  for (const t of trabajosDelEquipo(equipo)) {
    const incluye = trabajoIncluye(t, repo.id, PRINCIPAL);
    const ultima = t.ultima_por_repo?.[repo.id] ?? null;
    if (incluye || ultima) out.push({ trabajo: t, hace: equipo, incluye, ultima: ultima ?? (incluye && !t.ultima_por_repo ? (t.bien ?? null) : null) });
  }
  return out.sort((a, b) => Number(b.incluye) - Number(a.incluye) || a.trabajo.orden - b.trabajo.orden);
}

/** Lo que quedó en el destino de un espejo que ya no incluye el repositorio (sin actualizarse). */
export const sinActualizar = (l: EspejoDeRepo[]) => l.filter((x) => !x.incluye && !!x.ultima);

/** El nombre de un espejo y el de su destino («Dropbox Oficina», «otro disco»). */
export const nombreEspejo = (t: Pick<TrabajoEspejo, "nombre" | "adonde">) => t.nombre || nombrePorDefecto(t.adonde);
export function destinoEspejo(t: Pick<TrabajoEspejo, "adonde">, hace?: Equipo | null): string {
  if (t.adonde.tipo === "nube") return t.adonde.nube ?? "una nube";
  if (t.adonde.tipo === "zona") {
    const z = hace ? zonasDe(hace).find((x) => x.id === t.adonde.carpeta) : undefined;
    return z ? nombreZonaPorDefecto(z) : "otra zona del almacén";
  }
  return "otro disco";
}

/** Días enteros desde `iso`. */
export const diasDesde = (iso: string, ahora: number) => Math.max(0, Math.floor((ahora - Date.parse(iso)) / DIA));

/** «Tiene una copia en «Dropbox Oficina» que ya no se actualiza desde hace 12 días (salió del espejo «…»).» */
export function textoSinActualizar(x: EspejoDeRepo, ahora: number): string {
  const n = x.ultima ? diasDesde(x.ultima, ahora) : null;
  const desde = n == null ? "" : n === 0 ? " desde hoy" : n === 1 ? " desde hace 1 día" : ` desde hace ${n} días`;
  return `Tiene una copia en «${destinoEspejo(x.trabajo, x.hace)}» que ya no se actualiza${desde} (ya no entra en el espejo «${nombreEspejo(x.trabajo)}»). Lo subido se queda allí: la retención del espejo no lo quita.`;
}

/**
 * 15m (1): al cambiar un espejo, los repositorios que dejan de estar (de entre `opciones`, los
 * valores tal como los ve el espejo: `<usuario>/<repo>` en el almacén, el id en el equipo).
 */
export function salenDelEspejo(antes: Pick<TrabajoEspejo, "que" | "zona" | "quien">, despues: Pick<TrabajoEspejo, "que" | "zona" | "quien">, opciones: string[]): string[] {
  // Los repositorios son los de la zona de antes: si cambia de zona, salen todos.
  const zona = antes.zona || PRINCIPAL;
  return opciones.filter((r) => trabajoIncluye(antes, r, zona) && !trabajoIncluye(despues, r, zona));
}

/** Lo que pasa con lo ya subido de los que salen, en una frase (según la retención del espejo). */
export function textoSalen(nombres: string[], destino: string): string {
  if (!nombres.length) return "";
  const lista = nombres.length === 1 ? `«${nombres[0]}» deja` : `${nombres.slice(0, -1).map((n) => `«${n}»`).join(", ")} y «${nombres.at(-1)}» dejan`;
  return `${lista} de estar en este espejo. Lo ya subido a ${destino} se queda allí, sin actualizarse: la retención del espejo no lo quita (solo mira lo que incluye). Si ya no lo quieres allí, bórralo a mano.`;
}
