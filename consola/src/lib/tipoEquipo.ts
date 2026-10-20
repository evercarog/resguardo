// Plan 0.8.1, C3 (plan-mejoras 15c): el tipo de cada equipo, para cualquier
// entorno: **físico**, **máquina virtual** (y de qué tipo: VMware, Hyper-V,
// KVM/QEMU, Xen, VirtualBox…) o **contenedor** (LXC, Docker…).
//
// Lo detecta el agente (`entorno`, del firmware y de lo que dicen systemd y el
// primer proceso; y `datos_basicos`, que solo manda un agente que también sabe
// detectar el entorno: si lo manda sin entorno, es un equipo físico). Una
// persona lo puede corregir (`tipo_equipo`, con un agente que lo admite): lo
// guarda el equipo y lo ven igual todas sus consolas (como el nombre). Sin
// runas: se prueba en scripts/vectores-infra.ts.
import type { Equipo } from "./tipos";

export type ClaseEquipo = "fisico" | "virtual" | "contenedor" | "desconocido";

export interface TipoEquipo {
  clase: ClaseEquipo;
  /** «hyperv», «vmware», «kvm», «lxc»… si se sabe. */
  plataforma: string | null;
  /** De dónde sale: lo detectó el agente, lo corrigió una persona o no se sabe. */
  origen: "detectado" | "a_mano" | "sin_datos";
}

export const ADMITE_TIPO_EQUIPO = "tipo_equipo";
export const admiteTipoEquipo = (e: Pick<Equipo, "resumen"> | null | undefined) => !!e?.resumen?.admite?.includes(ADMITE_TIPO_EQUIPO);

/** Las plataformas de cada clase, con su nombre (las mismas que entiende el agente). */
export const PLATAFORMAS: Record<"virtual" | "contenedor", { id: string; nombre: string }[]> = {
  virtual: [
    { id: "hyperv", nombre: "Hyper-V" },
    { id: "vmware", nombre: "VMware" },
    { id: "kvm", nombre: "KVM/QEMU (Proxmox…)" },
    { id: "xen", nombre: "Xen" },
    { id: "virtualbox", nombre: "VirtualBox" },
    { id: "otra", nombre: "Otra" },
  ],
  contenedor: [
    { id: "lxc", nombre: "LXC" },
    { id: "docker", nombre: "Docker" },
    { id: "podman", nombre: "Podman" },
    { id: "wsl", nombre: "WSL" },
    { id: "otro", nombre: "Otro" },
  ],
};

export const NOMBRE_CLASE: Record<ClaseEquipo, string> = {
  fisico: "Equipo físico",
  virtual: "Máquina virtual",
  contenedor: "Contenedor",
  desconocido: "Tipo sin saber",
};

const nombrePlataforma = (clase: ClaseEquipo, p: string | null) =>
  p && (clase === "virtual" || clase === "contenedor") ? (PLATAFORMAS[clase].find((x) => x.id === p)?.nombre.replace(/ \(.*\)$/, "") ?? p) : null;

/** El tipo de un equipo: el corregido a mano, si lo hay; si no, el detectado. */
export function tipoEquipo(e: Pick<Equipo, "resumen"> | null | undefined): TipoEquipo {
  const r = e?.resumen;
  const mano = r?.datos_equipo?.tipo?.valor;
  if (mano && (mano.clase === "fisico" || mano.clase === "virtual" || mano.clase === "contenedor")) {
    return { clase: mano.clase, plataforma: typeof mano.plataforma === "string" && mano.plataforma ? mano.plataforma : null, origen: "a_mano" };
  }
  const en = r?.entorno;
  if (en?.contenedor) return { clase: "contenedor", plataforma: en.contenedor, origen: "detectado" };
  if (en?.virtual) return { clase: "virtual", plataforma: en.virtual, origen: "detectado" };
  // Un agente que manda los datos básicos ya sabe detectar el entorno: sin entorno, es físico.
  if (r?.datos_basicos && typeof r.datos_basicos === "object") return { clase: "fisico", plataforma: null, origen: "detectado" };
  return { clase: "desconocido", plataforma: null, origen: "sin_datos" };
}

/** «Máquina virtual · Hyper-V», «Contenedor · LXC», «Equipo físico». */
export function textoTipo(t: TipoEquipo): string {
  const p = nombrePlataforma(t.clase, t.plataforma);
  return p ? `${NOMBRE_CLASE[t.clase]} · ${p}` : NOMBRE_CLASE[t.clase];
}

/** Para agrupar y filtrar: «Máquina virtual (Hyper-V)», «Equipo físico». */
export function grupoTipo(e: Pick<Equipo, "resumen">): string {
  const t = tipoEquipo(e);
  const p = nombrePlataforma(t.clase, t.plataforma);
  return p ? `${NOMBRE_CLASE[t.clase]} (${p})` : NOMBRE_CLASE[t.clase];
}

/** El origen en palabras, para el globo: «detectado por el agente», «puesto a mano». */
export const TEXTO_ORIGEN: Record<TipoEquipo["origen"], string> = {
  detectado: "detectado por el agente",
  a_mano: "corregido a mano",
  sin_datos: "su agente no lo dice (actualízalo)",
};

/** El cuerpo de la orden `tipo_equipo` (vacío: volver a lo detectado). */
export function cuerpoTipo(clase: ClaseEquipo | "", plataforma: string): { clase: string; plataforma?: string } {
  if (!clase || clase === "desconocido") return { clase: "" };
  const validas = clase === "virtual" || clase === "contenedor" ? PLATAFORMAS[clase].map((x) => x.id) : [];
  return validas.includes(plataforma) ? { clase, plataforma } : { clase };
}
