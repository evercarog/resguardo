// Tipos de la API de Resguardo Server v1 (docs/api-servidor.md). Los nombres
// de los campos son los del contrato, tal cual.

export type Rol = "propietario" | "administrador" | "tecnico" | "lectura";

export interface Servidor {
  version: string;
  nombre: string;
  /** Ed25519 pública del servidor (base64). */
  identidad: string;
  huella_ca: string;
  inicializado: boolean;
  /** App key pública de la app «Resguardo» de Dropbox, si el servidor la configura (ver nubes.ts). */
  dropbox_app_key?: string | null;
  /** v1.17: ¿puede dar el instalador del agente «listo para vincular»? (sin el campo: no). */
  instalador_agente?: boolean;
  /** v1.19: ¿hay un Resguardo Agente en la máquina del servidor? («Vincular este servidor»). */
  agente_local?: boolean;
  /** v1.34: la dirección para los agentes y las otras consolas si no es la de esta consola
   * (consola en internet: https://agentes.<dominio>, con la autoridad TLS propia). */
  url_agentes?: string | null;
  /** v1.34: consola en internet (cuotas por cliente más estrictas por defecto). */
  publico?: boolean;
  /** v1.39: canal en vivo de la consola (`GET /api/clientes/{c}/vivo`, WebSocket). */
  vivo?: boolean;
}

/** v1.34: cuotas de un cliente. null = la predeterminada; 0 = sin límite. */
export interface Cuotas {
  equipos: number | null;
  /** Entradas del historial que se guardan por equipo. */
  historial: number | null;
  relevo_mb_mes: number | null;
  ordenes_min: number | null;
}

/** v1.34: un cliente en «Clientes del servidor» (solo cifras). */
export interface ClienteDelServidor {
  id: string;
  nombre: string;
  creado: string;
  personas: number;
  propietarios: number;
  soy_miembro: boolean;
  equipos: number;
  equipos_confirmados: number;
  equipos_conectados: number;
  historial: number;
  ordenes_30d: number;
  ultima_actividad: string | null;
  bytes: number;
  relevo_mes_bytes: number;
  cuotas: Cuotas;
  efectivas: Cuotas;
}

export interface ClientesDelServidor {
  publico: boolean;
  mes: string;
  predeterminadas: Cuotas;
  de_fabrica: Cuotas;
  clientes: ClienteDelServidor[];
}

/** v1.23: «Copia de la consola» (GET/PUT /api/servidor/respaldo, solo el propietario del servidor). */
export interface RespaldoConsola {
  activo: boolean;
  /** Cuándo se puso la clave de respaldo (null: aún no). */
  clave_puesta: string | null;
  sal: string | null;
  conservar: number;
  /** «HH:MM», hora del servidor. */
  hora: string;
  /** Carpeta del servidor donde deja las copias. */
  carpeta: string;
  ultima: { cuando: string; ok: boolean; mensaje: string; archivo: string | null; bytes: number | null; motivo: string } | null;
  proxima: string | null;
  copias: { archivo: string; bytes: number }[];
  identidad: string;
}

export interface Totp {
  secreto: string;
  uri: string;
}

export interface Cuenta {
  id: string;
  correo: string;
  nombre: string;
  superusuario: boolean;
  clientes: { id: string; nombre: string; rol: Rol }[];
}

/** v1.27: un propietario restableció la verificación en dos pasos de la cuenta. */
export interface Restablecida {
  cuando: string;
  /** Nombre de quien la restableció. */
  por: string;
}

export type RespuestaSesion =
  | { necesita: "totp" }
  | { necesita: "alta_totp"; totp: Totp; restablecida?: Restablecida }
  | { necesita: "restablecimiento"; restablecida: Restablecida; caducado: boolean };

export interface RespuestaTotp {
  cuenta: Cuenta;
  codigos_recuperacion?: string[];
  restablecida?: Restablecida;
}

/** v1.27: el código de un solo uso que el propietario le pasa a quien perdió el móvil. */
export interface CodigoRestablecimiento {
  codigo: string;
  caduca: string;
}

export interface ClienteResumen {
  id: string;
  nombre: string;
  rol: Rol;
  equipos: number;
  avisos: number;
  /** v1.32 (con un servidor anterior no llega). */
  marca?: MarcaCliente;
}

/** Marca de un cliente (v1.32): un acento de los de la consola y el logo (PNG, servido aparte). */
export interface MarcaCliente {
  acento: "teal" | "blue" | "indigo" | "violet" | "rose" | "amber" | "graphite" | null;
  /** URL del PNG con su versión (`/api/clientes/{c}/marca/logo?v=…`), o null. */
  logo: string | null;
  actualizada?: string | null;
  por?: string | null;
}

export interface Cliente {
  id: string;
  nombre: string;
  /** Sal pública del cliente (base64) para K_cfg y K_exp. */
  sal_cliente: string;
  espera_min_horas: number;
  rol: Rol;
  /** v1.32. */
  marca?: MarcaCliente;
}

export interface Miembro {
  cuenta: string;
  correo: string;
  nombre: string;
  rol: Rol;
}

export interface Invitacion {
  enlace: string;
  caduca: string;
}

// ---------------------------------------------------------------------------
// Equipos
// ---------------------------------------------------------------------------

/**
 * Parte en claro de la configuración (copias y horarios, sin rutas).
 * El contrato no fija su forma todavía; esto es lo que la consola sabe pintar
 * (ver «Preguntas» en consola/README.md). Todo es opcional.
 */
/** v1.35: una consola (Resguardo Server) que gestiona el equipo (docs/consolas-multiples.md). */
export interface ConsolaDelEquipo {
  id: string;
  nombre: string;
  url: string;
  identidad: string;
  /** La sal del cliente en esa consola, si el equipo la sabe (para la K_cfg al cambiar la clave). */
  sal_cliente: string | null;
  /** Redondeado a 15 min. */
  ultimo_contacto: string | null;
  desde: string | null;
  /** La consola que recibe este resumen (esta). */
  esta: boolean;
}

export interface ResumenEquipo {
  /** v1.28: lo nuevo que entiende el agente («retencion_plazos», «verificacion_auto», «almacen_propio»; v1.36 «escritorio»). */
  admite?: string[];
  /** v1.36: la ventana y los avisos en el equipo (docs/agente-ventana.md), y cuándo se cambiaron allí. */
  escritorio?: Escritorio | null;
  escritorio_cambiado_en_equipo?: string | null;
  /** v1.19 (agente ≥ 0.7.7): un puerto libre para «Este equipo guarda copias». */
  puerto_libre?: number | null;
  /** v1.6: servidores de respaldo que tiene el equipo y tras cuántos días se iría. */
  servidores_respaldo?: { url: string; identidad_corta: string }[];
  respaldo_dias?: number;
  /** v1.6: cambio de servidor en curso (si lo hay). */
  traslado?: { estado: "en_marcha"; hacia: string; hasta: string } | null;
  /** v1.36: las consolas que gestionan el equipo (la que lo recibe, `esta: true`). */
  consolas?: ConsolaDelEquipo[];
  /** v1.36: el último cambio (configuración, repositorios…) y desde qué consola llegó. */
  cambio_config?: { tipo: string; cuando: string; consola: { nombre: string; url: string; identidad: string } } | null;
  copias?: CopiaResumen[];
  repositorios?: RepositorioResumen[];
  destinos?: DestinoResumen[];
  guarda_copias?: {
    activo: boolean;
    puerto?: number;
    solo_red_local?: boolean;
    usuarios?: number;
    /** v1.21: la carpeta donde guarda (en ese equipo). */
    carpeta?: string;
    /** v1.31: libre y total del volumen de esa carpeta. */
    espacio?: EspacioVolumen | null;
    /** v1.21: los repositorios de cada equipo cliente (`<carpeta>/<usuario>/<repo>`), solo nombres. */
    repositorios?: { usuario: string; repos: string[] }[];
    /**
     * Espejo nocturno de lo que guarda (solo añade), v1.9: en otras carpetas o en
     * nubes conectadas en el equipo. `resultado` con error empieza por «ERROR:».
     */
    espejo?: {
      hora: string;
      ultima?: string | null;
      resultado?: string | null;
      limite_kib?: number | null;
      destinos?: { tipo: "carpeta" | "nube"; carpeta?: string | null; nube?: string | null; ultima?: string | null; resultado?: string | null; espacio?: EspacioVolumen | null }[];
    } | null;
    /** Nubes conectadas en el equipo (solo nombre y tipo: nunca tokens). */
    nubes?: { nombre: string; tipo: "dropbox" | "drive" }[];
    /**
     * v1.22: la retención que aplica este almacén en local, por repositorio
     * (`<carpeta>/<usuario>/<repo>`). Un agente anterior no manda la lista:
     * entonces no puede aplicarla.
     */
    retenciones?: RetencionAlmacen[];
  } | null;
  pausado_hasta?: string | null;
}

/** v1.31: bytes libres y totales de un volumen o de una nube, y cuándo se leyeron. */
export interface EspacioVolumen {
  libre: number;
  total: number;
  leido?: string | null;
}

export interface CopiaResumen {
  id: string;
  nombre: string;
  repo: string;
  /** «Cada día a las 13:00 y 19:00», ya en frase, o el horario estructurado. */
  horario?: string | Horario;
  carpetas?: number;
  ultima?: { cuando: string; estado: "ok" | "aviso" | "fallo"; bytes?: number; mensaje?: string | null } | null;
  proxima?: string | null;
  activa?: boolean;
  /** v1.16 (agente ≥ 0.7.7): si guarda versión solo cuando algo cambió. */
  solo_si_cambios?: boolean;
}

export interface RepositorioResumen {
  id: string;
  nombre: string;
  destino: string;
  versiones?: number;
  bytes?: number;
  ultima_version?: string | null;
  verificado?: string | null;
  prueba_restauracion?: string | null;
  retencion?: string | null;
  /** v1.28: la regla tal cual (para editarla); `retencion` es su texto. */
  retencion_regla?: Regla | null;
  /** v1.28: la verificación automática (cada N días, porcentaje rotativo) y cuándo toca. */
  verificacion_auto?: { cada_dias: number; porcentaje: number; horario?: Horario | null; proxima?: string | null; todo_leido?: string | null } | null;
  /** Importado de otro equipo (§10): se puede explorar y restaurar, no copiar en él. */
  solo_lectura?: boolean;
  /** v1.14: en un rest-server de solo añadir (adoptado o comprobado): la retención la aplica el servidor. */
  solo_anadir?: boolean | null;
  /** Copia externa diaria (restic copy) a otro destino: su nombre y la hora. */
  externa?: { destino: string; destino_id?: string; hora: string } | null;
  /** v1.22: su carpeta en el servidor rest, si no es su id (uno adoptado). */
  ruta?: string | null;
}

/**
 * v1.28: plazos de una retención (como `restic forget --keep-within-hourly 15d`):
 * la última versión de cada hora, día… dentro de ese tiempo, contado desde la
 * versión más reciente. Duraciones de restic: «15d», «6m», «1y», «48h», «1y6m».
 */
export interface Plazos {
  horarias?: string | null;
  diarias?: string | null;
  semanales?: string | null;
  mensuales?: string | null;
  anuales?: string | null;
}

/** Cuántas versiones guarda una retención (-1: todas las de ese tipo, «siempre», v1.28). */
export interface Regla {
  /** v1.28. */
  horarias?: number;
  diarias: number;
  semanales: number;
  mensuales: number;
  anuales: number;
  /** v1.28. */
  plazos?: Plazos;
}

/** Días (1 = lunes … 7 = domingo) y hora local («03:00»). */
export interface HorarioRetencion {
  dias: number[];
  hora: string;
  /** v1.40 (almacén con `admite: "retencion_almacen_horario"`): si hay, mandan ellas. */
  reglas?: ReglaHorario[];
}

/** v1.22: una retención que aplica un almacén (sin su clave). */
export interface RetencionAlmacen {
  usuario: string;
  repo: string;
  retencion: Regla;
  texto?: string;
  horario: HorarioRetencion;
  horario_texto?: string;
  verificar?: boolean;
  /** ¿Abre su clave el repositorio? «pendiente»: el equipo dueño aún no la añadió. */
  clave?: "ok" | "pendiente" | "sin_probar";
  ultima?: string | null;
  resultado?: "ok" | "fallo" | null;
  mensaje?: string | null;
  versiones?: number | null;
  proxima?: string | null;
}

export interface DestinoResumen {
  id: string;
  nombre: string;
  tipo: "local" | "rest" | "s3" | "b2" | "sftp" | "otro";
  /** Servidor o bucket, sin credenciales. */
  donde?: string;
  inmutable?: boolean;
  equipo_almacen?: string | null;
  /** v1.41, solo un destino local: su unidad («D:», solo Windows), si es extraíble (USB…; null: no se sabe) y si es de la red. Nunca la ruta. */
  unidad?: string | null;
  extraible?: boolean | null;
  red?: boolean;
}

export interface Equipo {
  id: string;
  nombre: string;
  so: string;
  version_agente: string;
  box_pub: string;
  sign_pub: string;
  sal_equipo: string;
  etiqueta: string | null;
  rol: "agente" | "almacenamiento";
  /** «trasladado»: se fue a otro servidor (cambiar_servidor). */
  modo: "gestionado" | "local" | "trasladado";
  confirmado: boolean;
  conectado: boolean;
  ultimo_contacto: string | null;
  estado_servicio: "en_marcha" | "detenido_por_admin" | null;
  siguiente_seq: number;
  /** La espera que confirmó el equipo (null: la del cliente). */
  espera_min_horas?: number | null;
  resumen: ResumenEquipo | null;
  /** v1.18: etiquetas libres para agrupar (en claro). No es `etiqueta` (el HMAC). */
  etiquetas?: string[];
}

export interface EquipoDetalle extends Equipo {
  ultimo_informe?: Informe | null;
}

export type EstadoEmparejamiento = "abierto" | "unido" | "confirmado" | "cancelado" | "caducado";

export interface Emparejamiento {
  id: string;
  codigo: string;
  caduca: string;
}

export interface EstadoDeEmparejamiento {
  estado: EstadoEmparejamiento;
  caduca: string;
  equipo?: { id: string; nombre: string; so: string; box_pub: string; sign_pub: string; sal_equipo: string };
  sas?: string;
  /** v1.26: 3 si el equipo calcula el SAS con la huella de la autoridad TLS; 2 (o nada), un agente anterior a 0.7.10. */
  sas_version?: number;
  /** Preparados (v1.17): nombre previsto, sistema y, mientras sirve, el código (para el alta). */
  nombre?: string;
  so?: "windows" | "linux";
  codigo?: string;
}

/** Un equipo preparado (v1.17): instalador listo o línea de Linux, con su código de 24 h. */
export interface Preparado {
  id: string;
  nombre: string;
  so: "windows" | "linux";
  estado: EstadoEmparejamiento;
  caduca: string;
  creado: string;
  equipo: string | null;
}

/** Lo que devuelve preparar la línea de Linux. */
export interface PreparadoLinux {
  id: string;
  nombre: string;
  so: "linux";
  codigo: string;
  caduca: string;
  servidor: string;
  huella_ca: string;
}

// ---------------------------------------------------------------------------
// Órdenes
// ---------------------------------------------------------------------------

export type EstadoOrden = "pendiente" | "entregada" | "en_marcha" | "hecha" | "fallida" | "rechazada" | "cancelada" | "caducada";

export interface Orden {
  id: string;
  tipo: string;
  seq: number;
  emitida: string;
  emitida_por: { id: string; nombre: string };
  not_before: string | null;
  caduca: string;
  estado: EstadoOrden;
  mensaje: string | null;
  detalle: string | null;
  firma_agente: string | null;
  actualizada: string;
  /** En `GET /ordenes?pendientes=1` (de todo el cliente) hace falta saber de qué equipo es. */
  equipo?: string;
}

export interface NuevaOrden {
  tipo: string;
  seq: number;
  sellado: string;
  caduca: string;
  not_before: string | null;
  sesion: string | null;
  relevo: { id: string; max_bytes: number } | null;
}

// ---------------------------------------------------------------------------
// Configuración, informes, avisos y auditoría
// ---------------------------------------------------------------------------

export interface ConfigCifrada {
  seq: number;
  cifrado: string;
  resumen: ResumenEquipo | null;
}

export interface Resumen {
  equipos: Equipo[];
  avisos_abiertos: number;
  pendientes: number;
}

/** El informe del agente (sin rutas). La forma la decide el agente; la consola pinta lo que reconoce. */
// ---------------------------------------------------------------------------
// Informe detallado por repositorio (api-servidor.md v1.7 §6, RepoInforme).
// Solo metadatos: sin rutas ni nombres de archivos.
// ---------------------------------------------------------------------------

export interface VersionInforme {
  /** 8 hex. */
  id: string;
  hora: string;
  copia: string | null;
  total_bytes: number | null;
  anadido: number | null;
  anadido_empaquetado: number | null;
  archivos_nuevos: number | null;
  archivos_cambiados: number | null;
  archivos_sin_cambios: number | null;
  duracion_s: number | null;
  etiquetas: string[];
}

export interface EjecucionInforme {
  hora: string;
  copia: string | null;
  resultado: "ok" | "aviso" | "fallo" | "sin_cambios";
  mensaje_corto: string | null;
  /** v1.12 (agente ≥ 0.7.4): lo que duró y añadió esa vuelta. */
  duracion_s?: number | null;
  anadido?: number | null;
  archivos_nuevos?: number | null;
  archivos_cambiados?: number | null;
  reintento?: boolean;
}

export interface TareaInforme {
  ultima: string | null;
  resultado: "ok" | "aviso" | "fallo";
  mensaje_corto: string | null;
}

export type EstadoComprobacion = "ok" | "aviso" | "fallo" | "desconocido";
export interface ComprobacionProteccion {
  id: "copias" | "borrado" | "externa" | "verificacion" | "restauracion" | "kit" | "retencion" | string;
  estado: EstadoComprobacion;
  etiqueta: string;
  detalle: string;
}

export interface RepoInforme {
  id: string;
  nombre: string;
  solo_lectura?: boolean;
  /** Últimos 60 días, la más reciente primero (≤ 500). */
  versiones: VersionInforme[];
  versiones_leidas: string | null;
  /** Últimos 60 días, la más reciente primero (≤ 400). */
  ejecuciones: EjecucionInforme[];
  espacio: { en_disco_bytes: number | null; sin_comprimir: number | null; ratio: number | null; leido: string | null } | null;
  verificacion: TareaInforme | null;
  prueba_restauracion: TareaInforme | null;
  externa: TareaInforme | null;
  proteccion: { puntuacion: number; total: number; items: ComprobacionProteccion[] } | null;
  /** Se quitaron las entradas más antiguas para que el informe quepa (~200 KiB). */
  recortado?: boolean;
}

export interface Informe {
  recibido: string;
  datos: {
    repos?: RepoInforme[];
    version?: string;
    copias?: { id: string; repo?: string; nombre?: string; estado?: string; cuando?: string; bytes?: number; archivos?: number; duracion_s?: number; mensaje?: string | null; ganchos?: ResultadoGancho[] }[];
    /** v1.12: la próxima vez que toca cada copia (null: desactivada o en pausa sin fecha). */
    proximas?: Record<string, string | null>;
    disco?: { libre?: number; total?: number };
    /** v1.25: lo que estaba en marcha al hacer el informe (por si el canal no pasa). */
    progreso?: TareaEnMarcha[];
    [k: string]: unknown;
  };
}

/** Fase de una tarea en marcha (v1.25): las de una copia y `en_marcha` (las demás tareas). */
export type FaseTarea = "antes_de_copiar" | "preparando" | "escaneando" | "subiendo" | "terminando" | "en_marcha";
export type TipoTarea = "copia" | "verificar" | "verificar_externa" | "copia_externa" | "prueba_restauracion";

/** Algo que está en marcha en un equipo, con su progreso (v1.25, `GET …/progreso`). Sin rutas. */
export interface TareaEnMarcha {
  tipo: TipoTarea;
  repo: string;
  /** Id de la copia (solo `tipo: "copia"`). */
  copia?: string | null;
  nombre?: string | null;
  fase: FaseTarea;
  /** Qué hace, en palabras (tareas que no son copias). */
  etapa?: string | null;
  /** De 0 a 1; sin él, aún no se sabe (barra sin porcentaje). */
  porcentaje?: number | null;
  archivos?: number | null;
  archivos_total?: number | null;
  bytes?: number | null;
  bytes_total?: number | null;
  /** Bytes por segundo, suavizado. */
  velocidad?: number | null;
  /** v1.36: lectura real del disco y subida (o escritura) al destino, bytes/s; archivos por segundo. */
  lectura?: number | null;
  subida?: number | null;
  archivos_s?: number | null;
  quedan_s?: number | null;
  versiones?: number | null;
  versiones_total?: number | null;
  empezo?: string | null;
  actualizado?: string | null;
}

export interface ProgresoEquipo {
  equipo: string;
  recibido: string;
  tareas: TareaEnMarcha[];
}

export type TipoAviso =
  | "intentos_fallidos"
  | "bloqueo"
  | "orden_destructiva"
  | "equipo_sin_contacto"
  | "copia_fallida"
  | "copia_atrasada"
  | "servicio_detenido"
  | "cambio_inusual"
  // v1.29: los crea el servidor (de los informes, del resumen y de los resultados).
  | "verificacion_fallida"
  | "externa_fallida"
  | "prueba_fallida"
  | "espejo_fallido"
  | "cambio_clave";

/**
 * v1.23: una entrada del historial que guarda el propio equipo (para siempre:
 * 12 meses con detalle y lo anterior resumido por copia y día) y sube a cada
 * consola nueva: lo de antes de llegar a este servidor y lo que va pasando.
 * Sin rutas. Los campos que no vienen al caso no están.
 */
export interface EntradaHistorial {
  id: string;
  hora: string;
  tipo: "copia" | "resumen_dia" | "verificacion" | "prueba_restauracion" | "externa" | "espejo" | "aviso";
  repo?: string;
  /** Id de la copia (solo «copia»). */
  copia?: string;
  resultado?: "ok" | "aviso" | "fallo" | "sin_cambios";
  mensaje?: string;
  duracion_s?: number;
  anadido?: number;
  archivos_nuevos?: number;
  archivos_cambiados?: number;
  reintento?: boolean;
  ganchos?: ResultadoGancho[];
  /** Tipo de aviso (solo «aviso»). */
  aviso?: TipoAviso;
  /** Solo «resumen_dia» (vueltas de una copia de un día de hace más de un año). */
  dia?: string;
  ok?: number;
  fallidas?: number;
  sin_cambios?: number;
  ultimo_error?: string;
}

export interface Aviso {
  id: string;
  equipo: string | null;
  tipo: TipoAviso;
  mensaje: string;
  creado: string;
  /** Nombre de quien lo marcó como visto. */
  visto_por: string | null;
}

export interface EntradaAuditoria {
  n: number;
  creado: string;
  /** «cuenta:correo», «equipo:id» o «servidor». */
  actor: string;
  accion: string;
  objetivo: string;
  /** JSON en texto. */
  datos: string;
  hash: string;
  prev_hash: string;
}

export type VerificacionAuditoria = { ok: true; entradas: number } | { ok: false; rota_en: number };

// ---------------------------------------------------------------------------
// Sesiones y relé
// ---------------------------------------------------------------------------

export interface MensajeSesion {
  n: number;
  de: "equipo" | "consola";
  cifrado: string;
}

export interface Relevo {
  estado: "subiendo" | "listo" | "caducado";
  trozos: number;
  bytes: number;
}

// ---------------------------------------------------------------------------
// Configuración declarativa (dentro de `config`, cifrada con K_cfg)
// Propuesta de la consola; el agente es quien manda (ver README, «Preguntas»).
// ---------------------------------------------------------------------------

export interface Horario {
  /** 1 = lunes … 7 = domingo. */
  dias: number[];
  /** «HH:MM», hora local del equipo. */
  horas: string[];
  /**
   * v1.24 (agente ≥ 0.7.9): reglas que se suman. Si hay alguna, mandan ellas;
   * `dias`/`horas` llevan lo mismo desplegado cuando se puede (para consolas
   * anteriores) o van vacíos.
   */
  reglas?: ReglaHorario[];
}

/** Una regla del horario (días: 1 = lunes … 7 = domingo; horas «HH:MM» del equipo). */
export type ReglaHorario =
  | { tipo: "horas"; dias: number[]; horas: string[] }
  /** `cada_min`: 5, 10, 15, 20, 30 u horas enteras (60, 120 … 1440). */
  | { tipo: "intervalo"; dias: number[]; cada_min: number; desde: string; hasta: string }
  /** Cada `cada` días (1 a 365) desde `inicio` («AAAA-MM-DD», la primera vez). */
  | { tipo: "cada_dias"; cada: number; inicio: string; hora: string }
  /** El día `dia` de cada mes (1 a 28; -1 = el último). */
  | { tipo: "mensual"; dia: number; hora: string };

export interface CopiaConfig {
  id: string;
  nombre: string;
  repo: string;
  carpetas: string[];
  exclusiones: string[];
  horario: Horario;
  activa: boolean;
  /**
   * Ganchos de plantilla (v1.10, agente ≥ 0.7.2): null, uno o una lista de
   * hasta 4. El agente rechaza la configuración entera si hay otro tipo o campo.
   */
  gancho?: Gancho | Gancho[] | null;
  /** «Solo guardar si hay cambios» (v1.16, agente ≥ 0.7.7). Sin el campo: encendido. */
  solo_si_cambios?: boolean;
}

/** Volcado COPY_ONLY de bases de SQL Server antes de copiar (la carpeta entra en la copia y los volcados se borran después). */
export interface GanchoSqlserver {
  tipo: "sqlserver";
  /** «.» (la predeterminada), «EQUIPO\INSTANCIA» o «EQUIPO,puerto». */
  instancia?: string;
  bases: string[];
  carpeta: string;
}
/** Aviso si el archivo más nuevo de la carpeta (copias propias de una aplicación) es más viejo que `horas`. */
export interface GanchoCarpetaReciente {
  tipo: "carpeta_reciente";
  carpeta: string;
  horas: number;
  extension?: string | null;
}
export type Gancho = GanchoSqlserver | GanchoCarpetaReciente;
/** Resultado de un gancho en el informe (`copias[].ganchos`). */
export interface ResultadoGancho {
  tipo: "sqlserver" | "carpeta_reciente";
  estado: "ok" | "aviso" | "fallo";
  mensaje: string | null;
}

/** v1.28: «Verificar automáticamente cada `cada_dias` días, `porcentaje` % de los datos (rotativa)». */
export interface VerificacionAuto {
  cada_dias: number;
  porcentaje: number;
  /**
   * v1.40 (agente con `admite: "verificacion_horario"`): el mismo horario que
   * el de las copias; entonces manda él y `cada_dias` es para un agente anterior.
   */
  horario?: Horario | null;
}

export interface Configuracion {
  v: 1;
  copias: CopiaConfig[];
  repositorios: { id: string; nombre: string; destino: string; retencion?: Regla | null; solo_lectura?: boolean; externa?: { destino: string; hora: string } | null }[];
  destinos: DestinoResumen[];
  /** Sin uso (el agente lo guarda tal cual). */
  verificacion?: { cada_dias: number; porcentaje: number } | null;
  /** v1.28: verificación automática por repositorio. Sin el campo, el agente no toca la que haya. */
  verificaciones?: Record<string, VerificacionAuto>;
  bandeja?: { visible: boolean; avisos: boolean };
  /** v1.36: la ventana y los avisos en el equipo (docs/agente-ventana.md). */
  escritorio?: Escritorio;
  /** v1.36: lo puso el equipo al cambiar algo con la clave en su ventana. */
  cambiado_en_equipo?: string;
}

/** v1.36: la ventana del agente y sus avisos. */
export interface Escritorio {
  ventana: "off" | "siempre_disponible" | "al_trabajar";
  avisos: "off" | "errores" | "todo";
}

// ---------------------------------------------------------------------------
// Cambiar de servidor, respaldo y exportar (F6, api-servidor.md §11)
// ---------------------------------------------------------------------------

/** Lo que un equipo necesita para irse a otro servidor (o tenerlo de respaldo). */
export interface DatosServidor {
  identidad: string;
  ca_pem: string;
}

export interface Ficha {
  ficha: string;
  caduca: string;
  usos: number;
  servidor: DatosServidor;
}

export interface ClienteRecibido extends Ficha {
  cliente: { id: string; nombre: string; sal_cliente: string; espera_min_horas: number };
}

// ---------------------------------------------------------------------------
// Notificaciones (api-servidor.md §13, v1.29)
// ---------------------------------------------------------------------------

export type Severidad = "critico" | "importante" | "informativo";
export type TipoCanal = "correo" | "webhook" | "ntfy" | "telegram";

/** Horas de silencio («HH:MM», hora del servidor). */
export interface Silencio {
  desde: string;
  hasta: string;
  salvo_criticos: boolean;
}

export interface ReglasCanal {
  severidades: Severidad[];
  /** Solo en los del servidor: de qué clientes (null: de todos). */
  clientes: string[] | null;
  silencio: Silencio | null;
  resumen_diario: boolean;
  resumen_semanal: boolean;
}

/** Lo que no es secreto de un canal. */
export interface ConfigCanal {
  host?: string;
  puerto?: number;
  seguridad?: "starttls" | "tls" | "ninguna";
  usuario?: string;
  remitente?: string;
  chat_id?: string;
  /** Webhook y ntfy: el servidor de la dirección (la dirección entera es secreta). */
  servidor?: string;
}

export interface CanalNotif {
  id: string;
  tipo: TipoCanal;
  nombre: string;
  activo: boolean;
  config: ConfigCanal;
  /** Los secretos puestos: nunca su valor, solo «configurado». */
  secretos: Record<string, "configurado">;
  reglas: ReglasCanal;
  completo: boolean;
  actualizado: string;
  por: string;
}

/** Crear o cambiar un canal: `secretos` con los nuevos ("" quita uno); `codigo` si cambia a dónde va. */
export interface CambioCanal {
  tipo?: TipoCanal;
  nombre?: string;
  activo?: boolean;
  config?: ConfigCanal;
  secretos?: Record<string, string>;
  reglas?: ReglasCanal;
  codigo?: string;
}

export interface AjustesNotif {
  url_consola: string | null;
  max_por_hora: number;
  hora_resumen: string;
  dia_semanal: number;
  canales: CanalNotif[];
}

export interface NotifCliente {
  canales: CanalNotif[];
  /** El correo que vale para las personas de este cliente (null: ninguno). */
  correo: { de: "servidor" | "cliente"; nombre: string } | null;
  servidor: { canales: { nombre: string; tipo: TipoCanal; severidades: Severidad[] }[]; url_consola: string | null };
}

export interface EnvioNotif {
  id: string;
  creado: string;
  enviado: string | null;
  canal: { id: string; nombre: string; tipo: TipoCanal } | null;
  ambito: "servidor" | "cliente";
  cliente: string | null;
  destino: string | null;
  tipo: "aviso" | "recuperacion" | "resumen" | "prueba";
  severidad: Severidad;
  titulo: string;
  estado: "pendiente" | "enviado" | "fallido" | "descartado";
  intentos: number;
  siguiente: string | null;
  error: string | null;
  nota: string | null;
}

export interface PrefsNotif {
  inmediatos: Severidad[];
  resumen: boolean;
  /** false: las de su papel (no las ha cambiado nadie). */
  propias: boolean;
}

export interface PersonaNotif {
  cuenta: string;
  nombre: string;
  correo: string;
  rol: Rol;
  preferencias: PrefsNotif;
  silencio: Silencio | null;
  resumen_diario: boolean;
  resumen_semanal: boolean;
}

export interface MisNotif {
  silencio: Silencio | null;
  resumen_diario: boolean;
  resumen_semanal: boolean;
  hora_resumen: string;
  dia_semanal: number;
  clientes: { id: string; nombre: string; rol: Rol; correo: boolean; preferencias: PrefsNotif }[];
}

// --- v1.40: observaciones y comentarios (lib/notas.svelte.ts) -----------------

/** De qué es una nota. `repositorio` y `copia`: «<equipo>/<id>»; `destino`: su id; `cliente`: el id del cliente. */
export type TipoNota = "cliente" | "equipo" | "repositorio" | "copia" | "destino";

export interface IndiceNota {
  tipo: TipoNota;
  objeto: string;
  /** La primera línea de la observación, sin marcas (o null si no tiene). */
  titulo: string | null;
  observacion: boolean;
  comentarios: number;
  actualizada: string;
}

export interface ObservacionNota {
  texto: string;
  actualizada: string;
  /** Nombre de quien la cambió por última vez. */
  por: string;
}

export interface ComentarioNota {
  id: string;
  texto: string;
  autor: { id: string; nombre: string };
  creado: string;
  editado: string | null;
  /** Lo puede cambiar quien mira (su autor, en sus 15 minutos). */
  editable: boolean;
  /** Lo puede borrar (su autor en sus 15 minutos, o un propietario). */
  borrable: boolean;
}

export interface NotasObjeto {
  tipo: TipoNota;
  objeto: string;
  observacion: ObservacionNota | null;
  comentarios: ComentarioNota[];
  minutos_cambio: number;
}

/** Para el paquete de exportación. */
export interface NotasExportadas {
  observaciones: { tipo: TipoNota; objeto: string; texto: string; actualizada: string; por: string }[];
  comentarios: { id: string; tipo: TipoNota; objeto: string; texto: string; autor: string; creado: string; editado: string | null }[];
}
