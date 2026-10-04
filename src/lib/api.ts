// Envoltorios tipados de los comandos definidos en src-tauri/src/lib.rs.
import { invoke } from "@tauri-apps/api/core";

export interface Repo {
  id: string;
  name: string;
  location: string;
  /** Destino (lugar) al que pertenece este repositorio y su nombre (ver docs/destinos.md). */
  place_id?: string | null;
  place_name?: string | null;
  /** Usuario HTTP del servidor REST (su contraseña vive en el almacén del sistema). */
  rest_username?: string | null;
  /** Certificado de CA propio para HTTPS. */
  cacert?: string | null;
  /** Destinos en la nube: ID de la clave (la secreta vive en el almacén del sistema) y región. */
  cloud_key_id?: string | null;
  cloud_region?: string | null;
  /** Planes de copia (qué, con qué etiquetas y cuándo). */
  plans: Plan[];
  /** Política de retención guardada. */
  retention?: Policy | null;
  /** Cada cuántas horas se espera una copia (null: se detecta del historial). */
  expected_hours?: number | null;
  /** Kit de recuperación guardado (cuándo y para qué ubicación). */
  kit?: KitStatus | null;
  /** Nube: el bucket tiene bloqueo de objetos (lo declara el usuario). */
  object_lock?: boolean;
  /** REST: última comprobación de si el servidor es de solo añadir. */
  append_only?: { checked_at: string; append_only?: boolean | null } | null;
}

/** Salud de la protección de un destino (ver `protection::Protection`). */
export interface Protection {
  score: number;
  total: number;
  items: ProtectionItem[];
}

export interface ProtectionItem {
  id: "copias" | "borrado" | "externa" | "verificacion" | "restauracion" | "kit" | "retencion";
  state: "ok" | "warn" | "bad" | "unknown";
  label: string;
  detail: string;
}

/** Salud de la protección. `lastSnapshot`: la versión más reciente que conoce la interfaz. */
export const protectionStatus = (id: string, lastSnapshot: string | null) => invoke<Protection>("protection_status", { id, lastSnapshot });

/** REST: comprueba (sin borrar nada) si el servidor es de solo añadir; como mucho una vez al día salvo con `force`. */
export const probeAppendOnly = (id: string, force = false) => invoke<Repo>("probe_append_only", { id, force });

/** Nube: declarar que el bucket tiene bloqueo de objetos. Pide la contraseña. */
export const setObjectLock = (id: string, on: boolean, password: string) => invoke<Repo>("set_object_lock", { id, on, password });

/** Kit de recuperación guardado (ver `kit::KitStatus`). */
export interface KitStatus {
  saved_at: string;
  location: string;
  /** ID del repositorio de restic, si se conocía. */
  config_id?: string | null;
}

/** Un destino en el kit de recuperación (ver `kit::KitEntry`): sin secretos. */
export interface KitEntry {
  id: string;
  name: string;
  kind: "local" | "rest" | "sftp" | "s3" | "b2" | "azure" | "gs" | "rclone" | "other";
  /** Ubicación de restic sin usuario ni contraseña del servidor. */
  location: string;
  /** REST: ruta del repositorio en la carpeta de datos del servidor. */
  rest_path?: string | null;
  /** REST: el servidor pide usuario y contraseña (no se imprimen). */
  rest_auth: boolean;
  cloud?: {
    provider: string;
    endpoint?: string | null;
    bucket?: string | null;
    prefix?: string | null;
    key_id?: string | null;
    region?: string | null;
  } | null;
  /** ID del repositorio de restic; null si no se pudo leer (`error` dice por qué). */
  config_id?: string | null;
  error?: string | null;
  kit?: KitStatus | null;
}

/**
 * Cuándo se copia un plan (ver src-tauri/src/plans.rs).
 * - mode "at": a las horas de `times`.
 * - mode "every": cada `every_hours` horas desde `from` mientras no pase de `to`.
 */
export interface PlanSchedule {
  /** Días de la semana: 0 = lunes … 6 = domingo. */
  days: number[];
  mode: "at" | "every";
  /** "HH:MM" (modo "at"). */
  times: string[];
  /** 1 a 24 (modo "every"). */
  every_hours: number;
  /** "HH:MM" (modo "every"). */
  from: string;
  to: string;
}

/** Un plan de copia: carpetas, exclusiones, etiquetas y horario. */
export interface Plan {
  /** Vacío en un plan nuevo: el backend le asigna uno. */
  id: string;
  name: string;
  paths: string[];
  excludes: string[];
  tags: string[];
  /** null: solo se copia a mano. */
  schedule: PlanSchedule | null;
  /** Solo guardar una versión si hay cambios (`--skip-if-unchanged`). Falta en planes antiguos: false. */
  skip_unchanged?: boolean;
}

/** Un plan programado tal como lo tiene el agente. */
export interface AgentPlan {
  id: string;
  name: string;
  paths: string[];
  excludes: string[];
  tags: string[];
  schedule: PlanSchedule;
  /** Cuándo se activó (referencia para la primera copia). */
  enabled_at: string;
  skip_unchanged?: boolean;
}

/**
 * Política de retención (ver `retention::Policy`). Cantidades: número de
 * versiones (0: no se usa; -1: todas, `unlimited`). Plazos: duraciones de
 * restic ("15d", "1y"). Las guardadas por versiones anteriores solo traen
 * cantidades y `keep_within`: los campos nuevos pueden faltar.
 */
export interface Policy {
  keep_last: number;
  keep_hourly: number;
  keep_daily: number;
  keep_weekly: number;
  keep_monthly: number;
  keep_yearly: number;
  /** Todas las versiones de este periodo, p. ej. "30d", "6m", "1y". */
  keep_within: string | null;
  /** La última de cada hora, día, semana, mes o año dentro del periodo. */
  keep_within_hourly?: string | null;
  keep_within_daily?: string | null;
  keep_within_weekly?: string | null;
  keep_within_monthly?: string | null;
  keep_within_yearly?: string | null;
  /** Agrupación: null, la de restic (equipo y carpetas); [] un solo grupo; o "host" / "paths" / "tags". */
  group_by?: ("host" | "paths" | "tags")[] | null;
  /** Aplicar solo a versiones con alguna de estas etiquetas, de este equipo o de estas carpetas. */
  filter_tags?: string[];
  filter_host?: string | null;
  filter_paths?: string[];
  /** Nunca borrar las versiones con alguna de estas etiquetas. */
  keep_tags?: string[];
}

export interface RetentionPreview {
  items: { snapshot: Snapshot; keep: boolean; reasons: string[] }[];
  groups: number;
}

export interface Snapshot {
  id: string;
  short_id: string;
  time: string;
  hostname: string;
  username?: string | null;
  paths: string[];
  tags: string[];
  excludes?: string[];
  parent?: string | null;
  program_version?: string | null;
  summary?: SnapshotSummary | null;
}

/** Resumen que restic (0.17+) guarda en cada snapshot. */
export interface SnapshotSummary {
  backup_start?: string | null;
  backup_end?: string | null;
  files_new?: number | null;
  files_changed?: number | null;
  files_unmodified?: number | null;
  dirs_new?: number | null;
  dirs_changed?: number | null;
  /** Datos nuevos que añadió el snapshot, sin comprimir. */
  data_added?: number | null;
  /** Espacio real que ocupó en disco (comprimido y deduplicado). */
  data_added_packed?: number | null;
  total_files_processed?: number | null;
  total_bytes_processed?: number | null;
}

/** Espacio real del repositorio (`restic stats --mode raw-data`). */
export interface RepoStats {
  total_size: number;
  total_uncompressed_size: number;
  compression_ratio: number;
  compression_space_saving: number;
  total_blob_count: number;
  snapshots_count: number;
}

export interface BackupSummary {
  files_new: number;
  files_changed: number;
  files_unmodified: number;
  data_added: number;
  total_files_processed: number;
  total_bytes_processed: number;
  total_duration: number;
  snapshot_id?: string | null;
}

export interface BackupResult {
  summary: BackupSummary | null;
  errors: string[];
  error_count: number;
  incomplete: boolean;
  /** «Solo guardar si hay cambios» y no los había: correcta, sin versión nueva. */
  unchanged?: boolean;
}

/** Payload del evento `backup-progress` (ver src-tauri/src/backup.rs). */
export type BackupProgress =
  | {
      kind: "status";
      repo_id: string;
      percent: number;
      files_done: number;
      total_files: number;
      bytes_done: number;
      total_bytes: number;
      seconds_remaining: number | null;
      current: string | null;
    }
  | { kind: "item_error"; repo_id: string; message: string };

export const BACKUP_PROGRESS_EVENT = "backup-progress";

/** Elemento de un snapshot (`restic ls`). */
export interface Entry {
  name: string;
  /** "dir", "file", "symlink"… */
  kind: string;
  /** Ruta dentro del snapshot, con `/` (en Windows: `/C/Users/...`). */
  path: string;
  size?: number | null;
  mtime?: string | null;
}

export interface RestoreSummary {
  total_files: number;
  files_restored: number;
  files_skipped: number;
  total_bytes: number;
  bytes_restored: number;
  bytes_skipped: number;
  seconds_elapsed: number;
}

export interface RestoreResult {
  summary: RestoreSummary | null;
  errors: string[];
  error_count: number;
  target: string;
}

/** Payload del evento `restore-progress` (ver src-tauri/src/restore.rs). */
export type RestoreProgress =
  | {
      kind: "status";
      repo_id: string;
      percent: number;
      files_done: number;
      total_files: number;
      bytes_done: number;
      total_bytes: number;
      seconds_remaining: number | null;
    }
  | { kind: "item_error"; repo_id: string; message: string };

export const RESTORE_PROGRESS_EVENT = "restore-progress";

export interface RestoreRequest {
  snapshot: string;
  /** Carpeta del snapshot donde están los elementos. */
  dir: string;
  /** Nombres dentro de `dir`; vacío = todo su contenido. */
  names: string[];
  target: string;
  /** Reemplazar archivos existentes (requiere contraseña). */
  overwrite: boolean;
  password?: string | null;
}

export const resticVersion = () => invoke<string>("restic_version");

export const listRepos = () => invoke<Repo[]>("list_repos");

export interface NewRepo {
  name: string;
  location: string;
  password: string;
  /** true: `restic init`; false: conectar a uno existente. */
  create: boolean;
  restUsername?: string | null;
  restPassword?: string | null;
  cacert?: string | null;
  /** Identificador para poder cancelar la comprobación con `cancelAddRepo`. */
  checkId?: string;
  /** Destinos en la nube: claves nuevas… */
  cloudKeyId?: string | null;
  cloudKeySecret?: string | null;
  cloudRegion?: string | null;
  /** …o las de otro destino ya guardado (su id). */
  cloudFrom?: string | null;
  /** rest-server: el usuario y la contraseña del servidor de otro repositorio (su id). */
  restFrom?: string | null;
}

export const addRepo = (repo: NewRepo) => invoke<Repo>("add_repo", { ...repo });

/** Detiene la comprobación de conexión de un `addRepo` en curso. */
export const cancelAddRepo = (checkId: string) => invoke<void>("cancel_add_repo", { checkId });

/** Requiere la contraseña del repositorio. */
/** Datos del kit de recuperación (todos los destinos si no se indica ninguno). Lee el ID de cada repositorio. */
export const recoveryInfo = (ids: string[] | null = null) => invoke<KitEntry[]>("recovery_info", { ids });

/** «Ya lo guardé en un lugar seguro»: anota el kit del destino. */
/** Anota que el kit está guardado. Cambia la configuración: pide la contraseña del destino. */
export const kitConfirm = (id: string, configId: string | null, password: string) =>
  invoke<Repo>("kit_confirm", { id, configId, password });

export const removeRepo = (id: string, password: string) => invoke<void>("remove_repo", { id, password });

export const listSnapshots = (id: string) => invoke<Snapshot[]>("list_snapshots", { id });

/** Guarda todos los planes del repositorio. Requiere la contraseña del repositorio. */
export const setPlans = (id: string, plans: Plan[], password: string) => invoke<Repo>("set_plans", { id, plans, password });

/**
 * Mueve la copia (plan) `plan` del destino `from` al destino `to`. Pide la
 * contraseña de los dos. Devuelve [origen, destino] ya guardados.
 */
export const moveJob = (from: string, plan: string, to: string, passwordFrom: string, passwordTo: string) =>
  invoke<Repo[]>("move_plan", { from, plan, to, passwordFrom, passwordTo });

/** Copia a mano el plan `plan` (su id). */
export const runBackup = (id: string, plan: string) => invoke<BackupResult>("run_backup", { id, plan });

export const cancelBackup = (id: string) => invoke<void>("cancel_backup", { id });

/** Solo comprueba la contraseña del repositorio. */
export const checkPassword = (id: string, password: string) => invoke<void>("check_password", { id, password });

export const listSnapshotDir = (id: string, snapshot: string, dir: string) =>
  invoke<Entry[]>("list_snapshot_dir", { id, snapshot, dir });

/** Una carpeta o un archivo de «Lo que más ocupa». */
export interface SizeItem {
  /** Ruta dentro del snapshot (formato de restic: `/C/Users/…`). */
  path: string;
  /** Tamaño lógico dentro del snapshot. */
  size: number;
  /** Archivos que contiene (carpetas) o 1 (archivos). */
  files: number;
}

export interface Largest {
  total_size: number;
  total_files: number;
  /** Carpetas más grandes (sin las que solo envuelven a otra). */
  folders: SizeItem[];
  files: SizeItem[];
}

/** Carpetas y archivos más grandes de un snapshot. Recorre el snapshot entero: puede tardar minutos. */
export const snapshotLargest = (id: string, snapshot: string, limit: number | null = null) =>
  invoke<Largest>("snapshot_largest", { id, snapshot, limit });

export const inspectTarget =(path: string) => invoke<{ exists: boolean; empty: boolean }>("inspect_target", { path });

export const runRestore = (id: string, req: RestoreRequest) => invoke<RestoreResult>("run_restore", { id, ...req });

export const cancelRestore = (id: string) => invoke<void>("cancel_restore", { id });

/** «Buscar un archivo» (ver src-tauri/src/search.rs). */
export interface SearchRequest {
  /** Nombre o parte del nombre; admite comodines (`*.xlsx`) o una ruta completa. */
  pattern: string;
  /** Buscar solo en estas versiones (vacío: en todas). */
  snapshots?: string[];
  paths?: string[];
  tags?: string[];
}

export interface SearchMatch {
  /** Ruta dentro de la versión (`/C/Users/...`). */
  path: string;
  type: string;
  size?: number | null;
  mtime?: string | null;
}

/** Payload del evento `search-progress`: las coincidencias de una versión. */
export interface SearchProgress {
  repo_id: string;
  /** ID completo de la versión. */
  snapshot: string;
  matches: SearchMatch[];
}

export const SEARCH_PROGRESS_EVENT = "search-progress";

export interface SearchOutcome {
  snapshots: number;
  matches: number;
  /** Se paró al llegar al máximo de coincidencias. */
  truncated: boolean;
}

/** Busca en las versiones del destino; los resultados llegan como eventos mientras busca. */
export const searchFiles = (id: string, req: SearchRequest) => invoke<SearchOutcome>("search_files", { id, req });

export const cancelSearch = (id: string) => invoke<void>("cancel_search", { id });

/** Qué conservaría y qué borraría restic con esta política (`forget --dry-run`). */
export const retentionPreview = (id: string, policy: Policy) =>
  invoke<RetentionPreview>("retention_preview", { id, policy });

export interface CachedStats {
  stats: RepoStats;
  /** Segundos desde 1970 (UTC) en que se calculó. */
  computed_at: number;
  /** true si se reutilizó el cálculo anterior sin ejecutar restic. */
  cached: boolean;
}

/** Espacio en disco. Solo ejecuta restic si la lista de snapshots cambió (o con `force`). */
export const repoStats = (id: string, snapshotIds: string[], force = false) =>
  invoke<CachedStats>("repo_stats", { id, snapshotIds, force });

/** Cambia el nombre que muestra la app. Requiere la contraseña del repositorio. */
export const renameRepo = (id: string, name: string, password: string) =>
  invoke<Repo>("rename_repo", { id, name, password });

export interface Diff {
  changes: { path: string; kind: "added" | "removed" | "modified" | "metadata" | "type" | "other" }[];
  total: number;
  stats: {
    changed_files: number;
    added: { files: number; dirs: number; bytes: number };
    removed: { files: number; dirs: number; bytes: number };
  };
}

/** Qué cambió entre dos snapshots (`restic diff`). */
export const snapshotDiff = (id: string, from: string, to: string) => invoke<Diff>("snapshot_diff", { id, from, to });

/** Frecuencia esperada de copias, en horas (null = automática). Requiere la contraseña. */
export const setExpectedInterval = (id: string, hours: number | null, password: string) =>
  invoke<Repo>("set_expected_interval", { id, hours, password });

/**
 * Horario del agente (ver `agent::Schedule`). weekday: 0 = lunes.
 * Las copias se programan con `plans`; hours/daily/weekly quedan para la
 * verificación y la copia externa (y en copias de versiones anteriores hasta
 * que el agente las convierte en un plan).
 */
export type Schedule =
  /** Copiar según los planes del repositorio que tienen horario. */
  | { kind: "plans" }
  | { kind: "hours"; every: number }
  | { kind: "daily"; time: string }
  | { kind: "weekly"; weekday: number; time: string }
  /** Solo vigilar: las copias las hace otro programa; se esperan cada `every` horas. */
  | { kind: "monitor"; every: number }
  /** Solo copia externa: después de cada copia con versión nueva, como mucho cada `min_minutes`. */
  | { kind: "after_backup"; min_minutes: number };

export interface AgentRun {
  started: string;
  finished: string;
  result: "ok" | "warning" | "error";
  message: string;
  snapshot_id?: string | null;
  data_added?: number | null;
  files_new?: number | null;
  files_changed?: number | null;
  /** Correcta y sin cambios: no se guardó una versión nueva (sin `snapshot_id`). */
  unchanged?: boolean;
}

export interface AgentInfo {
  supported: boolean;
  elevated: boolean;
  task_installed: boolean;
  repos: AgentRepo[];
  /** `runs`: claves "<repoId>#<planId>" para los planes (y "<repoId>" en versiones anteriores). */
  state: { runs: Record<string, AgentRun>; last_tick?: string | null; running?: AgentRunning | null };
  /** Verificaciones y copias externas (proceso de tareas del agente). */
  tasks: TasksState;
  /** Subidas a la nube frenadas por un cambio inusual, por destino. */
  offsite_holds?: Record<string, Hold>;
  /** «Copias a distancia» permitidas en este equipo. */
  remote_backup?: boolean;
  /** «Modo discreto» (null: desactivado). */
  discreet?: Discreet | null;
}

export interface AgentRepo {
  id: string;
  name: string;
  location: string;
  schedule: Schedule;
  enabled_at: string;
  /** Planes programados (con `schedule.kind === "plans"`). */
  plans: AgentPlan[];
  verify?: VerifyConfig | null;
  offsite?: OffsiteConfig | null;
  /** Prueba de restauración programada. */
  restore_test?: RestoreTestConfig | null;
  /** Copias automáticas en pausa (null: no lo están). */
  pause?: AgentPause | null;
}

/** Pausa de las copias automáticas de un destino (ver `agent::Pause`). */
export interface AgentPause {
  /** Cuándo empezó (RFC 3339). */
  since: string;
  /** Cuándo se reanudan solas; null: hasta reanudarlas a mano. */
  until?: string | null;
}

/** Verificación programada (`restic check`). */
export interface VerifyConfig {
  schedule: Schedule;
  /** Porcentaje de los datos que se leen cada vez, al azar (0: solo la estructura). */
  subset_percent: number;
  enabled_at: string;
  /** Rotativa: cada vez una parte fija de N, así se lee todo el repositorio cada N verificaciones (0: no). */
  rotate_parts?: number;
}

/** Prueba de restauración: restaurar unos archivos al azar y comprobarlos (ver `restore_test::RestoreTest`). */
export interface RestoreTestConfig {
  schedule: Schedule;
  /** Archivos que se restauran cada vez. */
  files: number;
  /** Tamaño máximo de todos ellos juntos, en MB. */
  max_mb: number;
  enabled_at: string;
}

/** Activa, cambia o quita (null) la prueba de restauración. Pide contraseña y administrador. */
export const agentSetRestoreTest = (id: string, restoreTest: { schedule: Schedule; files: number; max_mb: number } | null, password: string) =>
  invoke<AgentInfo>("agent_set_restore_test", { id, restoreTest, password });

/** Avance de la verificación rotativa de un destino (ver `tasks::Rotation`). */
export interface Rotation {
  parts: number;
  next_part: number;
  /** Cuándo se completó la última vuelta (todo el repositorio leído). */
  last_full_at?: string | null;
}

/** Copia externa (`restic copy` a otro repositorio). */
export interface OffsiteConfig {
  location: string;
  /** Proveedor ("b2", "wasabi"…) o "destino:<id>" si va a otro destino de la app. */
  provider: string;
  region?: string | null;
  schedule: Schedule;
  retention?: Policy | null;
  limit_upload_kib?: number | null;
  /** Nombre del destino de la app al que se sube (si es uno de ellos). */
  target_name?: string | null;
  enabled_at: string;
  /** Freno ante cambios inusuales (null: sin freno). */
  guard?: Guard | null;
  /** Verificación de la copia en la nube (desde el origen, con las credenciales de la subida). */
  verify?: VerifyConfig | null;
}

/** Umbrales del freno ante cambios inusuales (ver `tasks::Guard`). */
export interface Guard {
  /** Veces lo normal (mediana de las últimas copias). */
  factor: number;
  /** Mínimos absolutos: bytes añadidos y archivos nuevos + cambiados. */
  min_bytes: number;
  min_files: number;
}

/** Subida a la nube frenada por un cambio inusual (ver `tasks::Hold`). */
export interface Hold {
  since: string;
  snapshot_id: string;
  plan_name: string;
  data_added: number;
  files: number;
  /** Lo normal; null si aún hay pocas copias para saberlo. */
  typical_bytes?: number | null;
  typical_files?: number | null;
  backup_finished?: string;
}

/** Tarea del agente en curso (ver `tasks::RunningTask`). Los campos de progreso pueden faltar o ser null. */
export interface RunningTask {
  repo_id: string;
  /** "verify_offsite": verificación de la copia en la nube. */
  kind: "verify" | "offsite" | "verify_offsite" | "restore_test";
  started: string;
  /** Qué está haciendo, en palabras. */
  stage: string;
  /** Versiones subidas (copia externa) o elementos revisados (verificación). */
  done: number;
  total?: number | null;
  /** 0 a 1; en la copia externa, ponderado por lo que ocupa cada versión. */
  percent?: number | null;
  eta_s?: number | null;
  /** Datos subidos y por subir, estimados. */
  bytes_done?: number | null;
  bytes_total?: number | null;
  /** Fecha de la versión que se está subiendo. */
  current_snapshot_time?: string | null;
  updated?: string | null;
}

export interface TasksState {
  /** Claves "verify:<id>" y "offsite:<id>". */
  runs: Record<string, AgentRun>;
  running?: RunningTask | null;
  /** Verificación rotativa de cada destino. */
  rotation?: Record<string, Rotation>;
}

export interface OffsiteCreds {
  /** Contraseña propia del destino; vacía = la misma del repositorio. */
  password?: string | null;
  key_id?: string | null;
  key_secret?: string | null;
}

/** Activa, cambia o quita (null) la verificación. Pide contraseña y administrador. */
export const agentSetVerify = (id: string, verify: { schedule: Schedule; subset_percent: number; rotate_parts?: number } | null, password: string) =>
  invoke<AgentInfo>("agent_set_verify", { id, verify, password });

/** Comprueba el destino y lo crea si no existe: "existing" | "created". */
export const offsitePrepare = (id: string, location: string, region: string | null, creds: OffsiteCreds, password: string) =>
  invoke<"existing" | "created">("offsite_prepare", { id, location, region, creds, password });

/**
 * Activa, cambia o quita (null) la copia externa. `creds` null: se mantienen las guardadas.
 * Con `target` (id de otro destino de la app) se usan su ubicación y sus credenciales: `creds` null,
 * `passwordTarget` obligatoria y sin retención (la gestiona ese destino).
 */
export const agentSetOffsite = (
  id: string,
  offsite: {
    target?: string | null;
    location: string;
    provider: string;
    region: string | null;
    schedule: Schedule;
    apply_retention: boolean;
    /** Velocidad máxima de subida en KiB/s (null: sin límite). */
    limit_upload_kib?: number | null;
    /** Freno ante cambios inusuales (null: sin freno). */
    guard?: Guard | null;
    /** Verificar también la copia en la nube (null: no). */
    verify?: { schedule: Schedule; subset_percent: number; rotate_parts: number } | null;
  } | null,
  creds: OffsiteCreds | null,
  password: string,
  /** Contraseña del destino `target` (obligatoria con `target`). */
  passwordTarget: string | null = null,
) => invoke<AgentInfo>("agent_set_offsite", { id, offsite, creds, password, passwordTarget });

/** «Es normal, reanudar la subida»: quita el freno por un cambio inusual. Requiere la contraseña y administrador. */
export const agentOffsiteResume = (id: string, password: string) => invoke<AgentInfo>("agent_offsite_resume", { id, password });

/** Pide al agente verificar o subir ahora (lo atiende en ≤ 5 min). */
export const agentTaskNow = (id: string, kind: "verify" | "offsite" | "verify_offsite" | "restore_test") => invoke<void>("agent_task_now", { id, kind });

/** Copia que el agente está haciendo ahora (progreso guardado cada pocos segundos). */
export interface AgentRunning {
  repo_id: string;
  /** Plan que se está copiando. */
  plan_id?: string | null;
  started: string;
  percent?: number | null;
  files_done?: number;
  total_files?: number;
  bytes_done?: number;
  total_bytes?: number;
  seconds_remaining?: number | null;
  updated?: string | null;
}

export const agentInfo = () => invoke<AgentInfo>("agent_info");

/** Últimas líneas del registro del agente (las más recientes al final). */
export const agentLog = () => invoke<string[]>("agent_log");
/** Registro detallado, con rutas completas (solo administradores). */
export const agentLogDetail = () => invoke<string[]>("agent_log_detail");
/** Archivos que no se pudieron leer en la última copia automática de un plan («ruta: error»; solo administradores). */
export const agentFailedFiles = (id: string, plan: string) => invoke<string[]>("agent_failed_files", { id, plan });

/** Activa, cambia o desactiva (null) las copias automáticas. Requiere la contraseña y administrador. */
export const agentSetSchedule = (id: string, schedule: Schedule | null, password: string) =>
  invoke<AgentInfo>("agent_set_schedule", { id, schedule, password });

/**
 * Pausa las copias automáticas de un destino hasta `until` (RFC 3339, como mucho a 30 días;
 * null: hasta reanudarlas a mano). Requiere la contraseña y administrador.
 */
export const agentPause = (id: string, until: string | null, password: string) =>
  invoke<AgentInfo>("agent_pause", { id, until, password });

/** Reanuda las copias automáticas de un destino. Requiere la contraseña y administrador. */
export const agentResume = (id: string, password: string) => invoke<AgentInfo>("agent_resume", { id, password });

export const relaunchAsAdmin = () => invoke<void>("relaunch_as_admin");

/** Vuelve a crear la tarea del agente (requiere administrador). */
export const agentRepair = () => invoke<AgentInfo>("agent_repair");

/** Vínculo con Resguardo Web (ver src-tauri/src/web.rs). */
export interface WebInfo {
  link: { url: string; key: string; device_id: string; device_name: string; paired_at: string; revoked: boolean } | null;
  state: { last_report?: string | null; last_error?: string | null };
  elevated: boolean;
  default_url: string;
  default_key: string;
  default_name: string;
}

export const webInfo = () => invoke<WebInfo>("web_info");

/** Operaciones en curso (al intentar cerrar la ventana). */
export interface RunningJob {
  repo_id: string;
  name: string;
  kind: "backup" | "restore";
  can_hand_off: boolean;
}
export const runningJobs = () => invoke<RunningJob[]>("running_jobs");
export const closeApp = (mode: "handoff" | "background" | "stop") => invoke<void>("close_app", { mode });
/** Requiere administrador. */
export const webPair = (url: string, key: string, code: string, name: string) =>
  invoke<WebInfo>("web_pair", { url, key, code, name });
export const webUnpair = () => invoke<WebInfo>("web_unpair");
export const webReportNow = () => invoke<WebInfo>("web_report_now");

/** Requiere la contraseña del repositorio. */
export const setRetention = (id: string, policy: Policy | null, password: string) =>
  invoke<Repo>("set_retention", { id, policy, password });

/** Una entrada del historial de actividad (ver src-tauri/src/history.rs). */
export interface ActivityEntry {
  /** "pause" / "resume": copias automáticas en pausa / reanudadas. */
  /** "guard": cambio inusual (subida frenada) o su reanudación. */
  /** "config": un cambio de configuración del destino o de una copia; "kit": kit de recuperación guardado. */
  kind: "backup" | "verify" | "offsite" | "verify_offsite" | "restore_test" | "pause" | "resume" | "guard" | "config" | "kit" | "share";
  /** "agent": programada o pedida al agente (o el final de una pausa); "retry": reintento; "manual": «Copiar ahora» o a mano. */
  /** "remote": «Copiar ahora» pedido a distancia (desde `requested_from`). */
  origin: "agent" | "retry" | "manual" | "remote";
  requested_from?: string | null;
  repo_id: string;
  repo_name: string;
  plan_id?: string | null;
  plan_name?: string | null;
  started: string;
  finished: string;
  /** "info": pausas y reanudaciones (no es un resultado). */
  result: "ok" | "warning" | "error" | "info";
  message: string;
  snapshot_id?: string | null;
  data_added?: number | null;
  files_new?: number | null;
  files_changed?: number | null;
  /** Copia correcta sin cambios (no se guardó versión). */
  unchanged?: boolean;
  /** Usuario de Windows que lo hizo (copias a mano, cambios y kits). */
  user?: string | null;
}

/** «Copias a distancia»: administrador y la contraseña de un destino del agente. */
export const agentSetRemoteBackup = (id: string, enabled: boolean, password: string) =>
  invoke<AgentInfo>("agent_set_remote_backup", { id, enabled, password });

// ---------- Todos mis equipos (cuenta de Resguardo Web; ver src-tauri/src/account.rs) ----------

export interface AccountStatus {
  signed_in: boolean;
  email: string | null;
  /** Falta el código del autenticador. */
  needs_code: boolean;
  /** Nombre de este equipo (el origen que verán los demás). */
  this_device: string;
}

/** Filas tal como las da la web (PostgREST con la sesión del usuario). */
export interface AccountOverview {
  clients: { id: string; name: string }[];
  devices: RemoteDevice[];
  repos: RemoteRepo[];
  commands: RemoteCommand[];
  this_device: string;
}

export interface RemoteDevice {
  id: string;
  name: string;
  client_id: string | null;
  os?: string | null;
  app_version?: string | null;
  last_seen_at?: string | null;
  remote_backup_enabled?: boolean | null;
}

export interface RemoteRun {
  started?: string;
  finished?: string;
  result?: "ok" | "warning" | "error";
  message?: string;
  unchanged?: boolean;
}

export interface RemoteRepo {
  device_id: string;
  repo_id: string;
  name: string;
  kind: string;
  host?: string | null;
  expected_hours?: number | null;
  snapshots_count?: number | null;
  last_snapshot_at?: string | null;
  last_total_bytes?: number | null;
  last_run?: RemoteRun | null;
  plans?: { id: string; name: string; last_run?: RemoteRun | null }[] | null;
  paused?: boolean;
  offsite_run?: RemoteRun | null;
  offsite_hold?: Record<string, unknown> | null;
  protection?: { score: number; total: number; items: ProtectionItem[] } | null;
  running_since?: string | null;
}

export interface RemoteCommand {
  id: string;
  device_id: string;
  repo_id: string;
  plan_id: string;
  requested_at: string;
  requested_from?: string | null;
  status: "pending" | "claimed" | "done" | "failed" | "expired" | "rejected";
  finished_at?: string | null;
  message?: string | null;
}

export const accountStatus = () => invoke<AccountStatus>("account_status");
export const accountSignIn = (email: string, password: string) => invoke<AccountStatus>("account_sign_in", { email, password });
export const accountVerify = (code: string) => invoke<AccountStatus>("account_verify", { code });
export const accountSignOut = () => invoke<AccountStatus>("account_sign_out");
export const accountOverview = () => invoke<AccountOverview>("account_overview");
/** Pide «Copiar ahora» de un plan en otro equipo. Devuelve el id de la petición. */
export const accountRequestBackup = (device: string, repo: string, plan: string) => invoke<string>("account_request_backup", { device, repo, plan });
export const accountCommand = (id: string) => invoke<RemoteCommand | null>("account_command", { id });

/** Historial combinado del agente y de la app, de lo más reciente a lo más antiguo. */
export const activityHistory = (limit?: number) => invoke<ActivityEntry[]>("activity_history", { limit: limit ?? null });

// ---------- Bloqueo con Windows Hello (ver src-tauri/src/applock.rs) ----------

export type HelloAvailability = "available" | "not_configured" | "disabled_by_policy" | "device_not_present" | "device_busy" | "unsupported";

export interface AppLockStatus {
  enabled: boolean;
  /** Volver a pedirla tras N minutos sin usar la app (null: solo al abrir). */
  idle_minutes: number | null;
  locked: boolean;
  availability: HelloAvailability;
  /** Por qué no se puede usar Windows Hello (vacío si se puede). */
  explain: string;
}

export interface UnlockResult {
  result: "verified" | "canceled" | "retries_exhausted" | "unavailable";
  /** Aviso si no se pudo pedir Windows Hello (la app se abre igualmente). */
  message: string | null;
}

export const appLockStatus = () => invoke<AppLockStatus>("app_lock_status");
export const appLockUnlock = () => invoke<UnlockResult>("app_lock_unlock");
/** Cualquier cambio pide pasar Windows Hello. */
export const appLockSet = (enabled: boolean, idleMinutes: number | null) =>
  invoke<AppLockStatus>("app_lock_set", { enabled, idleMinutes });
export const appLockLock = () => invoke<void>("app_lock_lock");

// ---------- Bandeja del sistema ----------

/** Lo que muestra el icono de la bandeja (ver `tray::TrayStatus`). */
export interface TrayStatus {
  tone: "ok" | "warn" | "bad" | "neutral";
  /** «Resguardo: todo protegido», «Resguardo: 2 cosas necesitan atención»… */
  tooltip: string;
  /** Lo que está en marcha («Copiando «Laboral» · 45 %»). */
  progress: string | null;
  /** Copias de este equipo, para «Copiar ahora» desde la bandeja. */
  copies: { repo_id: string; plan_id: string; label: string }[];
}

export interface TraySettings {
  /** Al cerrar la ventana, Resguardo sigue en la bandeja. */
  close_to_tray: boolean;
  /** Avisos de Windows. */
  notifications: boolean;
  /** Se inicia con Windows (para este usuario), en la bandeja. */
  autostart: boolean;
}

export const trayUpdate = (status: TrayStatus) => invoke<void>("tray_update", { status });
export const traySettings = () => invoke<TraySettings>("tray_settings");
export const traySet = (s: TraySettings) =>
  invoke<TraySettings>("tray_set", { closeToTray: s.close_to_tray, notifications: s.notifications, autostart: s.autostart });

/** Al pulsar un aviso de Windows: qué abrir (ver `avisos::Target`). */
export type OpenView =
  | { kind: "copy"; repoId: string; planId: string }
  | { kind: "destination"; repoId: string }
  | { kind: "kit"; ids: string[] }
  /** «Pausar copias automáticas 1 hora», desde la bandeja. */
  | { kind: "pause" };

/** Vista pedida desde un aviso o la bandeja (una sola vez; null si no hay). */
export const takeView = () => invoke<OpenView | null>("take_view");

// ---------- «Ver versiones en Resguardo» (menú del Explorador) ----------

/** Una ruta de este equipo y las copias que la incluyen (ver `versiones::Located`). */
export interface LocatedPath {
  /** Ruta normalizada (`C:\Carpeta\archivo.ext`). */
  path: string;
  name: string;
  parent: string;
  is_dir: boolean;
  size: number | null;
  mtime: string | null;
  /** "inside": dentro de una carpeta de la copia; "contains": carpeta que contiene carpetas de la copia. */
  found: { repo_id: string; plan_id: string; kind: "inside" | "contains" }[];
}

/** Ruta pedida al arrancar con `--versiones` (solo la primera vez). */
export const versionsPending = () => invoke<string | null>("versions_pending");
/** Un destino (lugar): un bucket, un servidor, una carpeta… con varios repositorios. */
export interface Place {
  id: string;
  name: string;
  /** Clave de agrupación (local). */
  key: string;
}
export const listPlaces = () => invoke<Place[]>("list_places");

/** Un repositorio encontrado (o recordado) en un destino. */
export interface FoundRepo {
  location: string;
  /** Ruta dentro del destino («portatil», «clientes/altamar», «.»). */
  path: string;
  /** El repositorio de este equipo que ya lo usa. */
  repo_id: string | null;
  /** Encontrado al listar (o solo recordado). */
  listed: boolean;
}
export interface PlaceScan {
  found: FoundRepo[];
  /** Por qué no se pudo listar, o qué se muestra en su lugar. */
  note: string | null;
}
/** Lo que hay en un destino (solo lee). */
export const placeScan = (id: string) => invoke<PlaceScan>("place_scan", { id });

// ---------- Compartir un destino entre mis equipos (docs/compartir.md) ----------

export interface ShareStatus {
  /** De la nube o un rest-server: se puede compartir. */
  shareable: boolean;
  shared: boolean;
  since: string | null;
  /** Entregas: [equipo, cuándo]. */
  delivered: [string, string][];
  /** Este equipo está vinculado a Resguardo Web. */
  linked: boolean;
  elevated: boolean;
}
export const placeShareStatus = (id: string) => invoke<ShareStatus>("place_share_status", { id });
/** Compartir o dejar de compartir. Pide administrador y la contraseña de `repo` (un repositorio del destino). */
export const placeShareSet = (id: string, on: boolean, repo: string, password: string) => invoke<ShareStatus>("place_share_set", { id, on, repo, password });

export interface SharedPlace {
  id: string;
  device_id: string;
  device_name: string;
  kind: string;
  host: string | null;
  base: string;
  name: string;
  created_at: string;
}
export interface ShareRequest {
  id: string;
  share_id: string;
  status: "pending" | "delivered" | "received" | "rejected" | "expired" | "cancelled";
  reason?: string | null;
  not_before?: string | null;
  created_at: string;
}
export interface SharesAvailable {
  shares: SharedPlace[];
  requests: ShareRequest[];
  /** Id de este equipo en la web (null si no está vinculado). */
  this_device_id: string | null;
}
/** Lo que comparten mis otros equipos (sesión de «Todos mis equipos»). */
export const sharesAvailable = () => invoke<SharesAvailable>("shares_available");
export const shareRequest = (share: string) => invoke<unknown>("share_request", { share });
/** Cancela una petición mía aún pendiente. */
export const shareCancel = (request: string) => invoke<unknown>("share_cancel", { request });

export interface ReceivedShare {
  id: string;
  meta: { kind: string; host: string | null; base: string; name: string };
  /** Ubicación base del destino (para sugerir la de un repositorio nuevo). */
  base: string;
  from_device: string;
  received_at: string;
}
/** Destinos recibidos (requiere administrador). */
export const sharesReceived = () => invoke<ReceivedShare[]>("shares_received");
/** Crea (`create`) o conecta un repositorio en un destino recibido. */
export const receivedShareAdd = (id: string, name: string, location: string, password: string, create: boolean) =>
  invoke<Repo>("received_share_add", { id, name, location, password, create });

// ---------- Servidor de copias (fase 4) ----------

export interface ServerStatus {
  supported: boolean;
  elevated: boolean;
  /** Por qué no puede arrancar (falta el binario o su huella), o null. */
  binary_problem: string | null;
  enabled: boolean;
  path: string;
  port: number;
  local_subnet_only: boolean;
  running: boolean;
  lan_addresses: string[];
  tls_sha256: string | null;
  users: { name: string; created_at: string; repos: string[]; shared: boolean }[];
  /** Vinculado con Resguardo Web (para ofrecerlo a mis equipos). */
  linked: boolean;
}
export interface NewServerUser {
  status: ServerStatus;
  user: string;
  /** Se muestra una sola vez (también llega cifrada a los equipos que la pidan). */
  password: string;
  location: string;
}
export const serverStatus = () => invoke<ServerStatus>("server_status");
export const serverSetup = (path: string, port: number, localSubnetOnly: boolean) => invoke<ServerStatus>("server_setup", { path, port, localSubnetOnly });
export const serverDisable = () => invoke<ServerStatus>("server_disable");
export const serverAddUser = (name: string) => invoke<NewServerUser>("server_add_user", { name });
export const serverRemoveUser = (name: string) => invoke<ServerStatus>("server_remove_user", { name });
export const serverPublicIp = () => invoke<string>("server_public_ip");

// ---------- Equipos gestionados: consola (fase 5, docs/agente-gestionado.md) ----------

/** Un equipo gestionado por esta consola (sin contraseñas). */
export interface ManagedEndpoint {
  device_id: string;
  name: string;
  /** Su usuario en el Servidor de copias. */
  server_user: string;
  location: string;
  plans: Plan[];
  paired_at: string;
  /** Ya no se gestiona (se le mandó «Dejar de gestionar»). */
  stopped: boolean;
  tray: boolean;
  tray_toasts: boolean;
  /** Retención en el servidor: la aplica esta consola cada semana. */
  retention: ManagedRetention;
  last_prune: string | null;
  last_prune_error: string | null;
  /** De su carpeta en el servidor: la última versión, cuántas hay y cuánto ocupan. */
  last_snapshot: string | null;
  snapshots: number;
  bytes: number;
}
export type ManagedRetention = "frecuente" | "equilibrada" | "ligera" | "todo";
export interface ManagedPairingStart {
  pairing_id: string;
  /** «ABCD-EFGH-JK»: se escribe en el equipo con `resguardo-agente.exe --pair`. */
  code: string;
  expires_at: string;
}
export interface ManagedPairingPoll {
  /** "open" hasta que el equipo se une; "joined" con su código de comprobación; "confirmed", "cancelled" o "expired". */
  status: string;
  expires_at: string | null;
  endpoint_name: string | null;
  sas: string | null;
}
export const managedList = () => invoke<ManagedEndpoint[]>("managed_list");
export const managedPairStart = () => invoke<ManagedPairingStart>("managed_pair_start");
export const managedPairPoll = (pairing: string) => invoke<ManagedPairingPoll>("managed_pair_poll", { pairing });
export const managedPairConfirm = (pairing: string) => invoke<ManagedEndpoint>("managed_pair_confirm", { pairing });
export const managedPairCancel = (pairing: string) => invoke<void>("managed_pair_cancel", { pairing });
export const managedSetConfig = (device: string, plans: Plan[], tray: boolean, trayToasts: boolean) =>
  invoke<ManagedEndpoint>("managed_set_config", { device, plans, tray, trayToasts });
export const managedBackupNow = (device: string, plan: string) => invoke<void>("managed_backup_now", { device, plan });
export const managedUnpair = (device: string) => invoke<ManagedEndpoint>("managed_unpair", { device });
export const managedSetRetention = (device: string, retention: ManagedRetention) => invoke<ManagedEndpoint>("managed_set_retention", { device, retention });
export const managedPruneNow = (device: string) => invoke<void>("managed_prune_now", { device });
export interface AgentInstallerInfo {
  available: boolean;
  version: string;
  file_name: string;
}
export const managedAgentInstaller = () => invoke<AgentInstallerInfo>("managed_agent_installer");
/** Copia el instalador del agente (comprobando su huella) a esa carpeta. */
export const managedSaveAgentInstaller = (folder: string) => invoke<{ path: string; sha256: string }>("managed_save_agent_installer", { folder });
/** Añade (o devuelve) el repositorio del equipo en esta app, para explorarlo y restaurar. */
export const managedOpenRepo = (device: string) => invoke<Repo>("managed_open_repo", { device });

/** Principio del mensaje de un bloqueo antiguo (restic.rs, STALE_LOCK): se ofrece «Desbloquear». */
export const STALE_LOCK = "El repositorio tiene un bloqueo antiguo";
/** `restic unlock`: quita solo los bloqueos antiguos. */
export const unlockRepo = (id: string) => invoke<void>("unlock_repo", { id });
/**
 * Guarda una contraseña nueva del repositorio (comprobando que lo abre). "agente": también
 * para las copias automáticas; "agente-admin": hace falta abrir como administrador para ellas.
 */
export const updateSavedPassword = (id: string, password: string) => invoke<"app" | "agente" | "agente-admin">("update_saved_password", { id, password });

export const CLONE_PROGRESS_EVENT = "clone-progress";
export interface CloneProgress {
  from: string;
  stage: string;
  done: number;
  total: number;
}
/**
 * Clona `from` en una ubicación nueva del destino de `template` (un repositorio suyo, por sus
 * credenciales), con la contraseña nueva `passwordNew`. Pide la contraseña del de origen.
 */
export const cloneRepo = (from: string, template: string, location: string, name: string, passwordNew: string, password: string) =>
  invoke<Repo>("clone_repo", { from, template, location, name, passwordNew, password });
export const cancelClone = (from: string) => invoke<void>("cancel_clone", { from });
/** Solo cambia su nombre visible: no pide contraseña. */
export const renamePlace = (id: string, name: string) => invoke<Place>("rename_place", { id, name });

export const locatePath = (path: string) => invoke<LocatedPath>("locate_path", { path });
/** Restaura una versión con otro nombre junto al original (nunca reemplaza). Devuelve la ruta final. */
export const restoreVersion = (id: string, snapshot: string, path: string, name: string) =>
  invoke<string>("restore_version", { id, snapshot, path, name });
export const shellMenuStatus = () => invoke<boolean>("shell_menu_status");
export const shellMenuSet = (enabled: boolean) => invoke<boolean>("shell_menu_set", { enabled });

// ---------- Modo discreto ----------

/** «Mientras se trabaja, copiar con prioridad baja» (ver `discreto::Discreet`). */
export interface Discreet {
  /** 0 = lunes … 6 = domingo. */
  days: number[];
  from: string;
  to: string;
  /** Límite de subida a destinos remotos mientras dura, en KiB/s. */
  upload_kib?: number | null;
}

/** Activa, cambia o quita (null) el modo discreto. Pide administrador y la contraseña de un destino del agente. */
export const agentSetDiscreet = (id: string, discreet: Discreet | null, password: string) =>
  invoke<AgentInfo>("agent_set_discreet", { id, discreet, password });
