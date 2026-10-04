// «Novedades»: tras una actualización se muestran los cambios desde la versión
// que se usó la última vez. Se recuerda por usuario de Windows (localStorage
// del webview vive en su perfil).
import { CHANGELOG, compareVersions, releasesBetween, type Release } from "$lib/changelog";

const KEY = "resguardo.lastVersion";
/** Versión que está en marcha (las entradas posteriores aún no están publicadas). */
let installed = "";

export const news = $state<{ open: boolean; releases: Release[]; since: string | null }>({ open: false, releases: [], since: null });

/**
 * Al abrir la app: si la versión subió desde la última vez, abre «Novedades».
 * La primera vez (instalación nueva) no se muestra nada.
 */
export function checkNews(current: string) {
  if (!current) return;
  installed = current;
  let last: string | null = null;
  try {
    last = localStorage.getItem(KEY);
    localStorage.setItem(KEY, current);
  } catch {
    return;
  }
  if (!last || compareVersions(current, last) <= 0) return;
  const releases = releasesBetween(last, current);
  if (releases.length) Object.assign(news, { open: true, releases, since: last });
}

/** Reabrir desde Ajustes: las últimas entradas. */
export function openNews() {
  const published = installed ? CHANGELOG.filter((r) => compareVersions(r.version, installed) <= 0) : CHANGELOG;
  Object.assign(news, { open: true, releases: published.slice(0, 6), since: null });
}
