// El espejo del almacén por destino (docs/espejo.md): horario, «después de
// cada copia nueva» y lo que se manda al equipo. Sin dependencias de Svelte,
// para las pruebas (scripts/vectores-espejo.ts).
import { horarioEnFrase } from "./formato";
import { usuarioEnAlmacen } from "./direccion";
import type { Equipo, Horario } from "./tipos";

/** El agente entiende el espejo por destino (horario, selección, retención y verificación). */
export const ADMITE_FLEXIBLE = "espejo_flexible";
/** §3c: el agente conecta también B2, S3, SFTP, SMB y WebDAV (`conectar_nube` con datos). */
export const ADMITE_DESTINOS = "espejo_destinos";
export const admiteMasDestinos = (e: Pick<Equipo, "resumen"> | null | undefined) => !!e?.resumen?.admite?.includes(ADMITE_DESTINOS);

/** §3c: los tipos de nube o destino por rclone, con su nombre y si pueden ser inmutables. */
export const TIPOS_NUBE: Record<string, { nombre: string; inmutable: boolean }> = {
  dropbox: { nombre: "Dropbox", inmutable: false },
  drive: { nombre: "Google Drive", inmutable: false },
  b2: { nombre: "Backblaze B2", inmutable: true },
  s3: { nombre: "S3 compatible", inmutable: true },
  sftp: { nombre: "SFTP", inmutable: false },
  smb: { nombre: "Carpeta de red (SMB)", inmutable: false },
  webdav: { nombre: "WebDAV", inmutable: false },
};
export const nombreTipoNube = (t: string) => TIPOS_NUBE[t]?.nombre ?? t;

/** §3c: «Disco E:» para una carpeta de Windows (varios discos del almacén, cada uno claro); si no, «Otra carpeta». */
export function etiquetaCarpeta(carpeta: string | null | undefined): string {
  const m = /^([A-Za-z]):[\\/]/.exec(carpeta ?? "");
  return m ? `Disco ${m[1].toUpperCase()}:` : "Otra carpeta";
}

/** Un campo de un destino por rclone (lo que se pide en «Conectar otro destino»). */
export interface CampoDestino {
  clave: string;
  etiqueta: string;
  /** Contraseña o clave: con CampoClave y sin guardarlo en ningún sitio. */
  secreto?: boolean;
  opcional?: boolean;
  opciones?: string[];
  ayuda?: string;
  ejemplo?: string;
  largo?: boolean;
}

/** §3c: lo que pide cada tipo (lo mismo que comprueba el agente). */
export const CAMPOS_DESTINO: Record<"b2" | "s3" | "sftp" | "smb" | "webdav", CampoDestino[]> = {
  b2: [
    { clave: "cuenta", etiqueta: "keyID de la clave de aplicación", ejemplo: "0012ab34cd56ef7000000000a" },
    { clave: "clave", etiqueta: "Clave de aplicación", secreto: true, ayuda: "Mejor una clave limitada al bucket del espejo y sin permiso para saltarse el bloqueo de objetos." },
  ],
  s3: [
    { clave: "proveedor", etiqueta: "Proveedor", opcional: true, opciones: ["Other", "AWS", "Wasabi", "Minio", "Cloudflare", "Ceph", "DigitalOcean", "IDrive", "Scaleway", "IONOS", "Storj"] },
    { clave: "endpoint", etiqueta: "Dirección del servicio (endpoint)", opcional: true, ejemplo: "https://s3.eu-central-003.ejemplo.com", ayuda: "Con https. En AWS se puede dejar vacía." },
    { clave: "region", etiqueta: "Región", opcional: true, ejemplo: "eu-central-003" },
    { clave: "id_clave", etiqueta: "Id de la clave de acceso" },
    { clave: "clave", etiqueta: "Clave secreta", secreto: true },
  ],
  sftp: [
    { clave: "host", etiqueta: "Servidor", ejemplo: "nas.oficina.lan" },
    { clave: "puerto", etiqueta: "Puerto", opcional: true, ejemplo: "22" },
    { clave: "usuario", etiqueta: "Usuario" },
    { clave: "contrasena", etiqueta: "Contraseña", secreto: true },
    {
      clave: "clave_host",
      etiqueta: "Clave pública del servidor",
      largo: true,
      ejemplo: "ssh-ed25519 AAAA…",
      ayuda: "La de «ssh-keyscan servidor» o el archivo /etc/ssh/ssh_host_ed25519_key.pub del servidor. Con ella el equipo comprueba que habla con ese servidor y no con otro que se haga pasar por él.",
    },
  ],
  smb: [
    { clave: "host", etiqueta: "Servidor (NAS)", ejemplo: "nas.oficina.lan" },
    { clave: "usuario", etiqueta: "Usuario", ayuda: "Mejor uno solo para el espejo, que no use nadie más." },
    { clave: "contrasena", etiqueta: "Contraseña", secreto: true },
    { clave: "dominio", etiqueta: "Dominio", opcional: true },
    { clave: "puerto", etiqueta: "Puerto", opcional: true, ejemplo: "445" },
  ],
  webdav: [
    { clave: "url", etiqueta: "Dirección (https)", ejemplo: "https://nube.ejemplo.com/remote.php/dav/files/copias/" },
    { clave: "proveedor", etiqueta: "Servicio", opcional: true, opciones: ["other", "nextcloud", "owncloud"] },
    { clave: "usuario", etiqueta: "Usuario" },
    { clave: "contrasena", etiqueta: "Contraseña", secreto: true, ayuda: "En Nextcloud, mejor una contraseña de aplicación." },
  ],
};

/** El error de un campo (o null): lo mismo que rechazaría el agente, en palabras. */
export function errorCampoDestino(tipo: keyof typeof CAMPOS_DESTINO, clave: string, valor: string): string | null {
  const v = valor.trim();
  const campo = CAMPOS_DESTINO[tipo].find((c) => c.clave === clave);
  if (!campo) return null;
  if (!v) return campo.opcional ? null : "Falta.";
  if (campo.secreto) return valor.length > 500 || /[\u0000-\u001f]/.test(valor) ? "No es válida." : null;
  if (clave === "puerto") return /^\d{1,5}$/.test(v) && Number(v) > 0 && Number(v) < 65536 ? null : "Un número de 1 a 65535.";
  if (clave === "host") return /^\[?[A-Za-z0-9.:-]{1,253}\]?$/.test(v) && !v.startsWith("-") ? null : "Solo el nombre o la IP del servidor.";
  if (clave === "url" || (clave === "endpoint" && v.includes("://"))) {
    if (!/^https:\/\/[^\s/@]+(\/\S*)?$/.test(v)) return "Tiene que empezar por https:// (sin usuario ni contraseña en ella).";
    return null;
  }
  if (clave === "clave_host") return /^(?:\S+\s+)?(ssh-ed25519|ecdsa-sha2-nistp(256|384|521)|ssh-rsa)\s+[A-Za-z0-9+/=]{16,2000}(\s.*)?$/.test(v) ? null : "Pega la línea completa: «ssh-ed25519 AAAA…».";
  return /\s/.test(v) || v.length > 500 ? "Sin espacios." : null;
}

export const admiteEspejoFlexible = (e: Pick<Equipo, "resumen"> | null | undefined) => !!e?.resumen?.admite?.includes(ADMITE_FLEXIBLE);

/** Un destino del espejo como lo da el resumen del equipo. */
export interface DestinoEspejoResumen {
  tipo: "carpeta" | "nube";
  carpeta?: string | null;
  nube?: string | null;
  ultima?: string | null;
  resultado?: string | null;
  /** Su horario propio (sin él, cada día a `espejo.hora`). */
  horario?: Horario | null;
  /** También después de cada copia nueva. */
  tras_copia?: boolean | null;
  /** La próxima vuelta por horario. */
  proxima?: string | null;
  /** §3f: solo estos repositorios (`<usuario>` o `<usuario>/<repo>`); sin ellos, todos. */
  repos?: string[] | null;
  /** §3f: los repositorios que había al elegir la selección (los demás son nuevos). */
  vistos?: string[] | null;
  /** §3d: % de lo que hay en el destino que se comprueba cada día. */
  verificar_pct?: number | null;
  /** §3d: la última comprobación del destino. */
  verificacion?: { ultima: string; archivos: number; mal: number } | null;
  /** §3d: archivos dañados del almacén que no se copiaron en la última vuelta. */
  danados_origen?: number | null;
  /** §3b: borra lo que ya no está en el almacén pasados estos días (sin ello, nunca borra). */
  retencion_dias?: number | null;
  /** §3b: bloqueo de objetos: nunca se borra. */
  bloqueo?: boolean | null;
  /** §3b: lo que espera para borrarse. */
  por_borrar?: { archivos: number; bytes: number; primero?: string | null } | null;
  /** §3b: el freno de la última vuelta (no se anotó ni se borró nada). */
  freno?: string | null;
}

/** Lo que se manda de un destino en `guarda_copias.espejo.destinos` (sin sus resultados). */
export interface DestinoEspejoOrden {
  tipo: "carpeta" | "nube";
  carpeta: string;
  nube?: string;
  horario?: Horario;
  tras_copia?: boolean;
  repos?: string[];
  vistos?: string[];
  verificar_pct?: number;
  retencion_dias?: number;
  bloqueo?: boolean;
}

/** §3b: días de retención del espejo: por defecto, mínimo y máximo (como el agente). */
export const RETENCION_ESPEJO = { defecto: 30, min: 7, max: 3650 } as const;

/** §3b: «Nunca borra», «Con bloqueo de objetos: nunca borra» o «Borra lo que ya no está en el almacén a los 30 días». */
export function textoRetencion(d: Pick<DestinoEspejoResumen, "retencion_dias" | "bloqueo">): string {
  if (d.bloqueo) return "Con bloqueo de objetos: nunca borra";
  if (!d.retencion_dias) return "Nunca borra";
  return `Borra lo que ya no está en el almacén a los ${d.retencion_dias} días`;
}

/** §3b: el error de unos días de retención escritos a mano (o null). */
export function errorDiasRetencion(n: number): string | null {
  return Number.isInteger(n) && n >= RETENCION_ESPEJO.min && n <= RETENCION_ESPEJO.max ? null : `Entre ${RETENCION_ESPEJO.min} y ${RETENCION_ESPEJO.max} días.`;
}

/** Un destino del resumen en la forma de la orden: lo que ya tiene, para reenviarlo sin cambios. */
export function destinoParaOrden(d: DestinoEspejoResumen): DestinoEspejoOrden {
  const o: DestinoEspejoOrden = d.tipo === "nube" ? { tipo: "nube", nube: d.nube ?? "", carpeta: d.carpeta ?? "" } : { tipo: "carpeta", carpeta: d.carpeta ?? "" };
  if (d.horario && (d.horario.reglas?.length || d.horario.horas?.length)) o.horario = d.horario;
  if (d.tras_copia) o.tras_copia = true;
  if (Array.isArray(d.repos)) {
    o.repos = [...d.repos];
    o.vistos = [...(d.vistos ?? [])];
  }
  if (typeof d.verificar_pct === "number") o.verificar_pct = d.verificar_pct;
  if (d.bloqueo) o.bloqueo = true;
  else if (typeof d.retencion_dias === "number" && d.retencion_dias > 0) o.retencion_dias = d.retencion_dias;
  return o;
}

/** §3d: «Comprueba el 5 % cada día · 120 archivos bien» (o null si no comprueba nada). */
export function textoVerificacion(d: Pick<DestinoEspejoResumen, "verificar_pct" | "verificacion">): string | null {
  const pct = d.verificar_pct ?? 0;
  if (!pct) return null;
  const base = pct === 100 ? "Lo comprueba todo cada día" : `Comprueba el ${pct} % cada día`;
  const v = d.verificacion;
  if (!v) return base;
  return `${base} · ${v.archivos} ${v.archivos === 1 ? "archivo" : "archivos"} la última vez${v.mal ? `, ${v.mal} mal` : ", bien"}`;
}

/** Los repositorios que guarda un almacén, con el nombre que usa el espejo: `<usuario>` o `<usuario>/<repo>`. */
export function nombresRepos(repositorios: { usuario: string; repos: string[] }[] | null | undefined): string[] {
  const v = (repositorios ?? []).flatMap((u) => u.repos.map((r) => (r === "." || r === "" ? u.usuario : `${u.usuario}/${r}`)));
  return [...new Set(v.filter((n) => n.split("/").length <= 2))].sort();
}

/** Repositorios nuevos del almacén que no entran en un destino con selección (ni se vieron al elegirla). */
export function nuevosEn(d: Pick<DestinoEspejoResumen, "repos" | "vistos">, todos: string[]): string[] {
  if (!Array.isArray(d.repos)) return [];
  const conocidos = new Set([...(d.vistos ?? []), ...d.repos]);
  return todos.filter((r) => !conocidos.has(r));
}

/** «Todos los repositorios» o «2 repositorios: a, b». */
export function textoRepos(d: Pick<DestinoEspejoResumen, "repos">, nombre: (r: string) => string = (r) => r): string {
  if (!Array.isArray(d.repos)) return "Todos los repositorios";
  const n = d.repos.map(nombre);
  return n.length === 1 ? `Solo ${n[0]}` : `${n.length} repositorios: ${n.join(", ")}`;
}

/** El nombre de un repositorio en el espejo de su almacén (`<usuario>/<repo>`), o null. */
export function nombreEnAlmacen(donde: string | null | undefined, repo: { id: string; ruta?: string | null }): string | null {
  const u = usuarioEnAlmacen(donde);
  if (!u) return null;
  const r = repo.ruta || repo.id;
  return r === "." || !r ? u : `${u}/${r}`;
}

/** El espejo de un almacén visto desde uno de sus repositorios: solo los destinos a los que va (o null). */
export function espejoDelRepo<E extends { destinos?: Pick<DestinoEspejoResumen, "repos">[] | null }>(espejo: E | null, nombre: string | null): E | null {
  if (!espejo) return null;
  if (!espejo.destinos?.length) return espejo;
  const destinos = espejo.destinos.filter((d) => !Array.isArray(d.repos) || (!!nombre && d.repos.includes(nombre)));
  return destinos.length ? { ...espejo, destinos } : null;
}

/** El destino con estos repositorios añadidos a su selección (y lo de ahora como visto). */
export function conRepos(d: DestinoEspejoOrden, anadir: string[], todos: string[]): DestinoEspejoOrden {
  if (!d.repos) return d;
  return { ...d, repos: [...new Set([...d.repos, ...anadir])], vistos: [...todos] };
}

/** El horario «cada día a esa hora» de antes, como horario de las copias. */
export const horarioDiario = (hora: string): Horario => ({ dias: [1, 2, 3, 4, 5, 6, 7], horas: [hora] });

/** «Cada día a las 02:00», «Cada hora de 8:00 a 18:00, … y después de cada copia nueva». */
export function cuandoEspejo(d: Pick<DestinoEspejoResumen, "horario" | "tras_copia">, horaGlobal: string): string {
  const h = d.horario && (d.horario.reglas?.length || d.horario.horas?.length) ? horarioEnFrase(d.horario) : `Cada día a las ${horaGlobal}`;
  return d.tras_copia ? `${h} y después de cada copia nueva` : h;
}

const tieneHorario = (d: Pick<DestinoEspejoResumen, "horario">) => !!(d.horario && (d.horario.reglas?.length || d.horario.horas?.length));

/** Corto, para el mapa y las flechas: «cada noche a las 02:00» (como antes) o «con su horario y tras cada copia». */
export function cuandoCorto(d: Pick<DestinoEspejoResumen, "horario" | "tras_copia">, horaGlobal: string): string {
  const base = tieneHorario(d) ? "con su horario" : `cada noche a las ${horaGlobal}`;
  return d.tras_copia ? `${base} y tras cada copia` : base;
}

/** Lo mismo para todo el espejo: el de sus destinos si todos coinciden; si no, «con el horario de cada destino». */
export function cuandoCortoEspejo(e: { hora: string; destinos?: Pick<DestinoEspejoResumen, "horario" | "tras_copia">[] | null }): string {
  const textos = [...new Set((e.destinos?.length ? e.destinos : [{}]).map((d) => cuandoCorto(d, e.hora)))];
  return textos.length === 1 ? textos[0] : "con el horario de cada destino";
}

/** La hora que se manda en `espejo.hora` (para consolas anteriores): la primera hora del primer destino. */
export function horaParaConsolasAnteriores(destinos: DestinoEspejoOrden[], porDefecto: string): string {
  for (const d of destinos) {
    const h = d.horario?.horas?.[0] ?? d.horario?.reglas?.map((r) => ("horas" in r ? r.horas[0] : "desde" in r ? r.desde : r.hora)).find(Boolean);
    if (h && /^([01]\d|2[0-3]):[0-5]\d$/.test(h)) return h;
  }
  return porDefecto;
}

/** «1 de noviembre de 2026» para un día AAAA-MM-DD (a mediodía: sin saltos de zona horaria). */
export const diaLegible = (d: string) => new Intl.DateTimeFormat("es", { day: "numeric", month: "long", year: "numeric" }).format(new Date(`${d}T12:00:00`));
