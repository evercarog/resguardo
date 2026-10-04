// Plantillas de copia de un cliente (v1.20): «Copia de Siigo», «Documentos de
// usuario»… Llevan carpetas y ganchos (rutas de los equipos), así que se
// cifran aquí con una clave derivada de K_cfg (cripto/simetrico.ts,
// clavePlantillas) y el servidor solo guarda bytes. Usar una plantilla solo
// rellena el editor de copias: se revisa y se envía con la orden `config`
// de siempre, firmada con la clave de administración. Nada se aplica solo.
import * as api from "./api";
import { aB64, deB64 } from "./cripto/bytes";
import { cifrarPlantilla, clavePlantillas, descifrarPlantilla } from "./cripto/simetrico";
import type { CopiaConfig, Gancho, Horario, Regla } from "./tipos";
import { textoRegla } from "./retencion";

/** La de un repositorio (v1.28: también horarias y plazos). */
export type Retencion = Regla;

export interface Plantilla {
  v: 1;
  id: string;
  nombre: string;
  copia: {
    carpetas: string[];
    exclusiones: string[];
    /** Con `reglas` si se guardó desde un agente ≥ 0.7.9 con un horario que las necesita. */
    horario: Horario;
    solo_si_cambios?: boolean;
    gancho: Gancho[];
  };
  /** La retención del repositorio de la copia al guardarla (solo como sugerencia: es del repositorio). */
  retencion?: Retencion | null;
  creada: string;
  por?: string;
}

/** Las plantillas del cliente que se abren con esta K_cfg (y cuántas no). */
export async function cargarPlantillas(cliente: string, kcfg: Uint8Array): Promise<{ lista: Plantilla[]; ilegibles: number }> {
  const k = clavePlantillas(kcfg);
  try {
    const xs = await api.plantillas(cliente);
    const lista: Plantilla[] = [];
    let ilegibles = 0;
    for (const x of xs) {
      try {
        const p = descifrarPlantilla<Plantilla>(k, cliente, x.id, deB64(x.cifrado));
        // Lo de dentro tiene que decir lo mismo que lo de fuera.
        if (p.v !== 1 || p.id !== x.id) throw new Error("no cuadra");
        lista.push({ ...p, por: x.por });
      } catch {
        ilegibles++;
      }
    }
    return { lista: lista.sort((a, b) => a.nombre.localeCompare(b.nombre, "es")), ilegibles };
  } finally {
    k.fill(0);
  }
}

/** La plantilla de una copia del editor (sin su id, su repositorio ni si está activa). */
export function plantillaDe(k: CopiaConfig, nombre: string, ganchos: Gancho[], retencion: Retencion | null | undefined): Plantilla {
  return {
    v: 1,
    id: `pla-${crypto.randomUUID().slice(0, 12)}`,
    nombre: nombre.trim(),
    copia: {
      carpetas: [...k.carpetas],
      exclusiones: [...k.exclusiones],
      horario: JSON.parse(JSON.stringify(k.horario)) as Horario,
      solo_si_cambios: k.solo_si_cambios !== false,
      gancho: JSON.parse(JSON.stringify(ganchos)) as Gancho[],
    },
    retencion: retencion ? { ...retencion } : null,
    creada: new Date().toISOString(),
  };
}

export async function guardarPlantilla(cliente: string, kcfg: Uint8Array, p: Plantilla) {
  const k = clavePlantillas(kcfg);
  try {
    const { por: _por, ...limpia } = p;
    void _por;
    await api.ponerPlantilla(cliente, p.id, aB64(cifrarPlantilla(k, cliente, p.id, limpia)));
  } finally {
    k.fill(0);
  }
}

/** Una retención en frase corta: «7 diarias, 4 semanales, 12 mensuales y 2 anuales» (o, con plazos, como el agente). */
export function retencionEnFrase(r: Retencion): string {
  if (r.horarias || r.plazos || [r.diarias, r.semanales, r.mensuales, r.anuales].includes(-1)) return textoRegla(r);
  const partes = [
    r.diarias ? `${r.diarias} diarias` : "",
    r.semanales ? `${r.semanales} semanales` : "",
    r.mensuales ? `${r.mensuales} mensuales` : "",
    r.anuales ? `${r.anuales} anuales` : "",
  ].filter(Boolean);
  return partes.length > 1 ? `${partes.slice(0, -1).join(", ")} y ${partes.at(-1)}` : (partes[0] ?? "ninguna");
}
