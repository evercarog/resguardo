// Salud de un equipo en una palabra (y su tono), a partir de lo que informa.
// Un estado nunca va solo con color: siempre icono y texto (docs/diseno.md).
import type { CopiaResumen, Equipo, EstadoOrden } from "./tipos";
import { cuandoFrase, relativo } from "./formato";

export type Tono = "ok" | "warn" | "bad" | "info" | "paused" | "neutral";

export interface Salud {
  tono: Tono;
  texto: string;
  /** Una frase con qué pasa y qué hacer. */
  detalle: string;
}

const DIA = 86_400_000;
const minuscula = (t: string) => t.charAt(0).toLowerCase() + t.slice(1);

export function copiaAtrasada(c: CopiaResumen, ahora = Date.now()) {
  return c.activa !== false && !!c.proxima && Date.parse(c.proxima) < ahora - 3600_000;
}

/** ¿Copias automáticas en pausa? (`pausado_hasta`: una fecha o "indefinido"). */
export function enPausa(e: Equipo, ahora = Date.now()) {
  const p = e.resumen?.pausado_hasta;
  return p === "indefinido" || (!!p && Date.parse(p) > ahora);
}

/** La próxima copia programada (de las activas), o null. */
export function proximaCopia(copias: CopiaResumen[], ahora = Date.now()): string | null {
  return (
    copias
      .filter((c) => c.activa !== false && c.proxima && Date.parse(c.proxima) > ahora - 3600_000)
      .map((c) => c.proxima!)
      .sort()[0] ?? null
  );
}

/** Qué decir cuando aún no hay ninguna copia hecha. */
export function primeraCopiaFrase(copias: CopiaResumen[], ahora = Date.now()): string {
  const p = proximaCopia(copias, ahora);
  return p ? `La primera copia está programada para ${cuandoFrase(p, ahora)}. También puedes pulsar «Copiar ahora».` : "Pulsa «Copiar ahora» para hacer la primera.";
}

export function saludEquipo(e: Equipo, ahora = Date.now()): Salud {
  if (e.modo === "trasladado") return { tono: "neutral", texto: "Trasladado", detalle: "Se fue a otro servidor: desde aquí ya no recibe órdenes." };
  if (!e.confirmado) return { tono: "neutral", texto: "Sin confirmar", detalle: "Falta confirmar el número de comprobación." };
  if (e.estado_servicio === "detenido_por_admin")
    return { tono: "bad", texto: "Detenido", detalle: "Alguien con permisos de administrador paró el servicio en el equipo. Lo ya copiado sigue a salvo." };
  const sinContacto = !e.ultimo_contacto || Date.parse(e.ultimo_contacto) < ahora - DIA;
  if (sinContacto)
    return { tono: "bad", texto: "Sin contacto", detalle: `No se conecta desde ${relativo(e.ultimo_contacto, ahora)}. Comprueba que está encendido y con red.` };
  const copias = e.resumen?.copias ?? [];
  const fallida = copias.find((c) => c.ultima?.estado === "fallo");
  if (fallida) return { tono: "bad", texto: "Copia fallida", detalle: `La última copia de «${fallida.nombre}» falló${fallida.ultima?.mensaje ? `: ${minuscula(fallida.ultima.mensaje.replace(/\.$/, ""))}` : ""}.` };
  if (enPausa(e, ahora))
    return {
      tono: "paused",
      texto: "En pausa",
      detalle: e.resumen?.pausado_hasta === "indefinido" ? "Las copias automáticas están en pausa hasta que alguien las reanude." : `Las copias automáticas vuelven ${relativo(e.resumen?.pausado_hasta, ahora)}.`,
    };
  const atrasada = copias.find((c) => copiaAtrasada(c, ahora));
  if (atrasada)
    return {
      tono: "warn",
      texto: "Atrasado",
      detalle: atrasada.ultima ? `«${atrasada.nombre}» no se hace desde ${relativo(atrasada.ultima.cuando, ahora)}.` : `La primera copia de «${atrasada.nombre}» debía hacerse ${relativo(atrasada.proxima, ahora)} y aún no se ha hecho.`,
    };
  const aviso = copias.find((c) => c.ultima?.estado === "aviso");
  if (aviso) return { tono: "warn", texto: "Con avisos", detalle: aviso.ultima?.mensaje ?? `La última copia de «${aviso.nombre}» terminó con avisos.` };
  if (!copias.length && e.rol !== "almacenamiento") return { tono: "neutral", texto: "Sin copias", detalle: "Todavía no tiene copias. Crea la primera." };
  // «Al día» solo con alguna copia hecha: hasta entonces, neutro y con cuándo será la primera.
  const activas = copias.filter((c) => c.activa !== false);
  if (activas.length && !activas.some((c) => c.ultima && c.ultima.estado !== "fallo"))
    return { tono: "neutral", texto: "Sin copias todavía", detalle: primeraCopiaFrase(activas, ahora) };
  return { tono: "ok", texto: "Al día", detalle: e.rol === "almacenamiento" ? "Guarda copias y responde con normalidad." : "Todas sus copias están al día." };
}

/** ¿Un resultado del agente en texto (p. ej. del espejo) es un error? Los errores empiezan por «ERROR:». */
export const resultadoConError = (r: string | null | undefined) => !!r && /^\s*ERROR/i.test(r);

/** Prioridad para ordenar listas: lo urgente arriba. */
export const PESO: Record<Tono, number> = { bad: 0, warn: 1, paused: 2, info: 3, neutral: 4, ok: 5 };

export const ESTADO_ORDEN: Record<EstadoOrden, { texto: string; tono: Tono }> = {
  pendiente: { texto: "Pendiente", tono: "neutral" },
  entregada: { texto: "Entregada", tono: "info" },
  en_marcha: { texto: "En marcha", tono: "info" },
  hecha: { texto: "Hecha", tono: "ok" },
  fallida: { texto: "Fallida", tono: "bad" },
  rechazada: { texto: "Rechazada", tono: "bad" },
  cancelada: { texto: "Cancelada", tono: "neutral" },
  caducada: { texto: "Caducada", tono: "warn" },
};

/** Nombre de cada tipo de orden, para personas. */
export const NOMBRE_ORDEN: Record<string, string> = {
  copiar_ahora: "Copiar ahora",
  verificar_ahora: "Verificar ahora",
  probar_restauracion: "Probar la restauración",
  subir_ahora: "Subir a la nube ahora",
  desbloquear: "Desbloquear",
  reanudar: "Reanudar las copias",
  actualizar_agente: "Actualizar el agente",
  abrir_sesion: "Ver el progreso",
  explorar: "Explorar versiones",
  restaurar: "Restaurar",
  descargar: "Descargar archivos",
  cambiar_retencion: "Cambiar la retención",
  aplicar_retencion: "Aplicar la retención",
  quitar_repositorio: "Quitar un repositorio",
  dejar_de_copiar: "Dejar de copiar",
  cambiar_copia_externa: "Cambiar la copia externa",
  cambiar_derivada: "Cambiar una copia derivada",
  quitar_derivada: "Quitar una copia derivada",
  rotar_contrasena_repo: "Cambiar la contraseña del repositorio",
  alta: "Dar de alta",
  config: "Cambiar las copias",
  elegir_carpetas: "Elegir carpetas",
  pausar: "Pausar las copias",
  baja_equipo: "Dar de baja el equipo",
  desvincular: "Desvincular",
  cambiar_servidor: "Cambiar de servidor",
  cambiar_espera: "Cambiar la espera",
  cambiar_clave_admin: "Cambiar la clave de administración",
  guarda_copias: "Guarda copias",
  crear_repositorio: "Crear un repositorio",
  cambiar_destino: "Cambiar las credenciales del destino",
  servidores_respaldo: "Servidores de respaldo",
  conectar_nube: "Conectar una nube",
  quitar_nube: "Desconectar una nube",
  compartir_acceso: "Compartir el acceso a un repositorio",
  importar_repositorio: "Importar un repositorio",
  adoptar_repositorio: "Usar un repositorio que ya existe",
  copiar_historial: "Traer historial de otro repositorio",
  clave_almacen: "Dar al almacén su clave del repositorio",
  retencion_almacen: "Retención en el almacén",
  aplicar_retencion_almacen: "Aplicar la retención en el almacén",
  anadir_consola: "Conectar también a otra consola",
  quitar_consola: "Quitar una consola",
  cancelar_espera: "Cancelar una orden en espera",
};

export const nombreOrden = (t: string) => NOMBRE_ORDEN[t] ?? t.replaceAll("_", " ");
