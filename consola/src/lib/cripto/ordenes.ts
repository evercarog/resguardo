// Órdenes v2 (api-servidor.md §1 «Órdenes»): niveles, esperas, caducidad y el
// JSON que se sella para el equipo. El servidor solo ve el sobre y los
// metadatos en claro (tipo, seq, not_before y caduca), que el agente compara
// con los de dentro.
import { aB64, aleatorio, utf8 } from "./bytes";
import { sellarB64 } from "./sobre";

export type Nivel = "sesion" | "repo" | "admin";

/** Nivel de cada tipo (crates/protocolo/src/ordenes.rs). */
export const NIVEL: Record<string, Nivel> = {
  copiar_ahora: "sesion",
  verificar_ahora: "sesion",
  probar_restauracion: "sesion",
  subir_ahora: "sesion",
  desbloquear: "sesion",
  reanudar: "sesion",
  actualizar_agente: "sesion",
  abrir_sesion: "sesion",
  explorar: "repo",
  restaurar: "repo",
  descargar: "repo",
  cambiar_retencion: "repo",
  aplicar_retencion: "repo",
  quitar_repositorio: "repo",
  dejar_de_copiar: "repo",
  cambiar_copia_externa: "repo",
  // Tarea 4b: las demás copias derivadas de un repositorio.
  cambiar_derivada: "repo",
  quitar_derivada: "repo",
  rotar_contrasena_repo: "repo",
  alta: "admin",
  config: "admin",
  elegir_carpetas: "admin",
  pausar: "admin",
  baja_equipo: "admin",
  desvincular: "admin",
  cambiar_servidor: "admin",
  cambiar_espera: "admin",
  cambiar_clave_admin: "admin",
  guarda_copias: "admin",
  crear_repositorio: "admin",
  cambiar_destino: "admin",
  servidores_respaldo: "admin",
  conectar_nube: "admin",
  quitar_nube: "admin",
  importar_repositorio: "admin",
  adoptar_repositorio: "admin",
  copiar_historial: "admin",
  compartir_acceso: "repo",
  // v1.22: retención en el almacén (la clave, al equipo dueño; la regla, al almacén).
  clave_almacen: "repo",
  retencion_almacen: "admin",
  aplicar_retencion_almacen: "admin",
  // v1.35: varias consolas a la vez (docs/consolas-multiples.md).
  anadir_consola: "admin",
  quitar_consola: "admin",
  // v1.49: cancelar una orden en espera en el equipo, de cualquiera de sus consolas
  // (inofensiva: cancelar solo aumenta la protección; docs/consolas-multiples.md §5.7).
  cancelar_espera: "sesion",
  // v1.56: el nombre, las etiquetas y la observación del equipo, iguales en todas sus consolas
  // (docs/consolas-multiples.md §6). Inofensivas.
  nombre_equipo: "sesion",
  etiquetas_equipo: "sesion",
  observacion_equipo: "sesion",
  // Olvidar un destino que ya no usa nada (v1.56): desde la v1.59 pide la clave de
  // administración, como crearlo (olvida sus credenciales). Un agente anterior, que la
  // tenía por inofensiva, no mira la prueba que le llega de más.
  quitar_destino: "admin",
  // 0.7.26 (bloque 8): los datos comunes del cliente (colores de las etiquetas, catálogo de
  // destinos, plantillas), iguales en todas sus consolas. El tipo y las marcas de un destino
  // cuentan en la regla 3-2-1: con la clave de administración.
  datos_cliente: "sesion",
  datos_cliente_admin: "admin",
};

/** Además de la contraseña del repositorio, piden la clave de administración. */
export const PIDE_TAMBIEN_ADMIN = new Set(["quitar_repositorio", "compartir_acceso", "clave_almacen"]);

/** Tipos que abren una sesión interactiva (llevan `sesion`). */
/** Las que abren una sesión interactiva (protocolo/ordenes.rs). `restaurar` y `descargar` no: van sin `sesion`. */
export const ABRE_SESION = new Set(["abrir_sesion", "explorar", "elegir_carpetas"]);

/** Los técnicos no pueden mandarlas. */
export const SOLO_ADMIN_ROL = new Set(["baja_equipo", "desvincular", "cambiar_servidor", "cambiar_clave_admin", "servidores_respaldo", "anadir_consola", "quitar_consola", "nombre_equipo", "quitar_destino", "datos_cliente", "datos_cliente_admin"]);

/**
 * ¿Es destructiva? Algunas solo lo son con cierto cuerpo: desvincular con
 * «dejar de copiar» y «Guarda copias» al desactivarlo.
 */
/** Un destino del espejo del Servidor de copias (v1.9): otra carpeta o una nube conectada en el equipo. */
export interface DestinoEspejo {
  /** Tarea 7d.2: «zona», otra zona del almacén (`carpeta`: su id o «principal»). */
  tipo: "carpeta" | "nube" | "zona";
  carpeta?: string | null;
  nube?: string | null;
  /** Tarea 7d.2: de qué zona copia (sin ella, la principal). */
  zona?: string | null;
  /** (espejo por destino, docs/espejo.md) solo estos repositorios; sin ellos, todos. */
  repos?: string[] | null;
  /** Borra lo que ya no está en el almacén pasados estos días. */
  retencion_dias?: number | null;
  /** Bloqueo de objetos: nunca borra. */
  bloqueo?: boolean | null;
}
/** Lo que ya tiene el equipo, para saber si una orden quita algo (v1.9: quitar un destino del espejo es destructiva). */
export interface ContextoOrden {
  espejo?: { destinos?: DestinoEspejo[] | null } | null;
  /** Cuántas copias activas tiene ahora (resumen del equipo): `config` sin ninguna activa las para todas. */
  copiasActivas?: number;
  /** Tarea 4b: las copias derivadas que tiene (de qué repositorio, su id y su destino). */
  derivadas?: { repo: string; id: string; destino_id?: string | null }[];
}

/** ¿Una configuración deja sin ninguna copia activa (vacía o todas desactivadas)? */
export function configSinCopiasActivas(config: unknown): boolean {
  const copias = (config as { copias?: { activa?: boolean }[] } | null)?.copias;
  return !Array.isArray(copias) || !copias.some((k) => k?.activa !== false);
}

const sinBarraFinal = (x: string) => x.trim().replace(/[\\/]+$/, "");
/** Clave de un destino del espejo: tipo + carpeta + nube, como compara el agente. */
export const claveEspejo = (d: DestinoEspejo) => `${d.tipo}|${sinBarraFinal(d.carpeta ?? "")}|${(d.nube ?? "").trim()}${d.zona && d.zona !== "principal" ? `|${d.zona}` : ""}`;

/** Destinos de un cuerpo `espejo` (la forma nueva `{ destinos }` o la antigua `{ carpeta }`). */
export function destinosDeCuerpo(espejo: unknown): DestinoEspejo[] {
  const e = espejo as { destinos?: DestinoEspejo[]; carpeta?: string } | null | undefined;
  if (!e) return [];
  if (Array.isArray(e.destinos)) return e.destinos;
  return e.carpeta ? [{ tipo: "carpeta", carpeta: e.carpeta }] : [];
}

/** ¿El espejo nuevo deja fuera alguno de los destinos que ya tiene el equipo? */
function quitaDestinoEspejo(nuevo: unknown, actual: ContextoOrden["espejo"]): boolean {
  if (!actual) return false;
  // Espejo antiguo sin lista de destinos: no se sabe qué hay; mejor esperar.
  if (!Array.isArray(actual.destinos)) return true;
  const nuevos = new Map(destinosDeCuerpo(nuevo).map((d) => [claveEspejo(d), d]));
  return actual.destinos.some((d) => {
    const n = nuevos.get(claveEspejo(d));
    if (!n) return true;
    // Poner o acortar su retención (borrará), o quitar su bloqueo, también.
    const ret = (x: DestinoEspejo) => (x.bloqueo ? 0 : Number(x.retencion_dias ?? 0));
    if (ret(n) > 0 && (ret(d) === 0 || ret(n) < ret(d))) return true;
    if (d.bloqueo && !n.bloqueo) return true;
    // Dejar fuera repositorios que iban a ese destino (o pasar de todos a algunos) también.
    if (!Array.isArray(n.repos)) return false;
    if (!Array.isArray(d.repos)) return true;
    return d.repos.some((r) => !n.repos!.includes(r));
  });
}

export function esDestructiva(tipo: string, cuerpo: Record<string, unknown> = {}, esperaActualHoras?: number, contexto?: ContextoOrden): boolean {
  switch (tipo) {
    case "cambiar_retencion":
    case "aplicar_retencion":
    case "quitar_repositorio":
    case "dejar_de_copiar":
    case "pausar":
    case "baja_equipo":
    case "aplicar_retencion_almacen":
      return true;
    // v1.22: poner o cambiar la retención del almacén borrará versiones; quitarla, no.
    case "retencion_almacen":
      return cuerpo.quitar !== true;
    case "desvincular":
      return cuerpo.modo === "dejar_de_copiar";
    // Quitar la copia externa (hora: null) deja de proteger fuera de la oficina.
    // (v1.46: «Probar», `solo_probar`, no cambia nada.)
    case "cambiar_copia_externa":
      return cuerpo.hora === null && cuerpo.solo_probar !== true;
    // Tarea 4b: quitar una copia derivada, siempre; cambiar la retención o el destino de una que ya existe, también
    // (lo mismo que comprueba el agente, `derivada_reduce`). Crear una nueva o cambiar su horario, no.
    case "quitar_derivada":
      return true;
    case "cambiar_derivada": {
      if (cuerpo.solo_probar === true) return false;
      const actual = contexto?.derivadas?.find((d) => d.id === cuerpo.id && d.repo === cuerpo.repo);
      if (!actual) return false;
      const destino = (cuerpo.destino as { id?: string } | undefined)?.id;
      return (!!cuerpo.retencion && typeof cuerpo.retencion === "object") || destino !== actual.destino_id;
    }
    // Desconectar una nube que usa el espejo deja de proteger fuera.
    case "quitar_nube":
      return !!contexto?.espejo && (!Array.isArray(contexto.espejo.destinos) || contexto.espejo.destinos.some((d) => d.tipo === "nube" && (d.nube ?? "").trim() === String(cuerpo.nombre ?? "").trim()));
    case "guarda_copias":
      return (
        cuerpo.activo === false ||
        typeof cuerpo.quitar === "string" ||
        // Tarea 7b: quitar una zona del almacén (sus equipos dejan de poder copiar allí).
        typeof cuerpo.quitar_zona === "string" ||
        cuerpo.espejo === null ||
        // Confirmar lo que falta de golpe en el almacén (el espejo lo borrará pasados sus días).
        "espejo_freno" in cuerpo ||
        ("espejo" in cuerpo && quitaDestinoEspejo(cuerpo.espejo, contexto?.espejo))
      );
    // Vaciar o desactivar todas las copias que había deja el equipo sin copias automáticas.
    case "config":
      return (contexto?.copiasActivas ?? 0) > 0 && configSinCopiasActivas(cuerpo.config);
    // Acortar la espera reduce la protección.
    case "cambiar_espera":
      return esperaActualHoras !== undefined && Number(cuerpo.horas) < esperaActualHoras;
    // Restaurar en su sitio reemplazando lo que hay borra los archivos actuales.
    case "restaurar":
      return cuerpo.destino === "original" && cuerpo.reemplazar === true;
    default:
      return false;
  }
}

const HORA = 3600_000;

/**
 * v1.58: margen para entregar una orden con espera desde su hora (el equipo puede estar
 * apagado o sin red justo entonces). Antes, 24 h: una orden de un viernes con un fin de
 * semana sin el equipo caducaba sin aplicarse. Nunca pasa de 7 días desde que se emite.
 */
export const MARGEN_TRAS_ESPERA_H = 72;

/** Caducidad: 1 h lo interactivo, 7 días cambiar la clave o el servidor y conectar otra consola, 24 h lo demás (contado desde que puede ejecutarse). */
export function caducidad(tipo: string, desde: Date): Date {
  if (ABRE_SESION.has(tipo) || tipo === "descargar") return new Date(desde.getTime() + HORA);
  if (tipo === "cambiar_clave_admin" || tipo === "cambiar_servidor" || tipo === "anadir_consola") return new Date(desde.getTime() + 7 * 24 * HORA);
  return new Date(desde.getTime() + 24 * HORA);
}

/** RFC 3339 con la zona local («2026-10-02T10:00:00-05:00»), como pide el contrato. */
export function rfc3339(d: Date): string {
  const p = (n: number) => String(Math.trunc(Math.abs(n))).padStart(2, "0");
  const off = -d.getTimezoneOffset();
  const zona = `${off >= 0 ? "+" : "-"}${p(off / 60)}:${p(off % 60)}`;
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}T${p(d.getHours())}:${p(d.getMinutes())}:${p(d.getSeconds())}${zona}`;
}

export interface Autorizacion {
  prueba_admin: string | null;
  clave_repo: { repo: string; contrasena: string } | null;
  /** Solo en `alta`. */
  prueba_codigo?: string | null;
}

export interface OrdenPlana {
  v: 2;
  cliente: string;
  equipo: string;
  seq: number;
  nonce: string;
  emitida: string;
  caduca: string;
  not_before: string | null;
  tipo: string;
  cuerpo: Record<string, unknown>;
  autorizacion: Autorizacion;
  responder_a: string | null;
  /** v1.49: quién la manda (su nombre en esta consola): el equipo lo enseña en sus órdenes en espera y en su historial. */
  por?: string;
}

export interface Preparada {
  /** Lo que va al servidor en claro. */
  meta: { tipo: string; seq: number; caduca: string; not_before: string | null };
  /** El sobre, en base64. */
  sellado: string;
}

/**
 * Construye y sella una orden para el equipo. El JSON en claro (con la
 * contraseña o la prueba) solo existe en esta función: se convierte a bytes,
 * se sella y los bytes se borran.
 */
export function sellarOrden(
  args: {
    cliente: string;
    equipo: { id: string; box_pub: string };
    seq: number;
    tipo: string;
    cuerpo: Record<string, unknown>;
    autorizacion: Autorizacion;
    responderA?: string | null;
    /** Espera mínima en horas (la del equipo, o la del cliente): solo cuenta si es destructiva. */
    esperaHoras: number;
    /** Lo que ya tiene el equipo (p. ej. los destinos del espejo), para decidir si es destructiva. */
    contexto?: ContextoOrden;
    /** v1.49: el nombre de quien la manda (informativo; un agente anterior lo ignora). */
    por?: string | null;
    /**
     * v1.58: el número reservado (`equipo.seq_espera`) para una orden con espera a un agente
     * que no las guarda. Solo se usa si la orden resulta destructiva (lleva espera).
     */
    seqEspera?: number | null;
  },
  ahora = new Date(),
): Preparada {
  const destructiva = esDestructiva(args.tipo, args.cuerpo, args.esperaHoras, args.contexto);
  // Un minuto de margen: el reloj del servidor puede ir algo adelantado.
  const notBefore = destructiva ? new Date(ahora.getTime() + args.esperaHoras * HORA + 60_000) : null;
  // Con espera, un margen amplio para entregarla (v1.58). El agente no acepta más de 7 días entre «emitida» y «caduca».
  const hasta = notBefore ? Math.max(caducidad(args.tipo, notBefore).getTime(), notBefore.getTime() + MARGEN_TRAS_ESPERA_H * HORA) : caducidad(args.tipo, ahora).getTime();
  const caduca = new Date(Math.min(hasta, ahora.getTime() + 7 * 24 * HORA));
  const seq = notBefore && args.seqEspera ? args.seqEspera : args.seq;
  const plana: OrdenPlana = {
    v: 2,
    cliente: args.cliente,
    equipo: args.equipo.id,
    seq,
    nonce: aB64(aleatorio(16)),
    emitida: rfc3339(ahora),
    caduca: rfc3339(caduca),
    not_before: notBefore ? rfc3339(notBefore) : null,
    tipo: args.tipo,
    cuerpo: args.cuerpo,
    autorizacion: args.autorizacion,
    responder_a: args.responderA ?? null,
  };
  const por = (args.por ?? "").trim().slice(0, 60);
  if (por) plana.por = por;
  const bytes = utf8(JSON.stringify(plana));
  try {
    return {
      meta: { tipo: plana.tipo, seq: plana.seq, caduca: plana.caduca, not_before: plana.not_before },
      sellado: sellarB64(args.equipo.box_pub, bytes),
    };
  } finally {
    bytes.fill(0);
  }
}
