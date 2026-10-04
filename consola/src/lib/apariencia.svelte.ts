// Apariencia (tema, acento y densidad), como en la app. Es una preferencia de
// este navegador: se guarda en localStorage (no es nada secreto).
export type Tema = "sistema" | "light" | "dark" | "black";
export type Acento = "teal" | "blue" | "indigo" | "violet" | "rose" | "amber" | "graphite";
/** «Cómoda» (por defecto) o «Compacta»: menos aire en listas y tablas (docs/diseno.md §5). */
export type Densidad = "comoda" | "compacta";

export const ACENTOS: readonly Acento[] = ["teal", "blue", "indigo", "violet", "rose", "amber", "graphite"];

export const apariencia = $state({ tema: "sistema" as Tema, acento: "teal" as Acento, densidad: "comoda" as Densidad });

function leer(k: string) {
  try {
    return localStorage.getItem(k);
  } catch {
    return null;
  }
}

export function aplicarApariencia() {
  const r = document.documentElement;
  if (apariencia.tema === "sistema") r.removeAttribute("data-theme");
  else r.setAttribute("data-theme", apariencia.tema);
  if (apariencia.acento === "teal") r.removeAttribute("data-accent");
  else r.setAttribute("data-accent", apariencia.acento);
  if (apariencia.densidad === "compacta") r.setAttribute("data-densidad", "compacta");
  else r.removeAttribute("data-densidad");
  // La barra del sistema (PWA instalada, móvil) del color del fondo: el del tema
  // elegido o, en automático, el del sistema (cada <meta> con su `media`).
  const fijo = { light: "#ffffff", dark: "#0f0f11", black: "#000000", sistema: null }[apariencia.tema];
  for (const m of document.querySelectorAll<HTMLMetaElement>('meta[name="theme-color"]')) m.content = fijo ?? (m.media.includes("dark") ? "#0f0f11" : "#ffffff");
  try {
    localStorage.setItem("resguardo.tema", apariencia.tema);
    localStorage.setItem("resguardo.acento", apariencia.acento);
    localStorage.setItem("resguardo.densidad", apariencia.densidad);
  } catch {
    /* sin almacenamiento */
  }
}

export function iniciarApariencia() {
  apariencia.tema = (leer("resguardo.tema") as Tema) ?? "sistema";
  apariencia.acento = (leer("resguardo.acento") as Acento) ?? "teal";
  apariencia.densidad = leer("resguardo.densidad") === "compacta" ? "compacta" : "comoda";
  aplicarApariencia();
}
