// La configuración que se envía a un equipo (orden `config`, §6), como la
// prepara «Añadir o cambiar copias», y el plan de «Aplicar una plantilla» a varios
// equipos a la vez (v1.52, tarea 6). Sin estado ni red: se prueba en
// scripts/vectores-etiquetas.ts.
import { admiteTodasLasBases, admiteVerificarVolcado, ganchosDe, paraConfig, VERSION_GANCHOS, versionAlMenos } from "./ganchos";
import { errorReglas, normalizar, reglasDe, VERSION_REGLAS, VERSION_SOLO_CAMBIOS } from "./horario";
import type { Plantilla } from "./plantillas";
import { gruposAlDia } from "./exclusionesCopia";
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
  /** Plan 0.7.26: `inicio: "despues"` y `retraso_min` (agente con `admite: "inicio_despues"`). */
  despues?: boolean;
  /** Plan 0.8.0, F1: `a_mano` y `termina` (agente con `admite: "termina"`). */
  termina?: boolean;
  /** 14ccc: `verificar` en el gancho de SQL Server (agente con `admite: "verificar_volcado"`). */
  verificarVolcado?: boolean;
  /** 15u: «todas las bases» en el gancho de SQL Server (agente con `admite: "sqlserver_todas"`). */
  todasLasBases?: boolean;
  /** 15v: las etiquetas de las versiones de cada copia (agente con `admite: "etiquetas_copia"`). */
  etiquetas?: boolean;
  /** 15y: de qué grupos vienen las reglas y «no copiar más de N MB» (agente con `admite: "grupos_en_copias"`). */
  grupos?: boolean;
  /** 15ii: «al volver a conectarse» y `config.movil` (agente con `admite: "equipos_moviles"`). */
  moviles?: boolean;
  /** 15kk: apagar o hibernar al terminar (agente con `admite: "apagar_al_terminar"`). */
  apagar?: boolean;
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
    despues: !!equipo.resumen?.admite?.includes("inicio_despues"),
    termina: !!equipo.resumen?.admite?.includes("termina"),
    verificarVolcado: admiteVerificarVolcado(equipo),
    todasLasBases: admiteTodasLasBases(equipo),
    etiquetas: !!equipo.resumen?.admite?.includes("etiquetas_copia"),
    grupos: !!equipo.resumen?.admite?.includes("grupos_en_copias"),
    moviles: !!equipo.resumen?.admite?.includes("equipos_moviles"),
    apagar: !!equipo.resumen?.admite?.includes("apagar_al_terminar"),
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
    // Plan 0.7.26: `inicio` solo para «despues» (lo demás se deduce de `tras`) y el retraso si hay; los dos, solo a un agente que los entiende.
    // Plan 0.8.0, F1: «Solo a mano» y «Termina», solo a un agente que los entiende (la consola no se los ofrece a otro).
    const { solo_si_cambios, tras, inicio, retraso_min, a_mano, termina, etiquetas, grupos_exclusiones, mayores_mb, excepto_tamano, al_reconectar, al_terminar, al_terminar_servidor, ...resto } = k;
    const conTras = !!a.cadenas && !!tras;
    const conFin = !!a.termina && !!termina && (termina.veces != null || !!termina.hasta || !!termina.una_vez);
    return {
      ...resto,
      horario: {
        dias: [...new Set(k.horario.dias)].sort(),
        horas: normalizar(k.horario.horas),
        ...(a.reglas && k.horario.reglas?.length ? { reglas: k.horario.reglas } : {}),
      },
      gancho: a.ganchos ? paraConfig(ganchosDe(k.gancho as Gancho | Gancho[] | null), !!a.verificarVolcado, !!a.todasLasBases) : null,
      ...(a.soloCambios ? { solo_si_cambios: solo_si_cambios !== false } : {}),
      ...(conTras ? { tras } : {}),
      ...(conTras && a.despues && inicio === "despues" ? { inicio } : {}),
      ...(conTras && a.despues && retraso_min && retraso_min > 0 ? { retraso_min } : {}),
      ...(a.termina && a_mano ? { a_mano: true } : {}),
      ...(conFin ? { termina: Object.fromEntries(Object.entries(termina!).filter(([, v]) => v != null && v !== "")) } : {}),
      // 15v: sin etiquetas no va el campo (el agente pone «resguardo-agente»).
      ...(a.etiquetas && etiquetas?.length ? { etiquetas: [...etiquetas] } : {}),
      // 15y: con un agente anterior, los grupos ya están en `exclusiones` (sus reglas); el límite, no se le ofrece.
      ...(a.grupos && grupos_exclusiones?.length ? { grupos_exclusiones: grupos_exclusiones.map((g) => ({ id: g.id, nombre: g.nombre, reglas: [...g.reglas] })) } : {}),
      ...(a.grupos && mayores_mb ? { mayores_mb, ...(excepto_tamano?.length ? { excepto_tamano: [...excepto_tamano] } : {}) } : {}),
      // 15ii: además, al volver a conectarse (con su horario: sin él no tendría sentido).
      ...(a.moviles && al_reconectar && !conTras && !a_mano ? { al_reconectar: true } : {}),
      // 15kk: apagar o hibernar al terminar (en un servidor, con su confirmación).
      ...(a.apagar && al_terminar ? { al_terminar, ...(al_terminar_servidor ? { al_terminar_servidor: true } : {}) } : {}),
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
    // 15ii: un equipo que entra y sale (solo a un agente que lo entiende y si se ha tocado).
    ...(a.moviles && c0.movil ? { movil: movilParaEnviar(c0.movil) } : {}),
  };
}

/** 15ii: `config.movil` sin lo que no dice nada (como lo guarda el agente). */
export function movilParaEnviar(m: NonNullable<Configuracion["movil"]>) {
  return {
    ...(m.sale_oficina ? { sale_oficina: true, ...(m.margen_dias ? { margen_dias: Math.trunc(m.margen_dias) } : {}) } : {}),
    ...(m.bateria_min ? { bateria_min: Math.trunc(m.bateria_min) } : {}),
    ...(m.permitir_medida ? { permitir_medida: true } : {}),
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
export function planPlantilla(cfg: Configuracion, p: Plantilla, a: Admite, nuevoId: string, grupos: { id: string; nombre: string; reglas: string[] }[] = []): PlanPlantilla {
  const repos = cfg.repositorios.filter((r) => !r.solo_lectura).map((r) => ({ id: r.id, nombre: r.nombre }));
  if (!repos.length) return { ok: false, motivo: "Sin repositorio donde guardar: créalo antes en su ficha." };
  const nombre = p.nombre.trim();
  if (cfg.copias.some((k) => k.nombre.trim().toLowerCase() === nombre.toLowerCase())) return { ok: false, motivo: `Ya tiene una copia «${nombre}»: se salta.` };
  if (!p.copia.carpetas.length) return { ok: false, motivo: "La plantilla no tiene carpetas." };
  const ganchos = ganchosDe(p.copia.gancho);
  if (ganchos.length && !a.ganchos) return { ok: false, motivo: "Su agente es anterior a los pasos «Antes de copiar» de la plantilla: actualízalo primero." };
  const horario = JSON.parse(JSON.stringify(p.copia.horario)) as Horario;
  const reglas = reglasDe(horario, a.reglas);
  if (!reglas.length || errorReglas(reglas, a.reglas)) return { ok: false, motivo: "El horario de la plantilla no vale para su agente: aplícala desde «Añadir o cambiar copias»." };
  const copia: CopiaConfig = {
    id: nuevoId,
    nombre,
    repo: repos[0].id,
    carpetas: [...p.copia.carpetas],
    exclusiones: [...p.copia.exclusiones],
    horario,
    activa: true,
    gancho: JSON.parse(JSON.stringify(ganchos)) as Gancho[],
    solo_si_cambios: p.copia.solo_si_cambios !== false,
    // 15y y 15v: viajan con la plantilla (configParaEnviar solo los manda a quien los entiende).
    ...(p.copia.grupos_exclusiones?.length ? { grupos_exclusiones: JSON.parse(JSON.stringify(p.copia.grupos_exclusiones)) } : {}),
    ...(p.copia.mayores_mb && a.grupos ? { mayores_mb: p.copia.mayores_mb, excepto_tamano: [...(p.copia.excepto_tamano ?? [])] } : {}),
    ...(p.copia.etiquetas?.length ? { etiquetas: [...p.copia.etiquetas] } : {}),
  };
  // 0.8.2 (C2): sus grupos de exclusiones con las reglas de ahora (la plantilla guarda las de entonces).
  gruposAlDia(copia, grupos);
  return { ok: true, repos, copia };
}
