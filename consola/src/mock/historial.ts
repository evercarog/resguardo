// Informe detallado de ejemplo de un repositorio (api-servidor.md v1.7 §6,
// `RepoInforme`): 60 días de versiones y vueltas, espacio, verificación,
// prueba de restauración, copia externa y salud de la protección.
// Determinista (semilla por repositorio).
import type * as T from "../lib/tipos";

function aleatorio(semilla: string) {
  let h = 2166136261;
  for (const c of semilla) h = Math.imul(h ^ c.charCodeAt(0), 16777619);
  return () => {
    h = Math.imul(h ^ (h >>> 15), 2246822507);
    h = Math.imul(h ^ (h >>> 13), 3266489909);
    return ((h ^= h >>> 16) >>> 0) / 4294967296;
  };
}

const hex8 = (r: () => number) => Array.from({ length: 8 }, () => "0123456789abcdef"[Math.floor(r() * 16)]).join("");

export interface OpcionesHistorial {
  /** Proporción de vueltas que fallan / con avisos / sin cambios. */
  fallos?: number;
  avisos?: number;
  sinCambios?: number;
  /** La última vuelta falló (para «Copia fallida»). */
  ultimaFalla?: boolean;
  /** Tamaño medio de lo añadido por versión. */
  medioAnadido?: number;
  externa?: boolean;
  soloAnadir?: boolean | null;
  recortado?: boolean;
  /** El equipo dejó de informar hace estas horas (nada más reciente). */
  silencioHoras?: number;
}

/** El `RepoInforme` de un repositorio a partir de sus copias y su horario. */
export function informeRepo(r: T.RepositorioResumen, copias: T.CopiaResumen[], o: OpcionesHistorial = {}): T.RepoInforme {
  const rnd = aleatorio(r.id);
  const suyas = copias.filter((k) => k.repo === r.id);
  const versiones: T.VersionInforme[] = [];
  const ejecuciones: T.EjecucionInforme[] = [];
  const ahora = Date.now() - (o.silencioHoras ?? 0) * 3600_000;
  const total = r.bytes ?? 10_000_000_000;
  for (let d = 59; d >= 0; d--) {
    const dia = new Date(ahora - d * 86_400_000);
    for (const k of suyas) {
      const h = typeof k.horario === "object" && k.horario ? k.horario : { dias: [1, 2, 3, 4, 5], horas: ["13:00"] };
      const dow = dia.getDay() === 0 ? 7 : dia.getDay();
      if (!h.dias.includes(dow)) continue;
      for (const hh of h.horas) {
        const [H, M] = hh.split(":").map(Number);
        const t = new Date(dia.getFullYear(), dia.getMonth(), dia.getDate(), H, M + Math.floor(rnd() * 4));
        const dur = 40 + rnd() * 260;
        const fin = t.getTime() + dur * 1000;
        if (fin > ahora - 5 * 60_000) continue;
        const x = rnd();
        const pf = o.fallos ?? 0.03;
        const pa = o.avisos ?? 0.04;
        let resultado: T.EjecucionInforme["resultado"] = x < pf ? "fallo" : x < pf + pa ? "aviso" : x > 1 - (o.sinCambios ?? 0.12) ? "sin_cambios" : "ok";
        if (o.ultimaFalla && fin > ahora - 30 * 3600_000) resultado = "fallo";
        const hora = new Date(fin).toISOString();
        const conVersion = resultado === "ok" || resultado === "aviso";
        const anadido = conVersion ? Math.round((o.medioAnadido ?? 180_000_000) * (0.2 + rnd() * 1.8)) : 0;
        const nuevos = conVersion ? Math.floor(rnd() * 40) : 0;
        const cambiados = conVersion ? Math.floor(rnd() * 120) : 0;
        ejecuciones.push({
          hora,
          copia: k.id,
          resultado,
          mensaje_corto: resultado === "fallo" ? "No se pudo conectar con el servidor de copias." : resultado === "aviso" ? "3 archivos en uso no se pudieron leer." : null,
          // v1.12: lo de cada vuelta.
          duracion_s: resultado === "fallo" ? Math.round(dur / 8) : Math.round(dur),
          anadido: resultado === "fallo" ? null : anadido,
          archivos_nuevos: resultado === "fallo" ? null : nuevos,
          archivos_cambiados: resultado === "fallo" ? null : cambiados,
          reintento: false,
        });
        if (!conVersion) continue;
        versiones.push({
          id: hex8(rnd),
          hora,
          copia: k.id,
          total_bytes: Math.round(total * (0.92 + 0.08 * ((60 - d) / 60))),
          anadido,
          anadido_empaquetado: Math.round(anadido * 0.46),
          archivos_nuevos: nuevos,
          archivos_cambiados: cambiados,
          archivos_sin_cambios: 18_000 + Math.floor(rnd() * 900),
          duracion_s: Math.round(dur),
          etiquetas: d % 30 === 0 ? ["mensual"] : [],
        });
      }
    }
  }
  versiones.sort((a, b) => Date.parse(b.hora) - Date.parse(a.hora));
  ejecuciones.sort((a, b) => Date.parse(b.hora) - Date.parse(a.hora));
  const fallo = ejecuciones[0]?.resultado === "fallo";
  const hace = (h: number) => new Date(ahora - h * 3600_000).toISOString();
  const items: T.ComprobacionProteccion[] = [
    fallo
      ? { id: "copias", estado: "fallo", etiqueta: "Copias automáticas", detalle: "La última copia automática de alguna copia falló." }
      : { id: "copias", estado: "ok", etiqueta: "Copias automáticas", detalle: "Activas y al día." },
    o.soloAnadir === false
      ? { id: "borrado", estado: "aviso", etiqueta: "Protegida contra borrado", detalle: "Desde este equipo se podrían borrar versiones." }
      : o.soloAnadir === null
        ? { id: "borrado", estado: "desconocido", etiqueta: "Protegida contra borrado", detalle: "Sin comprobar: no se pudo consultar al servidor." }
        : { id: "borrado", estado: "ok", etiqueta: "Protegida contra borrado", detalle: "El servidor es de solo añadir: desde este equipo no se puede borrar nada." },
    o.externa
      ? { id: "externa", estado: "ok", etiqueta: "Copia externa", detalle: "Al día (última subida hace 15 h)." }
      : { id: "externa", estado: "fallo", etiqueta: "Copia externa", detalle: "Todas las versiones están en un solo sitio: configura una copia externa (la nube u otro disco)." },
    { id: "verificacion", estado: "ok", etiqueta: "Verificación", detalle: "Correcta hace 1 día." },
    { id: "restauracion", estado: "ok", etiqueta: "Prueba de restauración", detalle: "Correcta hace 9 días." },
    { id: "kit", estado: "ok", etiqueta: "Kit de recuperación", detalle: "Guardado al crear el repositorio." },
    { id: "retencion", estado: "ok", etiqueta: "Retención", detalle: "7 diarias, 4 semanales y 12 mensuales." },
  ];
  return {
    id: r.id,
    nombre: r.nombre,
    solo_lectura: !!r.solo_lectura,
    versiones,
    versiones_leidas: hace(2),
    ejecuciones,
    espacio: { en_disco_bytes: Math.round(total * 0.46), sin_comprimir: total, ratio: 2.17, leido: hace(10) },
    verificacion: { ultima: hace(30), resultado: "ok", mensaje_corto: "Sin errores en el 5 % revisado." },
    prueba_restauracion: { ultima: hace(24 * 9), resultado: "ok", mensaje_corto: "25 archivos restaurados y comprobados." },
    externa: o.externa ? { ultima: hace(15), resultado: "ok", mensaje_corto: null } : null,
    proteccion: { puntuacion: items.filter((i) => i.estado === "ok").length, total: items.length, items },
    ...(o.recortado ? { recortado: true } : {}),
  };
}

/**
 * v1.23: el historial que guarda el propio equipo (para siempre), de ejemplo:
 * una verificación por semana, vueltas de 61 a 89 días atrás (de antes de que
 * el informe las traiga) con sus ganchos y, de hace más de un año, resúmenes
 * por día. De la más reciente a la más antigua.
 */
export function historialMock(eq: { id: string; resumen: T.Equipo["resumen"] }): T.EntradaHistorial[] {
  const out: T.EntradaHistorial[] = [];
  const rnd = aleatorio(`historial-${eq.id}`);
  const ahora = Date.now();
  for (const r of eq.resumen?.repositorios ?? []) {
    for (let semana = 1; semana <= 12; semana++) {
      const hora = new Date(ahora - semana * 7 * 86_400_000 - 3 * 3600_000).toISOString();
      const fallo = rnd() < 0.15;
      out.push({ id: `${r.id}-v${semana}`, hora, tipo: "verificacion", repo: r.id, resultado: fallo ? "fallo" : "ok", ...(fallo ? { mensaje: "Faltan 2 paquetes de datos." } : {}) });
    }
    const copia = eq.resumen?.copias?.find((k) => k.repo === r.id);
    if (!copia) continue;
    for (let d = 61; d < 90; d += 3) {
      const hora = new Date(ahora - d * 86_400_000).toISOString();
      out.push({
        id: `${r.id}-c${d}`,
        hora,
        tipo: "copia",
        repo: r.id,
        copia: copia.id,
        resultado: "ok",
        duracion_s: Math.round(60 + rnd() * 200),
        anadido: Math.round(rnd() * 300_000_000),
        archivos_nuevos: Math.floor(rnd() * 30),
        archivos_cambiados: Math.floor(rnd() * 90),
        reintento: false,
        ...(d % 2 ? { ganchos: [{ tipo: "carpeta_reciente" as const, estado: "ok" as const, mensaje: "Hay un archivo de hoy." }] } : {}),
      });
    }
    for (let d = 400; d < 420; d += 2) {
      const hora = new Date(ahora - d * 86_400_000).toISOString();
      const mal = d % 6 === 0 ? 1 : 0;
      out.push({
        id: `dia-${r.id}-${copia.id}-${hora.slice(0, 10)}`,
        hora,
        tipo: "resumen_dia",
        dia: hora.slice(0, 10),
        repo: r.id,
        copia: copia.id,
        ok: 2 - mal,
        fallidas: mal,
        sin_cambios: 1,
        duracion_s: 400,
        anadido: Math.round(rnd() * 500_000_000),
        ...(mal ? { ultimo_error: "No se pudo conectar con el destino." } : {}),
      });
    }
  }
  return out.sort((a, b) => Date.parse(b.hora) - Date.parse(a.hora));
}
