// 0.8.3 (16g): en Infraestructura, los equipos agrupados por lo que se espera de cada uno.
// Un equipo físico de usuario no vive en ningún servidor: «Es su propio equipo», sin pedir
// nada. Solo las máquinas virtuales y los contenedores piden «¿En qué servidor físico
// está?», con el servidor que encaja propuesto. Sin runas: se prueba en
// scripts/vectores-083-servidor.ts.
import type { Equipo } from "./tipos";
import type { Infraestructura, ServidorFisico } from "./infra";
import { tipoEquipo } from "./tipoEquipo";

/** «virtual»: máquina virtual o contenedor (pide su servidor); «servidor»: un equipo físico anotado en un servidor; «usuario»: un equipo físico (o sin saber) que es su propio equipo. */
export type GrupoInfra = "virtual" | "servidor" | "usuario";

export function grupoInfra(e: Equipo, i: Pick<Infraestructura, "equipos"> | null | undefined): GrupoInfra {
  const t = tipoEquipo(e);
  if (t.clase === "virtual" || t.clase === "contenedor") return "virtual";
  return i?.equipos[e.id] ? "servidor" : "usuario";
}

/** Los hipervisores (de `HIPERVISORES`) donde puede vivir una plataforma que detecta el agente. */
export function hipervisoresPara(plataforma: string | null): string[] {
  switch (plataforma) {
    case "vmware":
      return ["vmware"];
    case "hyperv":
      return ["hyperv"];
    case "kvm":
      return ["proxmox", "kvm", "nutanix"];
    case "lxc":
      return ["proxmox"];
    case "xen":
      return ["xen"];
    case "virtualbox":
      return ["virtualbox"];
    default:
      return [];
  }
}

/**
 * El servidor que encaja con una máquina virtual o un contenedor: el único con el hipervisor
 * de su plataforma (o, si la plataforma no se sabe, el único servidor con hipervisor). Si hay
 * varios o ninguno, null (la persona elige).
 */
export function servidorPropuesto(e: Equipo, i: Pick<Infraestructura, "servidores"> | null | undefined): ServidorFisico | null {
  const t = tipoEquipo(e);
  if (t.clase !== "virtual" && t.clase !== "contenedor") return null;
  const con = (i?.servidores ?? []).filter((s) => s.hipervisor);
  const hv = hipervisoresPara(t.plataforma);
  const encajan = hv.length ? con.filter((s) => hv.includes(s.hipervisor!)) : con;
  return encajan.length === 1 ? encajan[0] : null;
}

/** Los servidores ordenados para elegir: primero los que encajan con su plataforma. */
export function servidoresParaElegir(e: Equipo, i: Pick<Infraestructura, "servidores"> | null | undefined): ServidorFisico[] {
  const hv = hipervisoresPara(tipoEquipo(e).plataforma);
  const xs = [...(i?.servidores ?? [])];
  return hv.length ? xs.sort((a, b) => Number(hv.includes(b.hipervisor ?? "")) - Number(hv.includes(a.hipervisor ?? ""))) : xs;
}

export interface EquiposPorGrupo {
  /** Máquinas virtuales y contenedores; los que faltan por anotar, arriba. */
  virtual: Equipo[];
  servidor: Equipo[];
  usuario: Equipo[];
  /** Lo que de verdad falta: máquinas virtuales y contenedores sin servidor. */
  faltan: number;
}

export function equiposPorGrupo(equipos: readonly Equipo[], i: Pick<Infraestructura, "equipos"> | null | undefined): EquiposPorGrupo {
  const out: EquiposPorGrupo = { virtual: [], servidor: [], usuario: [], faltan: 0 };
  for (const e of equipos) out[grupoInfra(e, i)].push(e);
  const anotado = (e: Equipo) => !!i?.equipos[e.id];
  out.virtual.sort((a, b) => Number(anotado(a)) - Number(anotado(b)));
  out.faltan = out.virtual.filter((e) => !anotado(e)).length;
  return out;
}

/** El texto de arriba de la lista: qué se espera y para qué sirve. */
export const TEXTO_EQUIPOS_INFRA =
  "Di en qué servidor físico está cada máquina virtual o contenedor: así la regla 3-2-1-1-0 sabe si una copia y sus datos dependen del mismo servidor (y avisa de «servidor físico compartido»). Los equipos de usuario son su propio equipo: no hace falta anotar nada.";

/** «Falta anotar 2 máquinas virtuales», o null si no falta nada. */
export function textoFaltan(n: number): string | null {
  if (!n) return null;
  return n === 1 ? "Falta decir dónde está 1 máquina virtual o contenedor." : `Falta decir dónde están ${n} máquinas virtuales o contenedores.`;
}
