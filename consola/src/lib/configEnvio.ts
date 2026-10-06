// La configuración que se envía a un equipo (orden `config`, §6), como la
// prepara «Cambiar las copias», y el plan de «Aplicar una plantilla» a varios
// equipos a la vez (v1.52, tarea 6). Sin estado ni red: se prueba en
// scripts/vectores-etiquetas.ts.
import { ganchosDe, paraConfig, VERSION_GANCHOS, versionAlMenos } from "./ganchos";
import { errorReglas, normalizar, reglasDe, VERSION_REGLAS, VERSION_SOLO_CAMBIOS } from "./horario";
import type { Plantilla } from "./plantillas";
import type { Configuracion, CopiaConfig, Equipo, Gancho, Horario } from "./tipos";
import { admiteVerificacion, admiteVerificacionHorario } from "./verificacion";

/** Tarea 8: el agente programa la prueba de restauración desde la consola (`config.pruebas_restauracion`). */
export const ADMITE_PRUEBA_AUTO = "prueba_auto";
export const admitePruebaAuto = (e: Equipo | null | undefined) => !!e?.resumen?.admite?.includes(ADMITE_PRUEBA_AUTO);
/** La prueba de restauración que propone la consola: cada mes (cada 30 días). */
export const PRUEBA_POR_DEFECTO = { cada_dias: 30 } as const;

/** Lo que entiende el agente de un equipo (según su versión y lo que anuncia). */
export interface Admite {
  reglas: boolean;
  ganchos: boolean;
  soloCambios: boolean;
  verif: boolean;
  verifHorario: boolean;
  escritorio: boolean;
  /** Tarea 7c: «después de la anterior» (`config.copias[].tras`). */
  cadenas?: boolean;
  /** Tarea 8: `config.pruebas_restauracion` (agente con `admite: "prueba_auto"`). */
  pruebas?: boolean;
}

export function admiteDe(equipo: Equipo, version: string | null | undefined = equipo.version_agente): Admite {
  return {
    reglas: versionAlMenos(version, VERSION_REGLAS),
    ganchos: versionAlMenos(version, VERSION_GANCHOS),
    soloCambios: versionAlMenos(version, VERSION_SOLO_CAMBIOS),
    verif: admiteVerificacion(equipo),
    verifHorario: admiteVerificacionHorario(equipo),
    escritorio: !!equipo.resumen?.admite?.includes("escritorio"),
    cadenas: !!equipo.resumen?.admite?.includes("cadenas"),
    pruebas: admitePruebaAuto(equipo),
  };
}

/** Lista para editar: los ganchos siempre como lista y «solo si hay cambios» encendido si no se dice. */
export function paraEditar(c0: Configuracion): Configuracion {
  return { ...c0, copias: c0.copias.map((k) => ({ ...k, gancho: ganchosDe(k.gancho), solo_si_cambios: k.solo_si_cambios ?? true })) };
}

/**
 * Solo lo que decide la consola: copias (con sus ganchos de plantilla),
 * verificación y bandeja. Los repositorios y destinos los escribe el equipo
 * (de `crear_repositorio`): van tal cual.
 */
export function configParaEnviar(c0: Configuracion, a: Admite) {
  // Horas ordenadas y sin repetir; «solo si hay cambios» solo a agentes que lo entienden (≥ 0.7.7; los anteriores lo hacen siempre).
  // Las reglas del horario, solo a agentes ≥ 0.7.9 (con uno anterior no se llega aquí con reglas: «Antes de enviar» lo impide).
  const copias = c0.copias.map((k) => {
    // Tarea 7c: `tras` solo a un agente que lo entiende (uno anterior haría la copia solo con su horario).
    const { solo_si_cambios, tras, ...resto } = k;
    return {
      ...resto,
      horario: {
        dias: [...new Set(k.horario.dias)].sort(),
        horas: normalizar(k.horario.horas),
        ...(a.reglas && k.horario.reglas?.length ? { reglas: k.horario.reglas } : {}),
      },
      gancho: a.ganchos ? paraConfig(ganchosDe(k.gancho as Gancho | Gancho[] | null)) : null,
      ...(a.soloCambios ? { solo_si_cambios: solo_si_cambios !== false } : {}),
      ...(a.cadenas && tras ? { tras } : {}),
    };
  });
  return {
    v: 1,
    copias,
    repositorios: c0.repositorios,
    destinos: c0.destinos,
    verificacion: c0.verificacion ?? null,
    // v1.36: con `escritorio`, `bandeja.avisos` dice lo mismo para un agente anterior.
    bandeja: c0.escritorio ? { visible: c0.bandeja?.visible ?? true, avisos: c0.escritorio.avisos !== "off" } : (c0.bandeja ?? null),
    // v1.28: solo a un agente que la entiende, y solo si se ha tocado alguna vez.
    // v1.40: `horario` solo a un agente que lo entiende (con él, `cada_dias` es para uno anterior).
    ...(a.verif && c0.verificaciones
      ? { verificaciones: Object.fromEntries(Object.entries(c0.verificaciones).map(([r, v]) => [r, a.verifHorario && v.horario ? v : { cada_dias: v.cada_dias, porcentaje: v.porcentaje }])) }
      : {}),
    // v1.36: la ventana y los avisos (si el agente lo entiende y lo tiene o se ha tocado).
    ...(a.escritorio && c0.escritorio ? { escritorio: c0.escritorio } : {}),
    // Tarea 8: la prueba de restauración automática, como la verificación (solo a un agente que la entiende).
    ...(a.pruebas && c0.pruebas_restauracion ? { pruebas_restauracion: c0.pruebas_restauracion } : {}),
  };
}

export type PlanPlantilla = { ok: true; copia: CopiaConfig; repos: { id: string; nombre: string }[] } | { ok: false; motivo: string };

/**
 * Qué pasaría al aplicar una plantilla a un equipo: una copia nueva con lo de
 * la plantilla (carpetas, exclusiones, horario, «antes de copiar») en su primer
 * repositorio que admite escritura (se puede cambiar antes de enviar). Se salta
 * (con el motivo) si no puede: sin repositorio, ya tiene una copia con ese
 * nombre, o su agente no entiende algo de la plantilla. Nada se envía aquí.
 */
export function planPlantilla(cfg: Configuracion, p: Plantilla, a: Admite, nuevoId: string): PlanPlantilla {
  const repos = cfg.repositorios.filter((r) => !r.solo_lectura).map((r) => ({ id: r.id, nombre: r.nombre }));
  if (!repos.length) return { ok: false, motivo: "Sin repositorio donde guardar: créalo antes en su ficha." };
  const nombre = p.nombre.trim();
  if (cfg.copias.some((k) => k.nombre.trim().toLowerCase() === nombre.toLowerCase())) return { ok: false, motivo: `Ya tiene una copia «${nombre}»: se salta.` };
  if (!p.copia.carpetas.length) return { ok: false, motivo: "La plantilla no tiene carpetas." };
  const ganchos = ganchosDe(p.copia.gancho);
  if (ganchos.length && !a.ganchos) return { ok: false, motivo: "Su agente es anterior a los pasos «Antes de copiar» de la plantilla: actualízalo primero." };
  const horario = JSON.parse(JSON.stringify(p.copia.horario)) as Horario;
  const reglas = reglasDe(horario, a.reglas);
  if (!reglas.length || errorReglas(reglas, a.reglas)) return { ok: false, motivo: "El horario de la plantilla no vale para su agente: aplícala desde «Cambiar las copias»." };
  return {
    ok: true,
    repos,
    copia: {
      id: nuevoId,
      nombre,
      repo: repos[0].id,
      carpetas: [...p.copia.carpetas],
      exclusiones: [...p.copia.exclusiones],
      horario,
      activa: true,
      gancho: JSON.parse(JSON.stringify(ganchos)) as Gancho[],
      solo_si_cambios: p.copia.solo_si_cambios !== false,
    },
  };
}
