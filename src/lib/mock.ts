// Backend simulado para diseñar la interfaz en un navegador (`npm run dev`).
// Imita los comandos de src-tauri/src/lib.rs con datos de ejemplo.
import { mockIPC } from "@tauri-apps/api/mocks";
import { emit } from "@tauri-apps/api/event";
import type { ActivityEntry, BackupProgress, BackupResult, LocatedPath, Plan, Repo, Snapshot } from "$lib/api";
import { latestPlanSlot, validatePlan } from "$lib/plans";
import { toSnapshotPath } from "$lib/paths";
import { EMPTY_POLICY } from "$lib/retention";
import pkg from "../../package.json";

const HOUR = 3600_000;
const delay = (ms: number) => new Promise((r) => setTimeout(r, ms));
let counter = 0;
const hex = (n = 64) =>
  Array.from({ length: n }, () => "0123456789abcdef"[Math.floor(Math.random() * 16)]).join("");

function snapshot(hoursAgo: number, paths: string[], tags: string[], bytes: number, files: number): Snapshot {
  const id = hex();
  return {
    id,
    short_id: id.slice(0, 8),
    time: new Date(Date.now() - hoursAgo * HOUR).toISOString(),
    hostname: "PORTATIL-ANA",
    username: "ana",
    paths,
    tags,
    summary: {
      backup_start: new Date(Date.now() - hoursAgo * HOUR).toISOString(),
      backup_end: new Date(Date.now() - hoursAgo * HOUR + (40 + (files % 90)) * 1000).toISOString(),
      files_new: files % 57,
      files_changed: files % 23,
      files_unmodified: files - (files % 57) - (files % 23),
      data_added: 40e6 + (files % 97) * 3e6,
      data_added_packed: (40e6 + (files % 97) * 3e6) * 0.68,
      total_bytes_processed: bytes,
      total_files_processed: files,
    },
    program_version: "restic 0.19.1",
  };
}

const DOCS = ["C:\\Users\\Ana\\Documentos", "C:\\Users\\Ana\\Imágenes"];

/** Dos planes de ejemplo: «Laboral» (L–S cada hora de 7 a 19) y «Domingo» (23:00). */
const LABORAL: Plan = {
  id: "laboral",
  name: "Laboral",
  paths: DOCS,
  excludes: ["*.tmp", "node_modules"],
  tags: ["diaria"],
  schedule: { days: [0, 1, 2, 3, 4, 5], mode: "every", times: [], every_hours: 1, from: "07:00", to: "19:00" },
  // Cada hora revisa, pero solo guarda versión si hay cambios.
  skip_unchanged: true,
};
const DOMINGO: Plan = {
  id: "domingo",
  name: "Domingo",
  paths: ["C:\\Users\\Ana"],
  excludes: ["AppData"],
  tags: ["semanal"],
  schedule: { days: [6], mode: "at", times: ["23:00"], every_hours: 1, from: "", to: "" },
};

const repos: Repo[] = [
  { id: "r1", name: "Disco externo", location: "E:\\Copias\\restic", plans: [LABORAL, DOMINGO] },
  {
    id: "r2",
    name: "Servidor de casa",
    location: "rest:https://nas.local:8000/portatil/",
    rest_username: "ana",
    plans: [{ id: "principal", name: "Principal", paths: [DOCS[0]], excludes: [], tags: [], schedule: null }],
    // Servidor de solo añadir y con retención: así su protección no sale vacía.
    append_only: { checked_at: new Date().toISOString(), append_only: true },
    retention: { ...EMPTY_POLICY, keep_daily: 14, keep_weekly: 8, keep_monthly: 12 },
  },
  { id: "r3", name: "NAS por SFTP", location: "sftp:ana@nas.local:/volume1/restic", plans: [] },
  {
    // Destino en la nube con claves guardadas: sirve para probar «Usar las claves de…» y la copia externa hacia otro destino.
    id: "r4",
    name: "Backblaze B2",
    location: "s3:https://s3.us-west-004.backblazeb2.com/copias-ana/portatil",
    cloud_key_id: "004a1b2c3d4e5f60000000001",
    cloud_region: "us-west-004",
    plans: [{ id: "fotos", name: "Fotos", paths: [DOCS[1]], excludes: [], tags: [], schedule: null }],
    // Bucket con bloqueo de objetos (Object Lock).
    object_lock: true,
  },
];

/** Con ?externa en la URL: «Disco externo» sube su copia externa a otro destino de la app, «Disco externo · Nube». */
const EXTERNA = new URLSearchParams(location.search).has("externa");
if (EXTERNA)
  repos.push({
    id: "r5",
    name: "Disco externo · Nube",
    location: "s3:https://s3.us-west-004.backblazeb2.com/copias-ana/disco-externo",
    cloud_key_id: "004a1b2c3d4e5f60000000001",
    cloud_region: "us-west-004",
    plans: [],
    // Su propia retención, más ligera que la del origen.
    retention: { ...EMPTY_POLICY, keep_within_daily: "60d", keep_monthly: -1 },
  });

/** Destinos (lugares), como `places::assign`: se agrupan por bucket, servidor o carpeta padre. */
const places: { id: string; name: string; key: string }[] = [];
function placeKey(location: string) {
  const l = location.trim();
  const m = /^([a-z0-9]+):(.*)$/i.exec(l);
  if (!m || m[1].length === 1) {
    const p = l.replace(/\\/g, "/").replace(/\/+$/, "").toLowerCase();
    const parts = p.split("/").filter(Boolean);
    const min = p.startsWith("//") ? 2 : 1;
    return parts.length <= min ? p : (p.startsWith("//") ? "//" : "") + parts.slice(0, -1).join("/");
  }
  const [, kind, rest] = m;
  if (kind === "s3") {
    const r = rest.replace(/^https?:\/\//, "").split("/");
    return `s3:${r[0].replace(/^.*@/, "").toLowerCase()}/${r[1] ?? ""}`;
  }
  if (kind === "rest") return `rest:${/^https/.test(rest) ? "https" : "http"}://${rest.replace(/^https?:\/\//, "").split("/")[0].replace(/^.*@/, "").toLowerCase()}`;
  if (kind === "sftp") return `sftp:${rest.replace(/^\/\//, "").split(/[:/]/)[0].toLowerCase()}`;
  return `${kind}:${rest.split(":")[0]}`;
}
function placeName(location: string) {
  const k = placeKey(location);
  if (k.startsWith("s3:")) {
    const [host, bucket] = k.slice(3).split("/");
    return `${host.endsWith("backblazeb2.com") ? "Backblaze B2" : host.endsWith("wasabisys.com") ? "Wasabi" : "Nube S3"} · ${bucket}`;
  }
  if (k.startsWith("rest:")) return `Servidor ${k.split("://")[1].split(":")[0]}`;
  if (k.startsWith("sftp:")) return `SFTP ${k.slice(5).split("@").pop()}`;
  if (k.startsWith("//")) return k.slice(2).split("/").join(" · ");
  if (/^[a-z]:/.test(k)) return `Unidad ${k[0].toUpperCase()}:`;
  return "Repositorio";
}
function assignPlaces() {
  for (const r of repos) {
    if (!r.place_id || !places.some((p) => p.id === r.place_id)) {
      const key = placeKey(r.location);
      let p = places.find((x) => x.key === key);
      if (!p) places.push((p = { id: `place-${places.length + 1}`, name: placeName(r.location), key }));
      r.place_id = p.id;
    }
  }
  for (let i = places.length - 1; i >= 0; i--) if (!repos.some((r) => r.place_id === places[i].id)) places.splice(i, 1);
  for (const r of repos) r.place_name = places.find((p) => p.id === r.place_id)?.name ?? null;
}

/** Contraseñas simuladas: los destinos de ejemplo usan "resguardo". */
const MOCK_PASSWORD = "resguardo";
const passwords: Record<string, string> = { r1: MOCK_PASSWORD, r2: MOCK_PASSWORD, r3: MOCK_PASSWORD, r4: MOCK_PASSWORD, r5: MOCK_PASSWORD };

async function checkPassword(id: string, password: string) {
  await delay(450);
  if (passwords[id] !== password) throw "Contraseña incorrecta.";
}

const snapshots: Record<string, Snapshot[]> = {
  r1: [
    ...Array.from({ length: 70 }, (_, i) => snapshot(2 + 24 * (i + 1) + (i % 3), DOCS, [], 4.2e9 - i * 1.3e7, 18_400 - i * 20)).filter(
      (_, i) => i % 5 !== 3,
    ),
    snapshot(2, DOCS, ["diaria"], 4.21e9, 18_402),
    snapshot(26, DOCS, ["diaria"], 4.19e9, 18_377),
    snapshot(50, DOCS, ["diaria"], 4.18e9, 18_350),
    snapshot(74, DOCS, ["diaria", "semanal"], 4.1e9, 18_121),
    snapshot(24 * 9, DOCS, ["semanal"], 3.9e9, 17_804),
    snapshot(24 * 16, DOCS, ["semanal"], 3.7e9, 17_220),
    snapshot(24 * 40, DOCS, ["mensual"], 3.2e9, 15_990),
    // Copias del plan «Domingo» (toda la carpeta de usuario).
    ...[1, 2, 3, 4].map((w) => snapshot(24 * 7 * w - 30, DOMINGO.paths, ["semanal"], 9.8e9 - w * 1e8, 52_000 - w * 300)),
  ],
  r2: [snapshot(5, [DOCS[0]], [], 1.2e9, 6_250), snapshot(24 * 3, [DOCS[0]], [], 1.1e9, 6_101)],
  // Copias diarias que se interrumpieron hace 3 días: aparece como atrasada.
  r3: Array.from({ length: 20 }, (_, i) => snapshot(24 * (3 + i) + 1, ["C:\\Users\\Ana\\Documentos"], [], 1.4e9, 7_900)),
  r4: [snapshot(30, [DOCS[1]], [], 2.6e9, 1_240), snapshot(24 * 8, [DOCS[1]], [], 2.5e9, 1_201)],
  // Copias subidas desde «Disco externo» (con ?externa): una de cada dos.
  r5: [],
};

/**
 * ?cambios: «Presupuesto.xlsx» está en las 10 versiones más recientes de
 * «Laboral» y cambió 3 veces (para «Ver versiones» con
 * `?cambios&versiones=C:\Users\Ana\Documentos\Proyectos\Presupuesto.xlsx`).
 */
const CAMBIOS = new URLSearchParams(location.search).has("cambios");
if (CAMBIOS) snapshots.r1.push(...[98, 122, 146, 170, 194, 218].map((h) => snapshot(h, DOCS, ["diaria"], 4.05e9, 18_000)));
const BUDGET_T0 = Date.now();
const isBudget = (path: string) => CAMBIOS && path.endsWith("/Proyectos/Presupuesto.xlsx");
/** Contenidos de «Presupuesto.xlsx» con ?cambios, del más reciente al más antiguo: [hasta hace N horas, KB de menos]. */
const BUDGET_STATES: [number, number][] = [
  [40, 0],
  [110, 12],
  [160, 20],
  [230, 31],
];
function budgetMatch(path: string, snap: Snapshot, size: number) {
  const age = (Date.now() - Date.parse(snap.time)) / HOUR;
  const i = BUDGET_STATES.findIndex(([h]) => age < h);
  if (i < 0) return null;
  // El contenido actual se guardó por última vez una hora antes de la versión más reciente.
  const newest = [...snapshots.r1].sort((a, b) => b.time.localeCompare(a.time))[0];
  const mtime = i === 0 ? Date.parse(newest.time) - HOUR : BUDGET_T0 - (BUDGET_STATES[i - 1][0] + 3) * HOUR;
  return { path, type: "file", size: size - BUDGET_STATES[i][1] * 1024, mtime: new Date(mtime).toISOString() };
}

if (EXTERNA) snapshots.r5 = snapshots.r1.filter((_, i) => i % 2 === 0).map((x) => ({ ...x, id: `${x.id.slice(8)}${x.id.slice(0, 8)}`, short_id: x.id.slice(8, 16) }));

const cancelled = new Set<string>();

// «Todos mis equipos» (cuenta de Resguardo Web). ?equipos: ya con la sesión iniciada.
// Contraseña de prueba «resguardo» y código «123456».
const account = {
  email: new URLSearchParams(location.search).has("equipos") ? ("ana@ejemplo.com" as string | null) : null,
  pending: null as string | null,
  commands: [] as { id: string; device_id: string; repo_id: string; plan_id: string; at: number }[],
};
function accountStatus() {
  return { signed_in: !!account.email, email: account.email ?? account.pending, needs_code: !!account.pending, this_device: "ESTUDIO" };
}
const ago = (min: number) => new Date(Date.now() - min * 60_000).toISOString();
function mockDevices() {
  return [
    { id: "d1", name: "ESTUDIO", client_id: "c1", os: "Windows x86_64", app_version: "0.6.1", last_seen_at: ago(2), remote_backup_enabled: true },
    { id: "d2", name: "SERVIDOR", client_id: "c1", os: "Windows x86_64", app_version: "0.6.1", last_seen_at: ago(4), remote_backup_enabled: true },
    { id: "d3", name: "PC-01", client_id: "c2", os: "Windows x86_64", app_version: "0.6.0", last_seen_at: ago(190), remote_backup_enabled: false },
  ];
}
function mockOverview() {
  const plan = (id: string, name: string, min: number, result = "ok") => ({ id, name, last_run: { started: ago(min + 2), finished: ago(min), result } });
  const prot = (score: number) => ({ score, total: 7, items: [] });
  return {
    clients: [
      { id: "c1", name: "Estudio Pérez" },
      { id: "c2", name: "Casa" },
    ],
    devices: mockDevices(),
    repos: [
      { device_id: "d1", repo_id: "r1", name: "Disco externo", kind: "local", host: null, expected_hours: 24, last_snapshot_at: ago(80), last_total_bytes: 4.2e9, plans: [plan("laboral", "Laboral", 80)], protection: prot(4), offsite_run: { finished: ago(300), result: "ok" } },
      { device_id: "d2", repo_id: "siigo", name: "Siigo", kind: "rest", host: "nas.local:8000", expected_hours: 1, last_snapshot_at: ago(25), last_total_bytes: 18.6e9, plans: [plan("siigo", "Siigo cada hora", 25), plan("contab", "Contabilidad", 600, "warning")], protection: prot(6), offsite_run: { finished: ago(30), result: "ok" } },
      { device_id: "d2", repo_id: "nube", name: "Backblaze B2", kind: "s3", host: "s3.us-west-004.backblazeb2.com", expected_hours: 24, last_snapshot_at: ago(30), last_total_bytes: 18.1e9, plans: [], protection: prot(5) },
      { device_id: "d3", repo_id: "fotos", name: "Fotos", kind: "local", host: null, expected_hours: 24, last_snapshot_at: ago(60 * 52), last_total_bytes: 96e9, plans: [plan("fotos", "Fotos", 60 * 52)], protection: prot(2) },
    ],
    commands: [],
    this_device: "ESTUDIO",
  };
}

// Bloqueo con Windows Hello. ?bloqueo: activado y la app se abre bloqueada
// (y vuelve a bloquearse tras 5 minutos); ?sinhello: Windows Hello sin configurar.
const HELLO_EXPLAIN =
  "Windows Hello no está configurado para tu usuario. Configura un PIN, huella o reconocimiento facial en Configuración de Windows → Cuentas → Opciones de inicio de sesión.";
const lockFlags = new URLSearchParams(location.search);
/** «Ver versiones en Resguardo»: `?versiones=C:\Users\Ana\Documentos\Proyectos\Presupuesto.xlsx` la abre al cargar. */
let versionsPath: string | null = new URLSearchParams(location.search).get("versiones");
let shellMenu = false;

/** Como `versiones::locate` y `describe` del backend, con el árbol de ejemplo. */
function locateMock(raw: string) {
  const path = raw.trim().replace(/\//g, "\\").replace(/\\+$/, "");
  if (!/^[A-Za-z]:\\/.test(path) || /(^|\\)\.\.?(\\|$)/.test(path)) throw "Esa ruta no es válida.";
  const snap = path.replace(/^([A-Za-z]):/, (_, d: string) => `/${d.toUpperCase()}`).replace(/\\/g, "/");
  // Lo que no está en el árbol de ejemplo existe igual (p. ej. C:\Datos\…, que no está en ninguna copia).
  const node = treeFiles().find((f) => f.path.toLowerCase() === snap.toLowerCase()) ?? { path: snap, size: 48_000, dir: !/\.[^.\\]+$/.test(path) };
  const key = (p: string) => p.replace(/\\+$/, "").toLowerCase();
  const within = (a: string, b: string) => key(a) === key(b) || key(a).startsWith(`${key(b)}\\`);
  const found = repos.flatMap((r) =>
    r.plans.flatMap((p): LocatedPath["found"] =>
      p.paths.some((x) => within(path, x))
        ? [{ repo_id: r.id, plan_id: p.id, kind: "inside" }]
        : p.paths.some((x) => within(x, path))
          ? [{ repo_id: r.id, plan_id: p.id, kind: "contains" }]
          : [],
    ),
  );
  // Ahora mismo, como en la versión más reciente del primer destino que la incluye.
  const newest = [...(snapshots[found[0]?.repo_id] ?? [])].sort((a, b) => b.time.localeCompare(a.time))[0];
  const i = path.lastIndexOf("\\");
  return {
    path,
    name: path.slice(i + 1),
    parent: path.slice(0, i) || path,
    is_dir: node.dir,
    size: node.dir ? null : node.size,
    mtime: newest ? new Date(Date.parse(newest.time) - 3_600_000).toISOString() : null,
    found,
  };
}

/** Bandeja del sistema: ajustes y lo último que mandó la interfaz (en `window.__bandeja`, para probar). */
const trayState = { close_to_tray: true, notifications: true, autostart: true };

const lockState = {
  enabled: lockFlags.has("bloqueo"),
  idle_minutes: lockFlags.has("bloqueo") ? 5 : (null as number | null),
  locked: lockFlags.has("bloqueo"),
  availability: lockFlags.has("sinhello") ? "not_configured" : "available",
};
const cancelledChecks = new Set<string>();
const statsCache = new Map<string, { key: string; result: any }>();

// Agente simulado. Con ?noadmin en la URL, la app "no es administrador".
/** Con ?rotativa: «Disco externo» se verifica cada semana leyendo una parte de 4 (va por la 2; el último ciclo completo, hace 3 semanas). */
const ROTATIVA = new URLSearchParams(location.search).has("rotativa");
const agentTasks = {
  runs: (ROTATIVA
    ? {
        "verify:r1": {
          started: new Date(Date.now() - 4 * 24 * HOUR).toISOString(),
          finished: new Date(Date.now() - 4 * 24 * HOUR + 20 * 60_000).toISOString(),
          result: "ok",
          message: "Parte 1 de 4 verificada sin errores (en 4 verificaciones se leen todos los datos).",
        },
      }
    : EXTERNA
      ? {
          // «Disco externo» sube a «Disco externo · Nube» y verifica la copia allí.
          "offsite:r1": {
            started: new Date(Date.now() - 9 * HOUR).toISOString(),
            finished: new Date(Date.now() - 9 * HOUR + 6 * 60_000).toISOString(),
            result: "ok",
            message: "3 copias subidas. Retención aplicada en el repositorio.",
            files_new: 3,
          },
          "verify_offsite:r1": {
            started: new Date(Date.now() - 3 * 24 * HOUR).toISOString(),
            finished: new Date(Date.now() - 3 * 24 * HOUR + 4 * 60_000).toISOString(),
            result: "ok",
            message: "Sin errores en la estructura del repositorio.",
          },
        }
      : {}) as Record<string, any>,
  running: null as any,
  rotation: (ROTATIVA ? { r1: { parts: 4, next_part: 2, last_full_at: new Date(Date.now() - 25 * 24 * HOUR).toISOString() } } : {}) as Record<string, any>,
};
/** Copia a mano de los planes con horario, como `agent::set_schedule`. */
function agentPlans(repo: Repo, previous: any[] = [], enabledAt = new Date().toISOString()) {
  return repo.plans
    .filter((p) => p.schedule)
    .map((p) => {
      const prev = previous.find((q) => q.id === p.id);
      const same = prev && JSON.stringify(prev.schedule) === JSON.stringify(p.schedule) && prev.paths.join() === p.paths.join();
      return { ...structuredClone(p), enabled_at: same ? prev.enabled_at : enabledAt };
    });
}

/** Hora de la última copia programada de un plan (o hace `fallbackHours` horas). */
function lastSlot(p: Plan, fallbackHours: number) {
  const slot = p.schedule ? latestPlanSlot(p.schedule, new Date()) : null;
  return new Date(slot ? slot.getTime() + 20_000 : Date.now() - fallbackHours * HOUR).toISOString();
}

/** Con ?inusual en la URL: la última copia de «Disco externo» cambió mucho más de lo normal y la subida está frenada. */
const INUSUAL = new URLSearchParams(location.search).has("inusual");
/** Con ?subiendo: la primera subida de «Disco externo» a Backblaze B2 está en marcha (704 versiones). */
const SUBIENDO = new URLSearchParams(location.search).has("subiendo");
const uploadStart = Date.now() - 12 * 60_000;
/** Progreso simulado de la subida: la primera versión (casi todo) tarda ~40 min; las demás, segundos. */
function mockUpload() {
  const total = 704;
  const firstMs = 40 * 60_000;
  const restMs = 3_000;
  const elapsed = Date.now() - uploadStart;
  const bytesFirst = 3.9e9;
  const bytesRest = 6e6;
  const bytesTotal = bytesFirst + bytesRest * (total - 1);
  let done: number, bytesDone: number;
  if (elapsed < firstMs) {
    done = 0;
    bytesDone = (bytesFirst * elapsed) / firstMs;
  } else {
    done = Math.min(total, 1 + Math.floor((elapsed - firstMs) / restMs));
    bytesDone = bytesFirst + bytesRest * (done - 1);
  }
  const percent = Math.min(1, bytesDone / bytesTotal);
  const when = new Date(Date.now() - (total - done) * 3 * HOUR);
  const pad = (n: number) => String(n).padStart(2, "0");
  const label = `${when.getFullYear()}-${pad(when.getMonth() + 1)}-${pad(when.getDate())} ${pad(when.getHours())}:${pad(when.getMinutes())}`;
  return {
    repo_id: "r1",
    kind: "offsite",
    started: new Date(uploadStart).toISOString(),
    stage: `Subiendo la versión del ${label} (${Math.min(total, done + 1)} de ${total})…`,
    done,
    total,
    percent,
    eta_s: Math.round(((elapsed / 1000) * (1 - percent)) / Math.max(percent, 0.01)),
    bytes_done: Math.round(bytesDone),
    bytes_total: bytesTotal,
    current_snapshot_time: when.toISOString(),
    updated: new Date().toISOString(),
  };
}
const offsiteHolds: Record<string, any> = {};
function seedHold() {
  const last = [...(snapshots.r1 ?? [])].sort((a, b) => b.time.localeCompare(a.time))[0];
  if (!INUSUAL || !last) return;
  offsiteHolds.r1 = {
    since: new Date(Date.now() - 40 * 60_000).toISOString(),
    snapshot_id: last.id,
    plan_name: "Laboral",
    data_added: 38.2 * 2 ** 30,
    files: 12_400,
    typical_bytes: 20 * 2 ** 20,
    typical_files: 40,
    backup_finished: new Date(Date.now() - 45 * 60_000).toISOString(),
  };
}

/** Pausa de ejemplo según la URL (?pausa o ?pausa=manual). */
function mockPause() {
  const flag = new URLSearchParams(location.search).get("pausa");
  if (flag === null) return null;
  const d = new Date();
  const until = flag === "manual" ? null : new Date(d.getFullYear(), d.getMonth(), d.getDate() + 1, 6, 0).toISOString();
  return { since: new Date(Date.now() - 2 * HOUR).toISOString(), until };
}

const agentState = {
  repos: [
    {
      id: "r1",
      name: "Disco externo",
      location: repos[0]?.location ?? "",
      paths: [],
      excludes: [],
      schedule: { kind: "plans" },
      enabled_at: new Date(Date.now() - 72 * HOUR).toISOString(),
      plans: repos[0] ? agentPlans(repos[0], [], new Date(Date.now() - 72 * HOUR).toISOString()) : [],
      // Con ?externa: sube a «Disco externo · Nube» con la retención que se aplicaba antes (la del origen),
      // distinta de la que ahora tiene ese destino: se ve el aviso de «Aplicar cambios».
      offsite: EXTERNA
        ? {
            location: "s3:https://s3.us-west-004.backblazeb2.com/copias-ana/disco-externo",
            provider: "destino:r5",
            region: "us-west-004",
            schedule: { kind: "daily", time: "02:30" },
            retention: { ...EMPTY_POLICY, keep_daily: 7, keep_weekly: 4, keep_monthly: 12, keep_yearly: 3 },
            limit_upload_kib: null,
            target_name: "Disco externo · Nube",
            enabled_at: new Date(Date.now() - 20 * 24 * HOUR).toISOString(),
            guard: { factor: 20, min_bytes: 2 * 2 ** 30, min_files: 5000 },
            // También verifica la copia en la nube, cada domingo, solo la estructura.
            verify: { schedule: { kind: "weekly", weekday: 6, time: "05:00" }, subset_percent: 0, rotate_parts: 0, enabled_at: new Date(Date.now() - 20 * 24 * HOUR).toISOString() },
          }
        : INUSUAL || SUBIENDO
          ? {
              // Con ?inusual: sube a Backblaze después de cada copia con cambios, con freno.
              location: "s3:https://s3.us-west-004.backblazeb2.com/copias-ana/disco-externo",
              provider: "b2",
              region: "us-west-004",
              schedule: { kind: "after_backup", min_minutes: 30 },
              retention: null,
              limit_upload_kib: null,
              target_name: null,
              enabled_at: new Date(Date.now() - 20 * 24 * HOUR).toISOString(),
              guard: { factor: 20, min_bytes: 2 * 2 ** 30, min_files: 5000 },
            }
          : null,
      // Con ?pausa en la URL, sus copias automáticas están en pausa hasta mañana a las 6:00
      // (con ?pausa=manual, hasta reanudarlas).
      pause: mockPause(),
      verify: ROTATIVA
        ? { schedule: { kind: "weekly", weekday: 6, time: "03:00" }, subset_percent: 0, rotate_parts: 4, enabled_at: new Date(Date.now() - 60 * 24 * HOUR).toISOString() }
        : null,
    },
  ] as any[],
  runs: {
    // Últimas copias a su hora (o hace un rato si hoy aún no tocaba).
    "r1#laboral": {
      started: lastSlot(LABORAL, 0.6),
      finished: new Date(new Date(lastSlot(LABORAL, 0.6)).getTime() + 95_000).toISOString(),
      // Sin cambios desde la última versión («Solo guardar si hay cambios»).
      result: "ok",
      message: "Sin cambios desde la última versión: no se guardó una nueva.",
      data_added: 0,
      files_new: 0,
      files_changed: 0,
      unchanged: true,
    },
    "r1#domingo": {
      started: lastSlot(DOMINGO, 50),
      finished: new Date(new Date(lastSlot(DOMINGO, 50)).getTime() + 400_000).toISOString(),
      result: "warning",
      message: "Copia terminada, pero algunos archivos no se pudieron leer.",
      data_added: 1.2e9,
    },
  } as Record<string, any>,
};
seedHold();

/** Con ?protegido: «Disco externo» lo tiene todo (copia externa, verificación, prueba de restauración, kit y retención). */
const PROTEGIDO = new URLSearchParams(location.search).has("protegido");
if (PROTEGIDO) {
  const r1 = agentState.repos[0];
  const ago = (h: number) => new Date(Date.now() - h * HOUR).toISOString();
  r1.offsite ??= {
    location: "s3:https://s3.us-west-004.backblazeb2.com/copias-ana/disco-externo",
    provider: "b2",
    region: "us-west-004",
    schedule: { kind: "after_backup", min_minutes: 30 },
    retention: null,
    limit_upload_kib: null,
    target_name: null,
    enabled_at: ago(24 * 20),
    guard: { factor: 20, min_bytes: 2 * 2 ** 30, min_files: 5000 },
  };
  r1.verify = { schedule: { kind: "weekly", weekday: 6, time: "03:00" }, subset_percent: 0, rotate_parts: 4, enabled_at: ago(24 * 60) };
  r1.restore_test = { schedule: { kind: "weekly", weekday: 5, time: "04:00" }, files: 20, max_mb: 200, enabled_at: ago(24 * 60) };
  const ok = (h: number, message: string) => ({ started: ago(h), finished: ago(h - 0.1), result: "ok", message });
  agentTasks.runs["offsite:r1"] = ok(0.4, "1 copia subida.");
  agentTasks.runs["verify:r1"] = ok(30, "Parte 2 de 4 verificada sin errores (en 4 verificaciones se leen todos los datos).");
  agentTasks.runs["restore_test:r1"] = ok(50, "20 archivos (85 MB) de la versión del 29/09 18:00 restaurados y comprobados.");
  repos[0].kit = { saved_at: ago(24 * 10), location: repos[0].location, config_id: null };
  repos[0].retention = { ...EMPTY_POLICY, keep_within_hourly: "15d", keep_within_daily: "1y", keep_monthly: -1 };
}

/**
 * Con ?protegido=nube: «Backblaze B2» lo tiene todo salvo el bloqueo de objetos (6 de 7). Marcarlo
 * («Tiene bloqueo de objetos», contraseña «resguardo») lo deja en 7 de 7: sirve para ver la celebración
 * de «Mejorar la protección».
 */
if (new URLSearchParams(location.search).get("protegido") === "nube" && repos[3]) {
  const r4 = repos[3];
  const ago = (h: number) => new Date(Date.now() - h * HOUR).toISOString();
  r4.plans[0].schedule = { days: [0, 1, 2, 3, 4, 5, 6], mode: "at", times: ["13:00"], every_hours: 1, from: "", to: "" };
  agentState.repos.push({
    id: r4.id,
    name: r4.name,
    location: r4.location,
    paths: [],
    excludes: [],
    schedule: { kind: "plans" },
    enabled_at: ago(24 * 30),
    plans: agentPlans(r4, [], ago(24 * 30)),
    offsite: {
      location: "s3:https://s3.eu-central-003.backblazeb2.com/copias-ana/fotos-2",
      provider: "b2",
      region: "eu-central-003",
      schedule: { kind: "daily", time: "02:00" },
      retention: null,
      limit_upload_kib: null,
      target_name: null,
      enabled_at: ago(24 * 20),
      guard: { factor: 20, min_bytes: 2 * 2 ** 30, min_files: 5000 },
    },
    verify: { schedule: { kind: "weekly", weekday: 6, time: "03:00" }, subset_percent: 0, rotate_parts: 0, enabled_at: ago(24 * 60) },
    restore_test: { schedule: { kind: "weekly", weekday: 5, time: "04:00" }, files: 20, max_mb: 200, enabled_at: ago(24 * 60) },
    pause: null,
  });
  const ok = (h: number, message: string) => ({ started: ago(h), finished: ago(h - 0.1), result: "ok", message });
  agentState.runs[`${r4.id}#${r4.plans[0].id}`] = { ...ok(10, "Copia completada."), data_added: 2e8 };
  agentTasks.runs[`offsite:${r4.id}`] = ok(1, "1 copia subida.");
  agentTasks.runs[`verify:${r4.id}`] = ok(30, "Estructura verificada sin errores.");
  agentTasks.runs[`restore_test:${r4.id}`] = ok(50, "20 archivos restaurados y comprobados.");
  r4.kit = { saved_at: ago(24 * 10), location: r4.location, config_id: null };
  r4.retention = { ...EMPTY_POLICY, keep_within_daily: "60d", keep_monthly: -1 };
  r4.object_lock = false;
}

/** Como `protection::evaluate` (aproximado): la salud de la protección de un destino. */
function mockProtection(id: string) {
  const repo = repos.find((r) => r.id === id)!;
  const entry = agentState.repos.find((r) => r.id === id);
  const kind = repo.location.startsWith("rest:") ? "rest" : /^(s3|b2|azure|gs):/.test(repo.location) ? "cloud" : "local";
  const sources = agentState.repos.filter((r) => r.offsite?.provider === `destino:${id}`);
  const target = sources.length > 0 && !repo.plans.length;
  const run = (k: string) => agentTasks.runs[`${k}:${id}`];
  const items: { id: string; state: string; label: string; detail: string }[] = [];
  const add = (iid: string, state: string, label: string, detail: string) => items.push({ id: iid, state, label, detail });
  const planRuns = (entry?.plans ?? []).map((p: any) => agentState.runs[`${id}#${p.id}`]).filter(Boolean);
  if (target) add("copias", "ok", "Copias", `Recibe la copia externa de «${sources[0].name}».`);
  else if (!entry) add("copias", "bad", "Copias automáticas", "Sin copias automáticas: solo se copia cuando alguien lo hace a mano.");
  else if (planRuns.some((r: any) => r.result === "error")) add("copias", "bad", "Copias automáticas", "La última copia automática de alguna copia falló.");
  else add("copias", "ok", "Copias automáticas", "Activas y al día.");
  if (kind === "rest") {
    const a = repo.append_only?.append_only;
    if (a === true) add("borrado", "ok", "Protegida contra borrado", "El servidor es de solo añadir: desde este equipo no se puede borrar nada.");
    else if (a === false) add("borrado", "warn", "Protegida contra borrado", "El servidor permite borrar: arranca rest-server con --append-only para que nadie pueda borrar las versiones desde aquí.");
    else add("borrado", "unknown", "Protegida contra borrado", "Sin comprobar: no se pudo consultar al servidor.");
  } else if (kind === "cloud")
    add("borrado", repo.object_lock ? "ok" : "warn", "Protegida contra borrado", repo.object_lock ? "El bucket tiene bloqueo de objetos (Object Lock)." : "Activa el bloqueo de objetos (Object Lock) en el bucket y márcalo aquí para que nadie pueda borrar las versiones.");
  else add("borrado", "warn", "Protegida contra borrado", "Un ransomware podría borrarla; usa un servidor de solo añadir o una copia externa con bloqueo.");
  if (target) add("externa", "ok", "Copia externa", `Este repositorio es la copia externa de «${sources[0].name}».`);
  else if (!entry?.offsite) add("externa", "bad", "Copia externa", "Todas las versiones están en un solo sitio: configura una copia externa (la nube u otro disco).");
  else if (offsiteHolds[id]) add("externa", "bad", "Copia externa", "Subida frenada por un cambio inusual: revísalo.");
  else if (!run("offsite")) add("externa", "warn", "Copia externa", "Configurada, todavía sin ninguna subida.");
  else add("externa", run("offsite").result === "error" ? "bad" : "ok", "Copia externa", run("offsite").result === "error" ? `La última subida falló.` : "Al día.");
  const vcfg = target ? sources[0].offsite?.verify : entry?.verify;
  const vrun = target ? agentTasks.runs[`verify_offsite:${sources[0].id}`] : run("verify");
  if (!vcfg) add("verificacion", "warn", "Verificación", target ? "Sin verificación: prográmala en el origen, en «Copia externa»." : "Sin verificación programada.");
  else if (!vrun) add("verificacion", "warn", "Verificación", "Programada, todavía sin ninguna hecha.");
  else add("verificacion", vrun.result === "error" ? "bad" : "ok", "Verificación", vrun.result === "error" ? "La última verificación encontró errores o no se pudo hacer." : "Sin errores.");
  if (!target) {
    const t = run("restore_test");
    if (!entry?.restore_test) add("restauracion", "warn", "Prueba de restauración", "Sin prueba: nadie comprueba que las copias se puedan recuperar de verdad.");
    else if (!t) add("restauracion", "warn", "Prueba de restauración", "Programada, todavía sin ninguna hecha.");
    else add("restauracion", t.result === "error" ? "bad" : "ok", "Prueba de restauración", t.result === "error" ? "La última falló." : "Correcta.");
  }
  const kitOk = !!repo.kit && repo.kit.location === repo.location;
  add("kit", kitOk ? "ok" : repo.kit ? "warn" : "bad", "Kit de recuperación", kitOk ? "Guardado." : repo.kit ? "La ubicación cambió desde el último kit: guarda uno nuevo." : "Sin kit: si pierdes este equipo no podrás abrir las copias.");
  if (!repo.retention) add("retencion", "warn", "Retención", "Sin política: el repositorio crece sin fin.");
  else add("retencion", "ok", "Retención", kind === "rest" && repo.append_only?.append_only ? "Configurada; se aplica en el servidor." : "Configurada.");
  return { score: items.filter((i) => i.state === "ok").length, total: items.length, items };
}

// Historial de actividad simulado (como `history::combined`): lo más reciente primero.
let activityLog: ActivityEntry[] | null = null;

/** Como `history::note`: un cambio de configuración («config») o un kit guardado («kit»). */
function logConfig(repoId: string, message: string, plan?: { id: string; name: string }, kind: "config" | "kit" = "config") {
  const now = new Date().toISOString();
  const name = repos.find((r) => r.id === repoId)?.name ?? "";
  logActivity({ kind, origin: "manual", repo_id: repoId, repo_name: name, plan_id: plan?.id ?? null, plan_name: plan?.name ?? null, started: now, finished: now, result: "info", message, user: "ana" });
}

/** Como `history::plan_changes`: qué cambió en las copias de un destino. */
function planChanges(old: Plan[], next: Plan[]) {
  const out: [Plan, string][] = [];
  for (const p of next) {
    const o = old.find((x) => x.id === p.id);
    if (!o) out.push([p, `Copia «${p.name}» creada.`]);
    else if (JSON.stringify(o) !== JSON.stringify(p)) {
      const what: string[] = [];
      if (o.name !== p.name) what.push(`nombre («${o.name}» → «${p.name}»)`);
      if (o.paths.join() !== p.paths.join()) what.push("carpetas");
      if (o.excludes.join() !== p.excludes.join()) what.push("exclusiones");
      if (o.tags.join() !== p.tags.join()) what.push("etiquetas");
      if (JSON.stringify(o.schedule) !== JSON.stringify(p.schedule))
        what.push(!o.schedule ? "horario (ahora se hace sola)" : !p.schedule ? "horario (ahora solo a mano)" : "horario");
      if (!!o.skip_unchanged !== !!p.skip_unchanged) what.push("«solo guardar si hay cambios»");
      if (what.length) out.push([p, `Copia «${p.name}» cambiada: ${what.join(", ")}.`]);
    }
  }
  for (const o of old) if (!next.some((p) => p.id === o.id)) out.push([o, `Copia «${o.name}» eliminada.`]);
  return out;
}

function logActivity(e: ActivityEntry) {
  (activityLog ??= seedActivity()).unshift(e);
}

/** Actividad realista de los últimos 10 días en los destinos de ejemplo. */
function seedActivity(): ActivityEntry[] {
  const out: ActivityEntry[] = [];
  const now = Date.now();
  const at = (daysAgo: number, h: number, m = 0) => {
    const d = new Date();
    d.setDate(d.getDate() - daysAgo);
    d.setHours(h, m, 20, 0);
    return d;
  };
  const add = (start: Date, secs: number, e: Omit<ActivityEntry, "started" | "finished">) => {
    const end = start.getTime() + secs * 1000;
    if (end > now) return;
    out.push({ ...e, started: start.toISOString(), finished: new Date(end).toISOString() });
  };
  const r1 = { repo_id: "r1", repo_name: "Disco externo" };
  const copy = (i: number, big = 1) => ({
    snapshot_id: hex(),
    data_added: Math.round((18 + ((i * 37) % 160)) * 1e6 * big),
    files_new: (i * 7) % 40,
    files_changed: ((i * 5) % 25) + 1,
  });

  // «Laboral»: cada hora de 7 a 19, de lunes a sábado, desde que se programó (hace 3 días).
  const slots: Date[] = [];
  for (let t = Math.ceil((now - 72 * HOUR) / HOUR) * HOUR; t <= now; t += HOUR) {
    const d = new Date(t);
    if ((d.getDay() + 6) % 7 <= 5 && d.getHours() >= 7 && d.getHours() <= 19) slots.push(new Date(t + 20_000));
  }
  // Dos fallos, cada uno seguido de un reintento correcto.
  const fails = new Map<number, string>([
    [3, "No se pudo abrir el repositorio E:\\Copias\\restic: el sistema no puede encontrar la ruta especificada. ¿Está conectado el disco externo?"],
    [slots.length - 6, "El repositorio está bloqueado por otra operación (restic lock). Se reintentará en unos minutos."],
  ]);
  slots.forEach((start, i) => {
    const base = { kind: "backup" as const, ...r1, plan_id: "laboral", plan_name: "Laboral" };
    const fail = fails.get(i);
    if (fail) {
      add(start, 4 + (i % 5), { ...base, origin: "agent", result: "error", message: fail });
      add(new Date(start.getTime() + 12 * 60_000), 70 + (i % 40), { ...base, origin: "retry", result: "ok", message: "Copia completada.", ...copy(i, 1.6) });
    } else {
      // Con «Solo guardar si hay cambios»: la primera del día guarda versión; el resto, casi siempre sin cambios.
      const first = i === 0 || slots[i - 1].getDate() !== start.getDate();
      if (!first && i % 3 !== 0)
        add(start, 20 + ((i * 7) % 15), {
          ...base,
          origin: "agent",
          result: "ok",
          message: "Sin cambios desde la última versión: no se guardó una nueva.",
          data_added: 0,
          files_new: 0,
          files_changed: 0,
          unchanged: true,
        });
      else add(start, 55 + ((i * 13) % 85), { ...base, origin: "agent", result: "ok", message: "Copia completada.", ...copy(i) });
    }
  });

  for (let day = 0; day <= 9; day++) {
    const weekday = (at(day, 0).getDay() + 6) % 7;
    // Copia externa de «Disco externo» a Backblaze B2, cada noche.
    const warn = day === 4;
    add(at(day, 2, 30), 420 + day * 37, {
      kind: "offsite",
      origin: "agent",
      ...r1,
      result: warn ? "warning" : "ok",
      message: warn
        ? "2 de 3 copias subidas: se acabó la ventana de subida de la noche. La que falta se subirá en la próxima."
        : "3 copias subidas.",
      files_new: warn ? 2 : 3,
    });
    if (weekday === 6) {
      // Domingo: verificación semanal de madrugada y la copia «Domingo» a las 23:00.
      add(at(day, 4), 380 + day * 3, { kind: "verify", origin: "agent", ...r1, result: "ok", message: "Sin errores (se leyó el 5 % de los datos)." });
      const recent = day < 7;
      add(at(day, 23), recent ? 402 : 371, {
        kind: "backup",
        origin: "agent",
        ...r1,
        plan_id: "domingo",
        plan_name: "Domingo",
        result: recent ? "warning" : "ok",
        message: recent
          ? "Copia terminada con 2 archivos que no se pudieron leer: C:\\Users\\Ana\\Documentos\\Outlook\\ana.pst (el proceso no tiene acceso al archivo porque está siendo utilizado por otro proceso) y C:\\Users\\Ana\\NTUSER.DAT (acceso denegado)."
          : "Copia completada.",
        ...copy(day, recent ? 9 : 5),
      });
    }
  }

  // Cambios de configuración (desde esta versión se anotan).
  const cfg = (start: Date, message: string, plan?: { plan_id: string; plan_name: string }) =>
    add(start, 0, { kind: "config", origin: "manual", ...r1, ...(plan ?? {}), result: "info", message, user: "ana" });
  cfg(at(3, 9, 12), "Copias automáticas activadas según los horarios de sus copias.");
  cfg(at(3, 9, 5), "Copia «Laboral» cambiada: exclusiones, horario.", { plan_id: "laboral", plan_name: "Laboral" });
  cfg(at(9, 16, 40), "Copia «Domingo» creada.", { plan_id: "domingo", plan_name: "Domingo" });
  cfg(at(9, 16, 31), "Verificación programada.");

  // Copias a mano.
  const manual = { kind: "backup" as const, origin: "manual" as const, user: "ana" };
  const r2 = { repo_id: "r2", repo_name: "Servidor de casa", plan_id: "principal", plan_name: "Principal" };
  const r4 = { repo_id: "r4", repo_name: "Backblaze B2", plan_id: "fotos", plan_name: "Fotos" };
  const laboral = { ...r1, plan_id: "laboral", plan_name: "Laboral" };
  add(new Date(now - 5 * HOUR), 48, { ...manual, ...r2, result: "ok", message: "Copia completada.", ...copy(2) });
  add(new Date(now - 72 * HOUR), 61, { ...manual, ...r2, result: "ok", message: "Copia completada.", ...copy(5) });
  add(at(6, 20, 12), 31, {
    ...manual,
    ...r2,
    result: "error",
    message: "No se pudo conectar con el servidor REST (https://nas.local:8000/portatil/): tiempo de espera agotado.",
  });
  add(new Date(now - 30 * HOUR), 214, { ...manual, ...r4, result: "ok", message: "Copia completada.", ...copy(9, 4) });
  add(new Date(now - 8 * 24 * HOUR), 262, { ...manual, ...r4, result: "ok", message: "Copia completada.", ...copy(11, 5) });
  add(at(4, 18, 10), 96, { ...manual, ...laboral, result: "ok", message: "Copia completada.", ...copy(4) });
  add(at(5, 13, 40), 88, { ...manual, ...laboral, result: "warning", message: "Copia terminada con 1 archivo que no se pudo leer.", ...copy(6) });
  add(at(8, 17, 55), 104, { ...manual, ...laboral, result: "ok", message: "Copia completada.", ...copy(8) });

  // Con ?inusual: el aviso del freno.
  const hold = offsiteHolds.r1;
  if (hold)
    add(new Date(hold.since), 0, {
      kind: "guard",
      origin: "agent",
      ...r1,
      plan_name: hold.plan_name,
      result: "warning",
      message: `La copia «${hold.plan_name}» añadió 38,2 GB y 12.400 archivos (lo normal: ~20 MB y ~40). La subida a la nube está frenada por precaución.`,
      snapshot_id: hold.snapshot_id,
      data_added: hold.data_added,
      files_new: hold.files,
    });

  // Con ?pausa: la pausa de «Disco externo», hace 2 horas.
  const pause = mockPause();
  if (pause)
    add(new Date(pause.since), 0, {
      kind: "pause",
      origin: "manual",
      ...r1,
      result: "info",
      message: pause.until ? "Hasta mañana a las 06:00." : "Hasta que las reanudes.",
    });

  return out.sort((a, b) => b.finished.localeCompare(a.finished));
}

let webLink: any = null;
/** ?vinculado o ?equipos: el equipo ya está vinculado con la web (hasta que se desvincule). */
let presetLink = /[?&](vinculado|equipos)(&|=|$)/.test(location.search);
function webInfo() {
  if (presetLink && !webLink)
    webLink = { url: "https://resguardo-web.example", key: "pub", device_id: "d1", device_name: "ESTUDIO", paired_at: new Date().toISOString(), revoked: false };
  return {
    link: webLink,
    state: { last_report: webLink ? new Date(Date.now() - 3 * 60_000).toISOString() : null, last_error: null },
    elevated: !new URLSearchParams(location.search).has("noadmin"),
    default_url: "https://tu-proyecto.supabase.co",
    default_key: "sb_publishable_ejemplo",
    default_name: "PORTATIL-ANA",
  };
}

// Compartir destinos entre equipos (fase 3): lo compartido aquí, lo que comparten «otros
// equipos» de la cuenta, las peticiones y lo recibido. ?compartido: ya hay algo recibido.
const shareState = {
  shared: {} as Record<string, { since: string; delivered: [string, string][] }>,
  others: [
    { id: "sh-altamar", device_id: "d2", device_name: "SERVIDOR-01", kind: "s3", host: "s3.us-west-004.backblazeb2.com", base: "altamar-copias", name: "Backblaze B2 · altamar-copias", created_at: ago(60 * 24 * 3) },
    { id: "sh-nas", device_id: "d3", device_name: "OFICINA", kind: "rest", host: "nas.oficina.lan:8000", base: "/", name: "Servidor de la oficina", created_at: ago(60 * 24 * 12) },
  ],
  requests: [] as { id: string; share_id: string; status: string; reason: string | null; not_before: string; created_at: string }[],
  received: [] as { id: string; meta: { kind: string; host: string | null; base: string; name: string }; base: string; from_device: string; received_at: string }[],
};
if (new URLSearchParams(location.search).has("compartido")) {
  shareState.received.push({ id: "sh-altamar", meta: { kind: "s3", host: "s3.us-west-004.backblazeb2.com", base: "altamar-copias", name: "Backblaze B2 · altamar-copias" }, base: "s3:https://s3.us-west-004.backblazeb2.com/altamar-copias", from_device: "SERVIDOR-01", received_at: ago(12) });
  shareState.requests.push({ id: "rq1", share_id: "sh-altamar", status: "received", reason: null, not_before: ago(20), created_at: ago(22) });
}

// Servidor de copias (fase 4). ?servidor: ya activo con dos equipos.
const serverState = {
  enabled: new URLSearchParams(location.search).has("servidor") || new URLSearchParams(location.search).has("gestionados"),
  path: "D:\\Copias de otros equipos",
  port: 8000,
  local_subnet_only: true,
  tls_sha256: "3F:A1:9C:07:5B:E2:44:D0:8A:61:2C:FE:90:13:B7:6D:28:4E:F5:A9:0B:C3:77:1E:D6:52:8F:04:BA:39:E1:6C" as string | null,
  users: new URLSearchParams(location.search).has("servidor")
    ? [
        { name: "altamar-pc1", created_at: ago(60 * 24 * 20), repos: ["portatil", "documentos"], shared: true },
        { name: "recepcion", created_at: ago(60 * 24 * 4), repos: ["recepcion"], shared: true },
      ]
    : ([] as { name: string; created_at: string; repos: string[]; shared: boolean }[]),
};
function mockServer() {
  const elevated = !new URLSearchParams(location.search).has("noadmin");
  return { supported: true, elevated, binary_problem: null, enabled: serverState.enabled, path: serverState.path, port: serverState.port, local_subnet_only: serverState.local_subnet_only, running: serverState.enabled, lan_addresses: ["192.168.1.20"], tls_sha256: serverState.enabled ? serverState.tls_sha256 : null, users: serverState.users, linked: !!webInfo().link };
}

// Equipos gestionados (fase 5). ?gestionados: la consola ya gestiona dos equipos.
const mockSchedule = (times: string[], days = [0, 1, 2, 3, 4]) => ({ days, mode: "at" as const, times, every_hours: 1, from: "08:00", to: "18:00" });
const managedState = {
  endpoints: new URLSearchParams(location.search).has("gestionados")
    ? [
        {
          device_id: "m1",
          name: "RECEPCION",
          server_user: "recepcion",
          location: "rest:https://192.168.1.20:8000/recepcion/equipo/",
          plans: [
            { id: "documentos", name: "Documentos", paths: ["C:\\Users\\Recepcion\\Documents", "C:\\Users\\Recepcion\\Desktop"], excludes: ["*.tmp"], tags: [], schedule: mockSchedule(["13:00", "19:00"]), skip_unchanged: true },
          ],
          paired_at: ago(60 * 24 * 9),
          stopped: false,
          tray: true,
          tray_toasts: false,
          retention: "frecuente" as const,
          last_prune: ago(60 * 24 * 2),
          last_prune_error: null,
          last_snapshot: ago(95),
          snapshots: 41,
          bytes: 38.6e9,
        },
        {
          device_id: "m2",
          name: "PORTATIL-LAURA",
          server_user: "portatil-laura",
          location: "rest:https://192.168.1.20:8000/portatil-laura/equipo/",
          plans: [],
          paired_at: ago(60 * 3),
          stopped: false,
          tray: true,
          tray_toasts: true,
          retention: "frecuente" as const,
          last_prune: null,
          last_prune_error: null,
          last_snapshot: null,
          snapshots: 0,
          bytes: 0,
        },
      ]
    : ([] as import("$lib/api").ManagedEndpoint[]),
  pairing: null as null | { id: string; polls: number; status: string },
};

// ?bloqueado: la última copia de «Laboral» falló por un bloqueo antiguo (se ofrece «Desbloquear»).
if (new URLSearchParams(location.search).has("bloqueado")) {
  (agentState.runs as any)["r1#laboral"] = {
    started: ago(25),
    finished: ago(24),
    result: "error",
    message:
      "El repositorio tiene un bloqueo antiguo (de hace 26 h 3 min), de una operación que se cortó. Si no hay nada en marcha en otro equipo, quítalo con «Desbloquear».",
  };
}

// Con ?discreto, el modo discreto está activado (todo el día, para verlo siempre en marcha).
if (new URLSearchParams(location.search).has("discreto")) {
  (agentState as any).discreet = { days: [0, 1, 2, 3, 4, 5, 6], from: "00:00", to: "23:59", upload_kib: 2048 };
}

function agentInfo() {
  return {
    supported: true,
    elevated: !new URLSearchParams(location.search).has("noadmin"),
    task_installed: agentState.repos.length > 0,
    remote_backup: !!(agentState as any).remote_backup,
    discreet: structuredClone((agentState as any).discreet ?? null),
    repos: structuredClone(agentState.repos),
    state: {
      runs: structuredClone(agentState.runs),
      last_tick: new Date(Date.now() - 2 * 60_000).toISOString(),
      // Con ?copiando en la URL, el agente está copiando el plan «Laboral».
      running: new URLSearchParams(location.search).has("copiando")
        ? {
            repo_id: "r1",
            plan_id: "laboral",
            started: new Date(Date.now() - 3 * 60_000).toISOString(),
            percent: 0.42,
            files_done: 7_800,
            total_files: 18_530,
            bytes_done: 1.8e9,
            total_bytes: 4.27e9,
            seconds_remaining: 250,
            updated: new Date().toISOString(),
          }
        : null,
    },
    tasks: SUBIENDO ? { ...structuredClone(agentTasks), running: mockUpload() } : structuredClone(agentTasks),
    offsite_holds: structuredClone(offsiteHolds),
  };
}

/** Aproximación del algoritmo de `restic forget` para el modo vista previa. */
function mockForget(list: Snapshot[], p: Record<string, any>) {
  const DAY = 86_400_000;
  const toMs = (d: string | null | undefined) => {
    const m = /^(\d+)([hdmy])$/.exec(d ?? "");
    return m ? Number(m[1]) * { h: DAY / 24, d: DAY, m: 30 * DAY, y: 365 * DAY }[m[2] as "h" | "d" | "m" | "y"] : 0;
  };
  // Filtro: alguna de las etiquetas, el equipo y alguna de las carpetas. Lo de fuera no se toca.
  const tags: string[] = p.filter_tags ?? [];
  const paths: string[] = p.filter_paths ?? [];
  const inFilter = (s: Snapshot) =>
    (!tags.length || tags.some((t) => s.tags?.includes(t))) &&
    (!p.filter_host || s.hostname === p.filter_host) &&
    (!paths.length || paths.some((x) => s.paths.includes(x)));
  const reasons = new Map<string, string[]>();
  const add = (id: string, r: string) => reasons.set(id, [...(reasons.get(id) ?? []), r]);
  // Grupos: por equipo y carpetas (restic), uno solo ([]), o las claves elegidas.
  const keys: string[] = p.group_by ?? ["host", "paths"];
  const groupKey = (s: Snapshot) =>
    keys.map((k) => (k === "host" ? s.hostname : k === "paths" ? [...s.paths].sort().join() : [...(s.tags ?? [])].sort().join())).join("|");
  const groups = new Map<string, Snapshot[]>();
  for (const s of list.filter(inFilter)) groups.set(groupKey(s), [...(groups.get(groupKey(s)) ?? []), s]);
  const count = (n: number) => (n === -1 ? Infinity : n || 0);
  for (const group of groups.values()) {
    const sorted = [...group].sort((a, b) => b.time.localeCompare(a.time));
    const latest = new Date(sorted[0].time).getTime();
    sorted.slice(0, count(p.keep_last)).forEach((s) => add(s.id, "last snapshot"));
    const buckets: [string, (d: Date) => string][] = [
      ["hourly", (d) => d.toISOString().slice(0, 13)],
      ["daily", (d) => d.toISOString().slice(0, 10)],
      ["weekly", (d) => `${d.getFullYear()}-${Math.floor((d.getTime() / DAY + 3) / 7)}`],
      ["monthly", (d) => d.toISOString().slice(0, 7)],
      ["yearly", (d) => String(d.getFullYear())],
    ];
    for (const [name, key] of buckets) {
      // Por cantidad: las N últimas de cada periodo. Por plazo: una por periodo dentro del plazo.
      const n = count(p[`keep_${name}`]);
      const within = toMs(p[`keep_within_${name}`]);
      for (const [limit, label, inRange] of [
        [n, `${name} snapshot`, () => true],
        [within ? Infinity : 0, `${name} within ${p[`keep_within_${name}`]}`, (s: Snapshot) => latest - new Date(s.time).getTime() <= within],
      ] as [number, string, (s: Snapshot) => boolean][]) {
        const seen = new Set<string>();
        for (const s of sorted) {
          if (seen.size >= limit) break;
          if (!inRange(s)) continue;
          const k = key(new Date(s.time));
          if (!seen.has(k)) {
            seen.add(k);
            add(s.id, label);
          }
        }
      }
    }
    const within = toMs(p.keep_within);
    if (within) sorted.filter((s) => latest - new Date(s.time).getTime() <= within).forEach((s) => add(s.id, `within ${p.keep_within}`));
    for (const t of p.keep_tags ?? []) sorted.filter((s) => s.tags?.includes(t)).forEach((s) => add(s.id, `has tags [${t}]`));
  }
  for (const s of list) if (!inFilter(s)) add(s.id, "outside filter");
  return {
    items: [...list].sort((a, b) => b.time.localeCompare(a.time)).map((s) => ({ snapshot: s, keep: reasons.has(s.id), reasons: reasons.get(s.id) ?? [] })),
    groups: groups.size,
  };
}

// Árbol de archivos de ejemplo para explorar snapshots.
type Node = { size?: number; children?: Record<string, Node> };
const file = (kb: number): Node => ({ size: Math.round(kb * 1024) });
const TREE: Record<string, Node> = {
  C: {
    children: {
      Users: {
        children: {
          Ana: {
            children: {
              Documentos: {
                children: {
                  Proyectos: {
                    children: {
                      "Informe anual 2026.docx": file(842),
                      "Presupuesto.xlsx": file(126),
                      "Presentación clientes.pptx": file(9_420),
                      Borradores: { children: { "idea-1.md": file(3), "idea-2.md": file(5) } },
                    },
                  },
                  Facturas: {
                    children: Object.fromEntries(
                      Array.from({ length: 24 }, (_, i) => [`factura-2026-${String(i + 1).padStart(3, "0")}.pdf`, file(80 + i * 7)]),
                    ),
                  },
                  "Contrato alquiler.pdf": file(1_210),
                  "Notas.txt": file(2),
                  "CV Ana.pdf": file(310),
                },
              },
              Imágenes: {
                children: {
                  "Vacaciones 2026": {
                    children: Object.fromEntries(Array.from({ length: 36 }, (_, i) => [`IMG_${4100 + i}.jpg`, file(3_800 + i * 91)])),
                  },
                  "Perfil.png": file(540),
                },
              },
            },
          },
        },
      },
    },
  },
};

function listTree(dir: string) {
  let node: Node | undefined = { children: TREE };
  for (const part of dir.split("/").filter(Boolean)) node = node?.children?.[part];
  const children = node?.children ?? {};
  const mtime = new Date(Date.now() - 5 * 86_400_000).toISOString();
  return Object.entries(children)
    .map(([name, n]) => ({
      name,
      kind: n.children ? "dir" : "file",
      path: dir === "/" ? `/${name}` : `${dir}/${name}`,
      size: n.size ?? null,
      mtime,
    }))
    .sort((a, b) => Number(b.kind === "dir") - Number(a.kind === "dir") || a.name.localeCompare(b.name));
}

// «Buscar un archivo»: recorre las versiones de la más reciente a la más
// antigua, como `restic find`. Cada archivo del árbol de ejemplo «existe»
// en un tramo de versiones y algunos cambian de tamaño con el tiempo.
function treeFiles(dir = "", nodes: Record<string, Node> = TREE): { path: string; size: number; dir: boolean }[] {
  return Object.entries(nodes).flatMap(([name, n]) => {
    const path = `${dir}/${name}`;
    return n.children ? [{ path, size: 0, dir: true }, ...treeFiles(path, n.children)] : [{ path, size: n.size ?? 0, dir: false }];
  });
}
const hashOf = (s: string) => [...s].reduce((h, c) => (h * 31 + c.charCodeAt(0)) >>> 0, 7);

function globRegex(pattern: string) {
  // Una ruta de Windows se pasa al formato de las versiones (como el backend).
  const p = pattern
    .trim()
    .replace(/\\/g, "/")
    .replace(/^([A-Za-z]):/, (_, d: string) => `/${d.toUpperCase()}`)
    .replace(/\/+$/, "");
  const glob = /[*?[]/.test(p) || p.includes("/") ? p : `*${p}*`;
  const re = glob.replace(/[.+^${}()|\\]/g, "\\$&").replace(/\*/g, ".*").replace(/\?/g, ".");
  return { re: new RegExp(`^${re}$`, "i"), full: p.includes("/") };
}

async function simulateSearch(id: string, req: { pattern: string; snapshots?: string[] }) {
  cancelled.delete(`${id}:search`);
  if (!req.pattern.trim()) throw "Escribe el nombre (o parte del nombre) del archivo que buscas.";
  const { re, full } = globRegex(req.pattern);
  const all = [...(snapshots[id] ?? [])].sort((a, b) => b.time.localeCompare(a.time));
  const list = req.snapshots?.length ? all.filter((s) => req.snapshots!.includes(s.short_id)) : all;
  const files = treeFiles().filter((f) => re.test(full ? f.path : f.path.slice(f.path.lastIndexOf("/") + 1)));
  let total = 0;
  let hitSnaps = 0;
  await delay(400);
  for (const [i, s] of list.entries()) {
    if (cancelled.has(`${id}:search`)) throw "Búsqueda cancelada.";
    const pos = all.indexOf(s);
    const matches = files
      .filter((f) => {
        if (isBudget(f.path)) return true;
        const h = hashOf(f.path);
        // Algunos archivos se borraron hace unas versiones; otros aparecieron tarde.
        const goneBefore = h % 5 === 0 ? 3 + (h % 7) : 0;
        const bornAt = h % 3 === 0 ? all.length - 1 - (h % 11) * 2 : all.length;
        return pos >= goneBefore && pos < bornAt;
      })
      .map((f) => {
        if (isBudget(f.path)) return budgetMatch(f.path, s, f.size);
        const step = hashOf(f.path) % 4 === 1 ? Math.floor(pos / 6) : 0;
        const mtime = new Date(Date.parse(all[Math.min(all.length - 1, step * 6)]?.time ?? s.time) - 3_600_000).toISOString();
        return { path: f.path, type: f.dir ? "dir" : "file", size: f.dir ? null : Math.round(f.size * (1 + step * 0.03)), mtime };
      })
      .filter((m) => m !== null);
    if (matches.length) {
      const room = 5000 - total;
      hitSnaps++;
      total += Math.min(room, matches.length);
      await emit("search-progress", { repo_id: id, snapshot: s.id, matches: matches.slice(0, room) });
      if (total >= 5000) return { snapshots: hitSnaps, matches: total, truncated: true };
    }
    await delay(i < 3 ? 120 : 45);
  }
  return { snapshots: hitSnaps, matches: total, truncated: false };
}

// «Lo que más ocupa»: carpetas y archivos grandes de ejemplo (rutas de restic).
const A = "/C/Users/Ana";
const BIG_FOLDERS: [string, number, number][] = [
  [`${A}/Documentos`, 2.62e9, 9_840],
  [`${A}/Documentos/Proyectos`, 1.8e9, 2_410],
  [`${A}/Documentos/Proyectos/Vídeos promo`, 1.12e9, 14],
  [`${A}/Imágenes`, 1.36e9, 3_190],
  [`${A}/Imágenes/2025`, 1.2e9, 3_100],
  [`${A}/Documentos/Copias WhatsApp`, 4.6e8, 3],
  [`${A}/Documentos/Proyectos/Cliente Norte/Planos`, 3.1e8, 1_220],
  [`${A}/Imágenes/2025/Boda Laura`, 2.9e8, 412],
  [`${A}/Imágenes/Vacaciones 2026`, 1.4e8, 36],
  [`${A}/Documentos/Facturas`, 2.3e7, 24],
  [`${A}/Vídeos`, 3.9e9, 42],
  [`${A}/Descargas`, 1.35e9, 610],
  [`${A}/Escritorio`, 2.1e8, 188],
];
const BIG_FILES: [string, number][] = [
  [`${A}/Vídeos/Cumpleaños Mateo 4K.mp4`, 2.1e9],
  [`${A}/Descargas/Win11_24H2_Spanish_x64.iso`, 5.8e8],
  [`${A}/Documentos/Proyectos/Vídeos promo/Anuncio 4K final.mov`, 6.4e8],
  [`${A}/Documentos/Proyectos/Vídeos promo/Anuncio 4K v2.mov`, 4.1e8],
  [`${A}/Documentos/Copias WhatsApp/msgstore-2026.db.crypt15`, 3.8e8],
  [`${A}/Vídeos/Graduación.mkv`, 9.2e8],
  [`${A}/Imágenes/2025/Boda Laura/Álbum completo.zip`, 1.9e8],
  [`${A}/Documentos/Proyectos/Cliente Norte/Planos/Modelo 3D.skp`, 8.7e7],
  [`${A}/Documentos/Copias WhatsApp/media-2026.tar`, 7.6e7],
  [`${A}/Descargas/instalador-office.exe`, 4.4e7],
  [`${A}/Imágenes/2025/Panorámica Tatacoa.tif`, 3.9e7],
  [`${A}/Documentos/Proyectos/Presentación clientes.pptx`, 9.6e6],
];

async function snapshotLargest(id: string, snapId: string, limit = 50) {
  const snap = (snapshots[id] ?? []).find((s) => s.id === snapId);
  if (!snap) throw "No se encontró la versión.";
  // Un servidor remoto tarda más en recorrer la versión.
  await delay(/^(rest|s3|sftp):/.test(repos.find((r) => r.id === id)?.location ?? "") ? 4000 : 1600);
  const roots = snap.paths.map((p) => toSnapshotPath(p).toLowerCase());
  const inside = (p: string) => roots.some((r) => p.toLowerCase() === r || p.toLowerCase().startsWith(r + "/"));
  // Se ajustan los tamaños de ejemplo al total de la versión.
  const top = BIG_FOLDERS.filter(([p]) => inside(p) && !BIG_FOLDERS.some(([o]) => o !== p && inside(o) && p.startsWith(o + "/")));
  const total = snap.summary?.total_bytes_processed ?? 0;
  const totalFiles = snap.summary?.total_files_processed ?? 0;
  const sumTop = top.reduce((n, [, size]) => n + size, 0);
  const filesTop = top.reduce((n, [, , files]) => n + files, 0);
  const k = total && sumTop ? Math.min(1, (total * 0.95) / sumTop) : 1;
  const kf = totalFiles && filesTop ? Math.min(1, (totalFiles * 0.95) / filesTop) : 1;
  const folders = BIG_FOLDERS.filter(([p]) => inside(p))
    .map(([path, size, files]) => ({ path, size: Math.round(size * k), files: Math.max(1, Math.round(files * kf)) }))
    .sort((a, b) => b.size - a.size)
    .slice(0, limit);
  const files = BIG_FILES.filter(([p]) => inside(p))
    .map(([path, size]) => ({ path, size: Math.round(size * k), files: 1 }))
    .sort((a, b) => b.size - a.size)
    .slice(0, limit);
  return {
    total_size: Math.max(total, Math.round(sumTop * k)),
    total_files: totalFiles || filesTop,
    folders,
    files,
  };
}

async function simulateRestore(id: string, a: Record<string, any>) {
  if (a.overwrite) await checkPassword(id, a.password ?? "");
  cancelled.delete(`${id}:restore`);
  const total = a.names.length ? a.names.length * 7 : 31;
  const totalBytes = total * 420_000;
  await delay(600);
  for (let i = 1; i <= 50; i++) {
    if (cancelled.has(`${id}:restore`)) throw "Restauración cancelada.";
    const p = i / 50;
    await emit("restore-progress", {
      kind: "status",
      repo_id: id,
      percent: p,
      files_done: Math.round(total * p),
      total_files: total,
      bytes_done: Math.round(totalBytes * p),
      total_bytes: totalBytes,
      seconds_remaining: Math.round((50 - i) * 0.08),
    });
    await delay(80);
  }
  const skipped = a.overwrite ? 0 : Math.floor(total / 5);
  return {
    summary: {
      total_files: total,
      files_restored: total - skipped,
      files_skipped: skipped,
      total_bytes: totalBytes,
      bytes_restored: (total - skipped) * 420_000,
      bytes_skipped: skipped * 420_000,
      seconds_elapsed: 4.1,
    },
    errors: [],
    error_count: 0,
    target: a.target,
  };
}

/** Última copia a mano de cada plan: con «Solo guardar si hay cambios», repetirla enseguida no guarda versión. */
const lastManual = new Map<string, number>();

async function simulateBackup(id: string, planId: string): Promise<BackupResult> {
  const repo = repos.find((r) => r.id === id)!;
  const plan = repo.plans.find((p) => p.id === planId);
  if (!plan) throw "Ese plan ya no existe.";
  if (!plan.paths.length) throw "Este plan no tiene carpetas para copiar.";
  const paths = plan.paths;
  cancelled.delete(id);
  const totalFiles = 18_530;
  const totalBytes = 4.27e9;
  await delay(900);
  const steps = 120;
  for (let i = 1; i <= steps; i++) {
    if (cancelled.has(id)) throw "Copia cancelada.";
    const p = i / steps;
    const eased = 1 - Math.pow(1 - p, 1.6);
    const payload: BackupProgress = {
      kind: "status",
      repo_id: id,
      percent: eased,
      files_done: Math.round(totalFiles * eased),
      total_files: totalFiles,
      bytes_done: Math.round(totalBytes * eased),
      total_bytes: totalBytes,
      seconds_remaining: Math.round((steps - i) * 0.12 * 8),
      current: `${paths[0] ?? "C:\\"}\\Proyectos\\informe-${(i * 37) % 900}.docx`,
    };
    await emit("backup-progress", payload);
    if (i === 70) {
      await emit("backup-progress", {
        kind: "item_error",
        repo_id: id,
        message: `${paths[0]}\\Outlook\\ana.pst: El proceso no tiene acceso al archivo porque está siendo utilizado por otro proceso.`,
      } satisfies BackupProgress);
    }
    await delay(120);
  }
  const key = `${id}#${planId}`;
  const recent = Date.now() - (lastManual.get(key) ?? 0) < 10 * 60_000;
  lastManual.set(key, Date.now());
  if (plan.skip_unchanged && recent) {
    // Como restic con --skip-if-unchanged: termina bien, con resumen y sin versión.
    return {
      summary: {
        files_new: 0,
        files_changed: 0,
        files_unmodified: totalFiles,
        data_added: 0,
        total_files_processed: totalFiles,
        total_bytes_processed: totalBytes,
        total_duration: 6.2,
        snapshot_id: null,
      },
      errors: [],
      error_count: 0,
      incomplete: false,
      unchanged: true,
    };
  }
  const snap = snapshot(0, paths, plan.tags, totalBytes, totalFiles);
  (snapshots[id] ??= []).push(snap);
  return {
    summary: {
      files_new: 42,
      files_changed: 17,
      files_unmodified: totalFiles - 59,
      data_added: 186e6,
      total_files_processed: totalFiles,
      total_bytes_processed: totalBytes,
      total_duration: 15.4,
      snapshot_id: snap.id,
    },
    errors: [`${paths[0]}\\Outlook\\ana.pst: El proceso no tiene acceso al archivo porque está siendo utilizado por otro proceso.`],
    error_count: 1,
    incomplete: true,
  };
}

export function installMocks() {
  console.info(
    `[Resguardo] Modo vista previa: backend simulado. Contraseña de los repositorios de ejemplo: "${MOCK_PASSWORD}". Añade ?vacio a la URL para empezar sin repositorios.`,
  );
  if (new URLSearchParams(location.search).has("vacio")) repos.length = 0;
  // ?novedades: como si la última versión usada fuera la 0.5.14 (se abre «Novedades»).
  if (new URLSearchParams(location.search).has("novedades")) {
    try {
      localStorage.setItem("resguardo.lastVersion", "0.5.14");
    } catch {
      /* sin almacenamiento */
    }
  }
  mockIPC(
    async (cmd, args) => {
      const a = (args ?? {}) as Record<string, any>;
      switch (cmd) {
        case "restic_version":
          return "restic 0.19.1 compiled with go1.26.4 on windows/amd64";
        case "list_repos":
          assignPlaces();
          return structuredClone(repos);
        case "list_places":
          assignPlaces();
          return structuredClone(places);
        case "place_scan": {
          // Como discover::scan: lo de este equipo, lo «encontrado» al listar y lo recordado.
          await delay(700);
          assignPlaces();
          const place = places.find((p) => p.id === a.id);
          if (!place) throw "Ese destino ya no existe.";
          const mine = repos.filter((r) => r.place_id === place.id);
          const rel = (loc: string) =>
            loc.startsWith("s3:") ? loc.replace(/^s3:(https?:\/\/)?[^/]+\/[^/]+\/?/, "") || "." : (loc.split(/[\\/]/).filter(Boolean).pop() ?? loc);
          const found: { location: string; path: string; repo_id: string | null; listed: boolean }[] = mine.map((r) => ({ location: r.location, path: rel(r.location), repo_id: r.id, listed: !place.key.startsWith("rest:") && !place.key.startsWith("sftp:") }));
          let note: string | null = null;
          const sample = mine[0]?.location ?? "";
          if (sample.startsWith("s3:")) {
            const base = sample.replace(/^(s3:(?:https?:\/\/)?[^/]+\/[^/]+).*$/, "$1");
            found.push({ location: `${base}/servidor-oficina`, path: "servidor-oficina", repo_id: null, listed: true });
          } else if (/^[a-z]:/i.test(sample)) {
            const base = sample.replace(/[\\/][^\\/]+$/, "");
            found.push({ location: `${base}\\antiguo-portatil`, path: "antiguo-portatil", repo_id: null, listed: true });
          } else note = "Este tipo de destino no permite listar sus repositorios: aquí están los que Resguardo conoce.";
          return { found: found.sort((x, y) => x.path.localeCompare(y.path)), note };
        }
        case "clone_repo": {
          await checkPassword(a.from, a.password);
          const src = repos.find((r) => r.id === a.from)!;
          const tpl = repos.find((r) => r.id === a.template)!;
          const total = (snapshots[src.id] ?? []).length || 3;
          cancelled.delete(`${a.from}:clone`);
          await emit("clone-progress", { from: a.from, stage: "Creando el repositorio nuevo…", done: 0, total });
          await delay(700);
          for (let i = 1; i <= total; i++) {
            if (cancelled.has(`${a.from}:clone`)) throw "Clonado detenido. Lo ya copiado se queda en el repositorio nuevo; puedes quitarlo o volver a clonar más tarde.";
            await delay(total > 20 ? 60 : 300);
            await emit("clone-progress", { from: a.from, stage: "Copiando las versiones…", done: i, total });
          }
          const repo: Repo = { id: `n${++counter}`, name: a.name, location: a.location, rest_username: tpl.rest_username ?? null, cacert: tpl.cacert ?? null, cloud_key_id: tpl.cloud_key_id ?? null, cloud_region: tpl.cloud_region ?? null, plans: [] };
          passwords[repo.id] = a.passwordNew;
          repos.push(repo);
          assignPlaces();
          snapshots[repo.id] = structuredClone(snapshots[src.id] ?? []);
          logConfig(repo.id, `Repositorio creado como clon de «${src.name}» (${total} versiones copiadas).`);
          return structuredClone(repo);
        }
        case "cancel_clone":
          cancelled.add(`${a.from}:clone`);
          return null;
        case "rename_place": {
          const p = places.find((x) => x.id === a.id);
          if (!p) throw "Ese repositorio ya no existe.";
          if (!String(a.name).trim()) throw "El nombre debe tener entre 1 y 80 caracteres.";
          p.name = String(a.name).trim();
          assignPlaces();
          return structuredClone(p);
        }
        case "list_snapshots":
          await delay(350);
          return structuredClone(snapshots[a.id] ?? []);
        case "add_repo": {
          // Una IP sin servidor simula el caso de "se queda conectando".
          if (/192\.168\.18\.192/.test(a.location)) {
            for (let i = 0; i < 100 && !cancelledChecks.has(a.checkId); i++) await delay(100);
            if (cancelledChecks.has(a.checkId)) throw "Operación cancelada.";
            throw "No se pudo conectar con el servidor. Revisa la dirección, el puerto y que esté encendido.\n\n(restic seguía reintentando tras 40 s y se detuvo.)";
          }
          await delay(900);
          if (a.password === "mal") throw "Contraseña incorrecta.";
          if (/:\/\/[^/@]+:[^/@]+@/.test(a.location)) throw "No incluyas la contraseña del servidor dentro de la ubicación.";
          // Claves de la nube: las de otro destino o unas nuevas (ID y secreta, las dos).
          let cloud: { key: string; region: string | null } | null = null;
          if (a.cloudFrom) {
            const from = repos.find((r) => r.id === a.cloudFrom);
            if (!from?.cloud_key_id) throw "Ese repositorio no tiene claves de nube guardadas.";
            cloud = { key: from.cloud_key_id, region: from.cloud_region ?? null };
          } else if (a.cloudKeyId || a.cloudKeySecret) {
            if (!a.cloudKeyId || !a.cloudKeySecret) throw "Faltan el ID o la clave secreta de la nube.";
            cloud = { key: a.cloudKeyId, region: a.cloudRegion || null };
          }
          const repo: Repo = {
            id: `n${++counter}`,
            name: a.name,
            location: a.location,
            rest_username: (a.restFrom ? repos.find((r) => r.id === a.restFrom)?.rest_username : a.restUsername) ?? null,
            cacert: a.cacert ?? null,
            cloud_key_id: cloud?.key ?? null,
            cloud_region: cloud?.region ?? null,
            plans: [],
          };
          passwords[repo.id] = a.password;
          repos.push(repo);
          assignPlaces();
          snapshots[repo.id] = [];
          return repo;
        }
        case "cancel_add_repo":
          cancelledChecks.add(a.checkId);
          return null;
        case "remove_repo": {
          await checkPassword(a.id, a.password);
          const i = repos.findIndex((r) => r.id === a.id);
          if (i >= 0) repos.splice(i, 1);
          return null;
        }
        case "set_plans": {
          await checkPassword(a.id, a.password);
          const repo = repos.find((r) => r.id === a.id)!;
          const list = a.plans as Plan[];
          if (list.length > 20) throw "Demasiados planes (máximo 20 por repositorio).";
          const clean = (l: string[]) => [...new Set(l.map((x) => x.trim()).filter(Boolean))];
          const out: Plan[] = [];
          for (const p of list) {
            const plan: Plan = {
              ...p,
              id: p.id || `p${++counter}`,
              name: p.name.trim(),
              paths: clean(p.paths),
              excludes: clean(p.excludes),
              tags: clean(p.tags),
            };
            const err = validatePlan(plan, out);
            if (err) throw err;
            out.push(plan);
          }
          for (const [p, message] of planChanges(repo.plans, out)) logConfig(repo.id, message, p);
          repo.plans = out;
          return structuredClone(repo);
        }
        case "move_plan": {
          if (a.from === a.to) throw "El plan ya está en ese repositorio.";
          await checkPassword(a.from, a.passwordFrom);
          await checkPassword(a.to, a.passwordTo);
          const src = repos.find((r) => r.id === a.from);
          const dst = repos.find((r) => r.id === a.to);
          if (!src) throw "Repositorio de origen no encontrado.";
          if (!dst) throw "Repositorio nuevo no encontrado.";
          const pos = src.plans.findIndex((p) => p.id === a.plan);
          if (pos < 0) throw "Ese plan ya no existe.";
          if (dst.plans.length >= 20) throw "El repositorio nuevo ya tiene el máximo de planes (20).";
          const [plan] = src.plans.splice(pos, 1);
          if (dst.plans.some((p) => p.id === plan.id)) plan.id = `p${++counter}`;
          const base = plan.name;
          for (let n = 2; dst.plans.some((p) => p.name.toLowerCase() === plan.name.toLowerCase()); n++) plan.name = `${base} (${n})`;
          dst.plans.push(plan);
          // Como administrador, el agente se actualiza solo en los destinos que copiaba (o que ahora tienen horario).
          if (!new URLSearchParams(location.search).has("noadmin")) {
            for (const repo of [src, dst]) {
              const prev = agentState.repos.find((r) => r.id === repo.id);
              const inAgent = prev?.schedule.kind === "plans";
              const scheduled = repo.plans.some((p) => p.schedule);
              if (!inAgent && !scheduled) continue;
              agentState.repos = agentState.repos.filter((r) => r.id !== repo.id);
              if (scheduled)
                agentState.repos.push({
                  id: repo.id,
                  name: repo.name,
                  location: repo.location,
                  paths: [],
                  excludes: [],
                  schedule: { kind: "plans" },
                  enabled_at: prev?.enabled_at ?? new Date().toISOString(),
                  plans: agentPlans(repo, prev?.plans),
                  verify: prev?.verify ?? null,
                  offsite: prev?.offsite ?? null,
                });
            }
          }
          return structuredClone([src, dst]);
        }
        case "run_backup": {
          // Como el backend: cada copia a mano queda en el historial de la app.
          const repo = repos.find((r) => r.id === a.id);
          const base = {
            kind: "backup" as const,
            origin: "manual" as const,
            repo_id: a.id,
            repo_name: repo?.name ?? "",
            plan_id: a.plan,
            plan_name: repo?.plans.find((p) => p.id === a.plan)?.name ?? null,
            started: new Date().toISOString(),
          };
          try {
            const r = await simulateBackup(a.id, a.plan);
            logActivity({
              ...base,
              finished: new Date().toISOString(),
              result: r.incomplete ? "warning" : "ok",
              message: r.unchanged
                ? "Sin cambios desde la última versión: no se guardó una nueva."
                : r.incomplete
                  ? r.error_count === 1 ? "Copia terminada con 1 archivo que no se pudo leer." : `Copia terminada con ${r.error_count} archivos que no se pudieron leer.`
                  : "Copia completada.",
              unchanged: r.unchanged,
              snapshot_id: r.summary?.snapshot_id ?? null,
              data_added: r.summary?.data_added ?? null,
              files_new: r.summary?.files_new ?? null,
              files_changed: r.summary?.files_changed ?? null,
            });
            return r;
          } catch (e) {
            logActivity({ ...base, finished: new Date().toISOString(), result: "error", message: String(e) });
            throw e;
          }
        }
        case "activity_history":
          // Con ?sinactividad en la URL, el historial está vacío.
          await delay(300);
          if (new URLSearchParams(location.search).has("sinactividad")) return [];
          return structuredClone((activityLog ??= seedActivity()).slice(0, a.limit ?? 500));
        case "cancel_backup":
          cancelled.add(a.id);
          return null;
        case "retention_preview": {
          await delay(400);
          const p = a.policy;
          const within = ["keep_within", "keep_within_hourly", "keep_within_daily", "keep_within_weekly", "keep_within_monthly", "keep_within_yearly"];
          const empty =
            !within.some((k) => p[k]) && !["keep_last", "keep_hourly", "keep_daily", "keep_weekly", "keep_monthly", "keep_yearly"].some((k) => p[k] > 0 || p[k] === -1);
          if (empty) throw "Define al menos una regla para ver qué se conservaría.";
          return mockForget(snapshots[a.id] ?? [], p);
        }
        case "set_retention": {
          await checkPassword(a.id, a.password);
          const repo = repos.find((r) => r.id === a.id)!;
          logConfig(repo.id, !a.policy ? "Retención quitada: se guardan todas las versiones." : repo.retention ? "Retención cambiada." : "Retención definida.");
          repo.retention = a.policy;
          return structuredClone(repo);
        }
        case "repo_stats": {
          const key = [...(a.snapshotIds ?? [])].sort().join(",");
          const hit = statsCache.get(a.id);
          if (!a.force && hit && hit.key === key) return { ...hit.result, cached: true };
          await delay(1500);
          const list = snapshots[a.id] ?? [];
          const latest = Math.max(0, ...list.map((s) => s.summary?.total_bytes_processed ?? 0));
          const result = {
            stats: {
              total_size: latest * 1.18,
              total_uncompressed_size: latest * 1.7,
              compression_ratio: 1.44,
              compression_space_saving: 30.6,
              total_blob_count: 48_210,
              snapshots_count: list.length,
            },
            computed_at: Math.floor(Date.now() / 1000),
            cached: false,
          };
          statsCache.set(a.id, { key, result });
          return result;
        }
        case "rename_repo": {
          await checkPassword(a.id, a.password);
          const repo = repos.find((r) => r.id === a.id)!;
          if (repo.name !== String(a.name).trim()) logConfig(repo.id, `Nombre cambiado: «${repo.name}» → «${String(a.name).trim()}».`);
          repo.name = String(a.name).trim();
          return structuredClone(repo);
        }
        case "plugin:app|version":
          return pkg.version;
        case "plugin:window|set_theme":
          return null;
        case "snapshot_diff": {
          await delay(700);
          const base = "/C/Users/Ana/Documentos";
          const changes = [
            ...["Proyectos/Informe anual 2026.docx", "Proyectos/Presupuesto.xlsx", "Notas.txt"].map((p) => ({ path: `${base}/${p}`, kind: "modified" })),
            ...Array.from({ length: 8 }, (_, i) => ({ path: `${base}/Facturas/factura-2026-${String(25 + i).padStart(3, "0")}.pdf`, kind: "added" })),
            { path: `${base}/Proyectos/Borradores/idea-3.md`, kind: "added" },
            { path: `${base}/Proyectos/Borradores/viejo.md`, kind: "removed" },
            { path: `${base}/CV Ana.pdf`, kind: "metadata" },
            { path: `${base}/Facturas/`, kind: "modified" },
          ];
          return { changes, total: changes.length, stats: { changed_files: 3, added: { files: 9, dirs: 0, bytes: 2.4e6 }, removed: { files: 1, dirs: 0, bytes: 4.1e3 } } };
        }
        case "set_expected_interval": {
          await checkPassword(a.id, a.password);
          const repo = repos.find((r) => r.id === a.id)!;
          repo.expected_hours = a.hours;
          return structuredClone(repo);
        }
        case "web_info":
          return webInfo();
        case "place_share_status": {
          assignPlaces();
          const sample = repos.find((r) => r.place_id === a.id);
          const s = shareState.shared[a.id];
          return {
            shareable: !!sample && /^(s3|b2|azure|gs|rest):/.test(sample.location),
            shared: !!s,
            since: s?.since ?? null,
            delivered: s?.delivered ?? [],
            linked: !!webInfo().link,
            elevated: !new URLSearchParams(location.search).has("noadmin"),
          };
        }
        case "place_share_set": {
          await checkPassword(a.repo, a.password);
          const place = places.find((x) => x.id === a.id);
          if (a.on) shareState.shared[a.id] = { since: new Date().toISOString(), delivered: [] };
          else delete shareState.shared[a.id];
          logConfig(a.repo, a.on ? `Destino «${place?.name}» compartido con mis equipos.` : `Destino «${place?.name}» ya no se comparte.`);
          const s = shareState.shared[a.id];
          return { shareable: true, shared: !!s, since: s?.since ?? null, delivered: s?.delivered ?? [], linked: true, elevated: true };
        }
        case "shares_available": {
          await delay(500);
          if (!account.email) throw "Inicia sesión en «Todos mis equipos» para ver lo que comparten tus equipos.";
          const mine = Object.keys(shareState.shared).map((pid) => {
            const pl = places.find((x) => x.id === pid);
            return { id: `sh-${pid}`, device_id: "d1", device_name: "ESTUDIO", kind: "s3", host: null, base: "", name: pl?.name ?? "Destino", created_at: shareState.shared[pid].since };
          });
          return { shares: [...shareState.others, ...mine], requests: shareState.requests, this_device_id: webInfo().link?.device_id ?? null };
        }
        case "share_request": {
          await delay(400);
          if (!webInfo().link) throw "Vincula antes este equipo con Resguardo Web (Ajustes → Este equipo): el destino llega cifrado para él.";
          if (shareState.requests.some((r) => r.share_id === a.share && r.status === "pending")) throw "Ya lo has pedido; llegará en unos minutos.";
          const req = { id: `rq${++counter}`, share_id: a.share, status: "pending", reason: null, not_before: new Date(Date.now() + 300_000).toISOString(), created_at: new Date().toISOString() };
          shareState.requests.unshift(req);
          // El otro equipo lo entrega en su siguiente ciclo (aquí, a los 6 s).
          setTimeout(() => {
            const sh = shareState.others.find((x) => x.id === a.share);
            if (!sh || req.status !== "pending") return;
            req.status = "received";
            shareState.received.unshift({
              id: sh.id,
              meta: { kind: sh.kind, host: sh.host, base: sh.base, name: sh.name },
              base: sh.kind === "rest" ? `rest:https://${sh.host}/` : `s3:https://${sh.host}/${sh.base}`,
              from_device: sh.device_name,
              received_at: new Date().toISOString(),
            });
            logActivity({ kind: "share", origin: "agent", repo_id: "", repo_name: "", started: new Date().toISOString(), finished: new Date().toISOString(), result: "info", message: `ESTUDIO recibió el destino «${sh.name}» desde «${sh.device_name}».` });
          }, 6000);
          return { id: req.id, not_before: req.not_before };
        }
        case "share_cancel": {
          const r = shareState.requests.find((x) => x.id === a.request && x.status === "pending");
          if (!r) throw "Esa petición ya no se puede cancelar.";
          r.status = "cancelled";
          return null;
        }
        case "server_status":
          return mockServer();
        case "server_setup":
          await delay(900);
          if (!a.path?.trim()) throw "Elige una carpeta completa (por ejemplo, D:\\Copias).";
          Object.assign(serverState, { enabled: true, path: a.path, port: a.port, local_subnet_only: a.localSubnetOnly, tls_sha256: "3F:A1:9C:07:5B:E2:44:D0:8A:61:2C:FE:90:13:B7:6D:28:4E:F5:A9:0B:C3:77:1E:D6:52:8F:04:BA:39:E1:6C" });
          return mockServer();
        case "server_disable":
          serverState.enabled = false;
          return mockServer();
        case "server_add_user": {
          const user = String(a.name).trim().toLowerCase().normalize("NFD").replace(/[\u0300-\u036f]/g, "").replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "");
          if (!user) throw "Escribe un nombre para el equipo (letras, cifras o guiones).";
          if (serverState.users.some((u) => u.name === user)) throw `Ya hay un equipo «${user}» en este servidor.`;
          serverState.users.push({ name: user, created_at: new Date().toISOString(), repos: [], shared: !!webInfo().link });
          return { status: mockServer(), user, password: "9f2c4e81a07b3d65c1e8f4a92b0d7e3c", location: `rest:https://192.168.1.20:${serverState.port}/${user}/` };
        }
        case "server_remove_user":
          serverState.users = serverState.users.filter((u) => u.name !== a.name);
          return mockServer();
        case "server_public_ip":
          await delay(400);
          return "203.0.113.7";
        case "managed_list":
          await delay(250);
          if (new URLSearchParams(location.search).has("noadmin")) throw "Esta acción requiere abrir Resguardo como administrador.";
          return structuredClone(managedState.endpoints);
        case "managed_pair_start": {
          await delay(700);
          if (!serverState.enabled) throw "Activa antes el Servidor de copias (Ajustes → Este equipo): los equipos gestionados copian en él.";
          managedState.pairing = { id: `pa${++counter}`, polls: 0, status: "open" };
          return { pairing_id: managedState.pairing.id, code: "K7QM-4TXR-2W", expires_at: new Date(Date.now() + 15 * 60_000).toISOString() };
        }
        case "managed_pair_poll": {
          await delay(200);
          const p = managedState.pairing;
          if (!p || p.id !== a.pairing) return { status: "expired", expires_at: null, endpoint_name: null, sas: null };
          // El equipo «se une» al tercer sondeo.
          if (p.status === "open" && ++p.polls >= 3) p.status = "joined";
          const joined = p.status !== "open";
          return { status: p.status, expires_at: new Date(Date.now() + 15 * 60_000).toISOString(), endpoint_name: joined ? "ALMACEN-PC" : null, sas: joined ? "482 913" : null };
        }
        case "managed_pair_confirm": {
          await delay(1600);
          const p = managedState.pairing;
          if (!p || p.status !== "joined") throw "El equipo aún no se ha unido.";
          p.status = "confirmed";
          serverState.users.push({ name: "almacen-pc", created_at: new Date().toISOString(), repos: ["equipo"], shared: false });
          const e: import("$lib/api").ManagedEndpoint = { device_id: `m${++counter}`, name: "ALMACEN-PC", server_user: "almacen-pc", location: "rest:https://192.168.1.20:8000/almacen-pc/equipo/", plans: [], paired_at: new Date().toISOString(), stopped: false, tray: true, tray_toasts: false, retention: "frecuente", last_prune: null, last_prune_error: null, last_snapshot: null, snapshots: 0, bytes: 0 };
          managedState.endpoints.push(e);
          return structuredClone(e);
        }
        case "managed_pair_cancel":
          await delay(300);
          if (managedState.pairing) managedState.pairing.status = "cancelled";
          return;
        case "managed_set_config": {
          await delay(600);
          const e = managedState.endpoints.find((x) => x.device_id === a.device);
          if (!e) throw "Ese equipo no está gestionado por esta consola.";
          Object.assign(e, { plans: a.plans, tray: a.tray, tray_toasts: a.trayToasts });
          return structuredClone(e);
        }
        case "managed_backup_now":
          await delay(500);
          return;
        case "managed_set_retention": {
          await delay(300);
          const e = managedState.endpoints.find((x) => x.device_id === a.device);
          if (!e) throw "Ese equipo no está gestionado por esta consola.";
          e.retention = a.retention;
          return structuredClone(e);
        }
        case "managed_prune_now": {
          await delay(2500);
          const e = managedState.endpoints.find((x) => x.device_id === a.device);
          if (e) Object.assign(e, { last_prune: new Date().toISOString(), last_prune_error: null });
          return;
        }
        case "managed_agent_installer":
          return { available: !new URLSearchParams(location.search).has("sinagente"), version: "0.6.8", file_name: "Resguardo-Agente-0.6.8-setup.exe" };
        case "managed_save_agent_installer":
          await delay(700);
          return { path: `${a.folder}\\Resguardo-Agente-0.6.8-setup.exe`, sha256: "5f0c1d3e9a2b47c8e6d1f0a9b8c7d6e5f4a3b2c1d0e9f8a7b6c5d4e3f2a1b0c9" };
        case "unlock_repo":
          await delay(900);
          return;
        case "update_saved_password":
          await delay(900);
          if (a.password !== MOCK_PASSWORD) throw "Esa contraseña no abre el repositorio: no se ha cambiado nada.";
          return "agente";
        case "managed_unpair": {
          await delay(500);
          const e = managedState.endpoints.find((x) => x.device_id === a.device);
          if (!e) throw "Ese equipo no está gestionado por esta consola.";
          e.stopped = true;
          return structuredClone(e);
        }
        case "managed_open_repo": {
          await delay(800);
          const e = managedState.endpoints.find((x) => x.device_id === a.device);
          if (!e) throw "Ese equipo no está gestionado por esta consola.";
          const loc = e.location.replace("192.168.1.20", "localhost");
          const existing = repos.find((r) => r.location === loc);
          if (existing) return structuredClone(existing);
          const repo: Repo = { id: `n${++counter}`, name: `${e.name} (equipo gestionado)`, location: loc, rest_username: e.server_user, cacert: null, cloud_key_id: null, cloud_region: null, plans: [] };
          passwords[repo.id] = MOCK_PASSWORD;
          repos.push(repo);
          assignPlaces();
          snapshots[repo.id] = [];
          return structuredClone(repo);
        }
        case "shares_received":
          await delay(300);
          if (new URLSearchParams(location.search).has("noadmin")) throw "Esta acción requiere abrir Resguardo como administrador.";
          return structuredClone(shareState.received);
        case "received_share_add": {
          await delay(900);
          const r = shareState.received.find((x) => x.id === a.id);
          if (!r) throw "Ese destino compartido ya no está en este equipo.";
          if (!a.location.startsWith(r.base.replace(/\/+$/, ""))) throw "La ubicación tiene que estar dentro del destino compartido.";
          if (!a.create && a.password !== MOCK_PASSWORD) throw "Contraseña incorrecta.";
          const repo: Repo = { id: `n${++counter}`, name: a.name, location: a.location, rest_username: r.meta.kind === "rest" ? "estudio" : null, cacert: null, cloud_key_id: r.meta.kind === "rest" ? null : "004compartida", cloud_region: null, plans: [] };
          passwords[repo.id] = a.password;
          repos.push(repo);
          assignPlaces();
          snapshots[repo.id] = [];
          logConfig(repo.id, `Repositorio ${a.create ? "creado" : "conectado"} en el destino compartido «${r.meta.name}» (desde «${r.from_device}»).`);
          return structuredClone(repo);
        }
        case "web_pair": {
          await delay(900);
          if (String(a.code).replace("-", "") !== "ABCDEFGH") throw "La web respondió: Código no válido o caducado";
          webLink = { url: a.url, key: a.key, device_id: "d1", device_name: a.name, paired_at: new Date().toISOString(), revoked: false };
          return webInfo();
        }
        case "web_unpair":
          webLink = null;
          presetLink = false;
          return webInfo();
        case "web_report_now":
          await delay(600);
          return webInfo();
        case "running_jobs":
          return [];
        case "close_app":
          return null;
        case "agent_repair":
          return agentInfo();
        case "agent_info":
          return agentInfo();
        case "agent_log_detail":
          if (new URLSearchParams(location.search).has("noadmin")) throw "Esta acción requiere abrir Resguardo como administrador.";
          return [
            `${new Date(Date.now() - 65 * 60_000).toISOString().slice(0, 19).replace("T", " ")} COPIA [r1#p2] empieza «Disco externo» / «Domingo»`,
            `${new Date(Date.now() - 64 * 60_000).toISOString().slice(0, 19).replace("T", " ")} ARCHIVO [r1#p2] C:\\Users\\Ana\\Documentos\\Outlook\\correo.pst: The process cannot access the file because it is being used by another process.`,
            `${new Date(Date.now() - 64 * 60_000).toISOString().slice(0, 19).replace("T", " ")} AVISO: Copia de «Disco externo» («Domingo»): Copia terminada, pero algunos archivos no se pudieron leer.`,
          ];
        case "agent_failed_files":
          if (new URLSearchParams(location.search).has("noadmin")) throw "Esta acción requiere abrir Resguardo como administrador.";
          await delay(300);
          return [
            "C:\\Users\\Ana\\Documentos\\Outlook\\correo.pst: The process cannot access the file because it is being used by another process.",
            "C:\\Users\\Ana\\AppData\\Local\\Temp\\bloqueado.tmp: Access is denied.",
          ];
        case "agent_log": {
          const t = (min: number) => new Date(Date.now() - min * 60_000).toISOString().slice(0, 19).replace("T", " ");
          return [
            `${t(65)} Copia de «Disco externo»…`,
            `${t(64)} «Disco externo»: Copia completada.`,
            `${t(35)} Informe a la web: No se pudo conectar con la web: timeout`,
            `${t(5)} Copia de «Servidor de casa»…`,
            `${t(4)} AVISO: «Servidor de casa»: Copia terminada, pero algunos archivos no se pudieron leer.`,
          ];
        }
        case "agent_set_schedule": {
          await checkPassword(a.id, a.password);
          const repo = repos.find((r) => r.id === a.id)!;
          const kind = a.schedule?.kind;
          if (kind && kind !== "plans" && kind !== "monitor") throw "Programa las copias con los planes del repositorio.";
          const previous = agentState.repos.find((r) => r.id === a.id);
          const plans = kind === "plans" ? agentPlans(repo, previous?.plans) : [];
          // Sin planes con horario se conserva si tiene verificación o copia externa.
          if (kind === "plans" && !plans.length && !previous?.verify && !previous?.offsite)
            throw "Ningún plan tiene horario: añade días y horas a algún plan.";
          agentState.repos = agentState.repos.filter((r) => r.id !== a.id);
          if (a.schedule)
            agentState.repos.push({
              id: repo.id,
              name: repo.name,
              location: repo.location,
              paths: [],
              excludes: [],
              schedule: a.schedule,
              enabled_at: new Date().toISOString(),
              plans,
              verify: previous?.verify ?? null,
              offsite: previous?.offsite ?? null,
            });
          return agentInfo();
        }
        case "agent_set_verify": {
          await checkPassword(a.id, a.password);
          const r = agentState.repos.find((x) => x.id === a.id);
          if (!r) throw "Activa primero las copias automáticas o «Solo vigilar» en este repositorio.";
          logConfig(a.id, !a.verify ? "Verificación quitada." : r.verify ? "Verificación cambiada." : "Verificación programada.");
          r.verify = a.verify ? { ...a.verify, rotate_parts: a.verify.rotate_parts ?? 0, enabled_at: new Date().toISOString() } : null;
          return agentInfo();
        }
        case "offsite_prepare":
          await checkPassword(a.id, a.password);
          await delay(900);
          if (!a.creds?.key_id) throw "El servidor rechazó el usuario o la contraseña (403).";
          return a.location.includes("existente") ? "existing" : "created";
        case "agent_set_offsite": {
          await checkPassword(a.id, a.password);
          const r = agentState.repos.find((x) => x.id === a.id);
          if (!r) throw "Activa primero las copias automáticas o «Solo vigilar» en este repositorio.";
          const repo = repos.find((x) => x.id === a.id)!;
          if (a.offsite?.target) {
            // Hacia otro destino de la app: su ubicación y sus credenciales.
            if (a.offsite.target === a.id) throw "El repositorio de la copia externa debe ser otro.";
            const tgt = repos.find((x) => x.id === a.offsite.target);
            if (!tgt) throw "Repositorio no encontrado.";
            if (!a.passwordTarget) throw `Falta la contraseña de «${tgt.name}».`;
            await checkPassword(tgt.id, a.passwordTarget);
            r.offsite = {
              location: tgt.location,
              provider: `destino:${tgt.id}`,
              region: tgt.cloud_region ?? null,
              schedule: a.offsite.schedule,
              limit_upload_kib: a.offsite.limit_upload_kib ?? null,
              // La retención propia de ese destino (si solo recibe esta copia externa).
              retention: a.offsite.apply_retention ? (tgt.retention ?? null) : null,
              guard: a.offsite.guard ?? null,
              enabled_at: new Date().toISOString(),
            };
            return agentInfo();
          }
          r.offsite = a.offsite
            ? { ...a.offsite, retention: a.offsite.apply_retention ? repo.retention : null, enabled_at: new Date().toISOString() }
            : null;
          return agentInfo();
        }
        case "agent_pause": {
          // Como `agent::set_pause`: contraseña, administrador y como mucho 30 días.
          await checkPassword(a.id, a.password);
          if (new URLSearchParams(location.search).has("noadmin")) throw "Esta acción requiere abrir Resguardo como administrador.";
          const r = agentState.repos.find((x) => x.id === a.id);
          if (!r) throw "Este repositorio no tiene copias automáticas en este equipo.";
          if (a.until) {
            const t = new Date(a.until).getTime();
            if (!(t > Date.now())) throw "Elige un momento futuro para reanudar las copias.";
            if (t > Date.now() + 30 * 24 * HOUR) throw "La pausa puede durar como mucho 30 días. Para más tiempo, elige «Hasta que la reanude».";
          }
          const active = r.pause && (!r.pause.until || new Date(r.pause.until).getTime() > Date.now());
          r.pause = { since: active ? r.pause.since : new Date().toISOString(), until: a.until ?? null };
          const now = new Date().toISOString();
          logActivity({
            kind: "pause",
            origin: "manual",
            repo_id: r.id,
            repo_name: r.name,
            started: now,
            finished: now,
            result: "info",
            message: a.until ? `Hasta el ${new Date(a.until).toLocaleString("es", { dateStyle: "full", timeStyle: "short" })}.` : "Hasta que las reanudes.",
          });
          return agentInfo();
        }
        case "agent_resume": {
          await checkPassword(a.id, a.password);
          if (new URLSearchParams(location.search).has("noadmin")) throw "Esta acción requiere abrir Resguardo como administrador.";
          const r = agentState.repos.find((x) => x.id === a.id);
          if (!r) throw "Este repositorio no tiene copias automáticas en este equipo.";
          if (r.pause) {
            r.pause = null;
            const now = new Date().toISOString();
            logActivity({ kind: "resume", origin: "manual", repo_id: r.id, repo_name: r.name, started: now, finished: now, result: "info", message: "Reanudadas a mano." });
          }
          return agentInfo();
        }
        case "agent_offsite_resume": {
          await checkPassword(a.id, a.password);
          if (new URLSearchParams(location.search).has("noadmin")) throw "Esta acción requiere abrir Resguardo como administrador.";
          const hold = offsiteHolds[a.id];
          if (hold) {
            delete offsiteHolds[a.id];
            const now = new Date().toISOString();
            logActivity({
              kind: "guard",
              origin: "manual",
              repo_id: a.id,
              repo_name: repos.find((r) => r.id === a.id)?.name ?? "",
              started: now,
              finished: now,
              result: "info",
              message: "Subida reanudada: el cambio era normal.",
              snapshot_id: hold.snapshot_id,
            });
          }
          return agentInfo();
        }
        case "agent_set_restore_test": {
          await checkPassword(a.id, a.password);
          const r = agentState.repos.find((x) => x.id === a.id);
          if (!r) throw "Activa primero las copias automáticas o «Solo vigilar» en este repositorio.";
          logConfig(a.id, !a.restoreTest ? "Prueba de restauración quitada." : r.restore_test ? "Prueba de restauración cambiada." : "Prueba de restauración programada.");
          r.restore_test = a.restoreTest ? { ...a.restoreTest, enabled_at: new Date().toISOString() } : null;
          return agentInfo();
        }
        case "agent_task_now": {
          // Simula la tarea: en curso unos segundos y después el resultado.
          agentTasks.running = { repo_id: a.id, kind: a.kind, started: new Date().toISOString(), stage: a.kind === "restore_test" ? "Restaurando y comprobando…" : a.kind !== "offsite" ? "Leyendo el 5 % de los datos…" : "Subiendo la copia del 2026-09-29 06:34 (2.ª)…", done: 2, updated: new Date().toISOString() };
          setTimeout(() => {
            agentTasks.running = null;
            const done = {
              started: new Date(Date.now() - 8000).toISOString(),
              finished: new Date().toISOString(),
              result: "ok" as const,
              message: a.kind === "restore_test"
                ? "20 archivos (85 MB) de la versión del 30/09 18:00 restaurados y comprobados."
                : a.kind !== "offsite" ? "Sin errores (se leyó el 5 % de los datos)." : "3 copias subidas. Retención aplicada en el repositorio.",
              files_new: a.kind === "offsite" ? 3 : null,
            };
            agentTasks.runs[`${a.kind}:${a.id}`] = done;
            logActivity({ ...done, kind: a.kind, origin: "agent", repo_id: a.id, repo_name: repos.find((r) => r.id === a.id)?.name ?? "" });
          }, 8000);
          return null;
        }
        case "relaunch_as_admin":
          throw "En la vista previa no se puede reabrir como administrador.";
        case "recovery_info": {
          // Como `kit::entry`: sin secretos; el ID del repositorio, inventado pero estable.
          await delay(600);
          const list = repos.filter((r) => !a.ids || a.ids.includes(r.id));
          return list.map((r) => {
            const loc = r.location;
            const kind = loc.startsWith("rest:") ? "rest" : loc.startsWith("sftp:") ? "sftp" : loc.startsWith("s3:") ? "s3" : loc.startsWith("b2:") ? "b2" : "local";
            const pub = loc.replace(/^(rest:https?:\/\/)[^/@]*@/, "$1");
            let cloud = null;
            if (kind === "s3") {
              const m = /^s3:https?:\/\/([^/]+)\/([^/]+)\/?(.*)$/.exec(loc);
              cloud = { provider: "s3", endpoint: m?.[1] ?? null, bucket: m?.[2] ?? null, prefix: m?.[3] || null, key_id: r.cloud_key_id ?? null, region: r.cloud_region ?? null };
            }
            const seed = [...r.id].reduce((n, c) => n * 31 + c.charCodeAt(0), 7);
            const configId = Array.from({ length: 64 }, (_, i) => "0123456789abcdef"[(seed * (i + 3) * 2654435761) % 16 >>> 0]).join("");
            return {
              id: r.id,
              name: r.name,
              kind,
              location: pub,
              rest_path: kind === "rest" ? new URL(loc.slice(5)).pathname.replace(/^\/+|\/+$/g, "") : null,
              rest_auth: !!r.rest_username,
              cloud,
              // «NAS por SFTP» no responde: el kit se imprime sin su ID.
              config_id: kind === "sftp" ? null : configId,
              error: kind === "sftp" ? "No se pudo conectar con el servidor." : null,
              kit: r.kit ?? null,
            };
          });
        }
        case "protection_status":
          await delay(150);
          return mockProtection(a.id);
        case "probe_append_only": {
          // «Servidor de casa» es de solo añadir.
          await delay(500);
          const repo = repos.find((r) => r.id === a.id)!;
          if (repo.location.startsWith("rest:")) repo.append_only = { checked_at: new Date().toISOString(), append_only: true };
          return structuredClone(repo);
        }
        case "set_object_lock": {
          await checkPassword(a.id, a.password);
          const repo = repos.find((r) => r.id === a.id)!;
          if (repo.object_lock !== a.on) logConfig(repo.id, a.on ? "Marcado como bucket con bloqueo de objetos." : "Ya no se cuenta con bloqueo de objetos.");
          repo.object_lock = a.on;
          return structuredClone(repo);
        }
        case "kit_confirm": {
          await checkPassword(a.id, a.password ?? "");
          const repo = repos.find((r) => r.id === a.id)!;
          repo.kit = { saved_at: new Date().toISOString(), location: repo.location, config_id: a.configId ?? null };
          logConfig(repo.id, "Kit de recuperación guardado.", undefined, "kit");
          return structuredClone(repo);
        }
        case "check_password":
          return checkPassword(a.id, a.password);
        case "list_snapshot_dir":
          await delay(250);
          return listTree(a.dir);
        case "snapshot_largest":
          return snapshotLargest(a.id, a.snapshot, a.limit ?? 50);
        case "inspect_target":
          // Para probar la opción de reemplazar: cualquier ruta que contenga "Documentos" "tiene contenido".
          return { exists: /documentos/i.test(a.path), empty: !/documentos/i.test(a.path) };
        case "run_restore":
          return simulateRestore(a.id, a);
        case "cancel_restore":
          cancelled.add(`${a.id}:restore`);
          return null;
        case "app_lock_status":
          return { ...lockState, explain: lockState.availability === "available" ? "" : HELLO_EXPLAIN };
        case "app_lock_unlock":
          if (!lockState.locked) return { result: "verified", message: null };
          if (lockState.availability !== "available") {
            lockState.locked = false;
            return { result: "unavailable", message: `${HELLO_EXPLAIN} Por eso Resguardo se ha abierto sin pedirla.` };
          }
          await delay(900);
          lockState.locked = false;
          return { result: "verified", message: null };
        case "app_lock_set":
          if (lockState.availability !== "available") {
            if (a.enabled) throw HELLO_EXPLAIN;
          } else await delay(900);
          Object.assign(lockState, { enabled: a.enabled, idle_minutes: a.enabled ? (a.idleMinutes ?? null) : null, locked: false });
          return { ...lockState, explain: lockState.availability === "available" ? "" : HELLO_EXPLAIN };
        case "versions_pending": {
          const v = versionsPath;
          versionsPath = null;
          return v;
        }
        case "take_view": {
          const w = window as unknown as { __vista?: unknown };
          const v = w.__vista ?? null;
          w.__vista = null;
          return v;
        }
        case "locate_path":
          return locateMock(a.path);
        case "restore_version": {
          await delay(1200);
          const loc = locateMock(a.path);
          return `${loc.parent}\\${a.name}`;
        }
        case "shell_menu_status":
          return shellMenu;
        case "shell_menu_set":
          await delay(150);
          shellMenu = a.enabled;
          return shellMenu;
        case "tray_update":
          (window as unknown as { __bandeja: unknown }).__bandeja = a.status;
          return null;
        case "tray_settings":
          return { ...trayState };
        case "tray_set":
          await delay(150);
          Object.assign(trayState, { close_to_tray: a.closeToTray, notifications: a.notifications, autostart: a.autostart });
          return { ...trayState };
        case "app_lock_lock":
          if (lockState.enabled) lockState.locked = true;
          return null;
        case "account_status":
          return accountStatus();
        case "account_sign_in":
          await delay(600);
          if (!a.email || a.password !== "resguardo") throw "Correo o contraseña incorrectos.";
          account.pending = a.email;
          return accountStatus();
        case "account_verify":
          await delay(600);
          if (String(a.code).replace(/\D/g, "") !== "123456") throw "El código no es correcto o ya caducó.";
          account.email = account.pending;
          account.pending = null;
          return accountStatus();
        case "account_sign_out":
          account.email = null;
          account.pending = null;
          return accountStatus();
        case "account_overview":
          if (!account.email) throw "Inicia sesión con tu cuenta de Resguardo Web.";
          await delay(500);
          return mockOverview();
        case "account_request_backup": {
          const dev = mockDevices().find((d) => d.id === a.device);
          if (!dev?.remote_backup_enabled) throw "Este equipo no permite copias a distancia. Actívalo en Resguardo, en ese equipo.";
          if (account.commands.some((c) => c.device_id === a.device && c.repo_id === a.repo && Date.now() - c.at < 40_000))
            throw "Ya hay una copia de este repositorio pedida; espera a que termine.";
          const id = `cmd-${account.commands.length + 1}`;
          account.commands.push({ id, device_id: a.device, repo_id: a.repo, plan_id: a.plan, at: Date.now() });
          return id;
        }
        case "account_command": {
          const c = account.commands.find((x) => x.id === a.id);
          if (!c) return null;
          const age = Date.now() - c.at;
          const status = age < 12_000 ? "pending" : age < 30_000 ? "claimed" : "done";
          return { id: c.id, device_id: c.device_id, repo_id: c.repo_id, plan_id: c.plan_id, requested_at: new Date(c.at).toISOString(), requested_from: "ESTUDIO", status, message: status === "done" ? "Copia completada." : null };
        }
        case "agent_set_discreet":
          await checkPassword(a.id, a.password);
          (agentState as any).discreet = a.discreet;
          return agentInfo();
        case "agent_set_remote_backup":
          await checkPassword(a.id, a.password);
          (agentState as any).remote_backup = a.enabled;
          return agentInfo();
        case "search_files":
          return simulateSearch(a.id, a.req);
        case "cancel_search":
          cancelled.add(`${a.id}:search`);
          return null;
        case "plugin:path|resolve_directory":
          // BaseDirectory.Desktop = 18 (ver @tauri-apps/api/path).
          return a.directory === 18 ? "C:\\Users\\Ana\\Desktop" : "C:\\Users\\Ana\\Downloads";
        case "plugin:opener|reveal_item_in_dir":
          return null;
        case "plugin:dialog|open":
          if (a.options?.filters) return "C:\\certs\\mi-ca.pem";
          return a.options?.multiple ? ["C:\\Users\\Ana\\Música"] : "D:\\Copias\\restic";
        default:
          throw `Comando simulado no implementado: ${cmd}`;
      }
    },
    { shouldMockEvents: true },
  );
}
