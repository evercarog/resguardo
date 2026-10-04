// Lo común del editor del equipo en modo local (sin consola).
import type { CopiaConfig, Regla, VerificacionAuto } from "$lib/tipos";
import type { Escritorio } from "../../tipos";

export interface RepoLocal {
  id: string;
  nombre: string;
  destino: string;
  retencion?: Regla | null;
  externa?: { destino: string; hora: string } | null;
}
export interface DestinoLocal {
  id: string;
  nombre: string;
  tipo: "local" | "rest" | "s3" | "b2" | "sftp";
  donde: string;
}
export interface ConfigLocal {
  v: 1;
  copias: CopiaConfig[];
  verificaciones?: Record<string, VerificacionAuto>;
  escritorio?: Escritorio;
  bandeja?: unknown;
  verificacion?: unknown;
  cambiado_en_equipo?: string;
}
export interface EstadoLocal {
  repositorios: RepoLocal[];
  destinos: DestinoLocal[];
  config: ConfigLocal;
  nubes: { nombre: string; tipo: string }[];
  resumen: { guarda_copias?: GuardaCopias | null } & Record<string, unknown>;
  pausado_hasta: string | null;
  nombre_equipo: string;
}
/** Lo que recibe cada parte del editor local. */
export interface PropsParte {
  estado: EstadoLocal;
  recargar: () => Promise<void>;
  alBloquear: () => void;
}

export interface GuardaCopias {
  activo: boolean;
  puerto: number;
  carpeta: string;
  usuarios: number;
  espejo?: { destinos?: { tipo: string; carpeta?: string; nube?: string }[]; hora?: string } | null;
}

/** Un id para el agente (letras, números, - y _): del nombre y unas letras al azar. */
export function idDe(nombre: string): string {
  const base = nombre
    .normalize("NFD")
    .replace(/[̀-ͯ]/g, "")
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "")
    .slice(0, 24);
  const azar = Array.from(crypto.getRandomValues(new Uint8Array(3)), (b) => b.toString(16).padStart(2, "0")).join("");
  return `${base || "id"}-${azar}`;
}

/** Contraseña de un repositorio nuevo: 24 caracteres al azar (unos 139 bits), en grupos de 4 para copiarla a mano sin errores (sin 0/O ni 1/l/I). */
export function contrasenaNueva(): string {
  const letras = "abcdefghijkmnpqrstuvwxyzABCDEFGHJKLMNPQRSTUVWXYZ23456789";
  let t = "";
  // Sin sesgo: solo los bytes por debajo de 224 (4 × 56).
  while (t.length < 24) for (const b of crypto.getRandomValues(new Uint8Array(32))) if (b < 224 && t.length < 24) t += letras[b % letras.length];
  return t.slice(0, 24).match(/.{4}/g)!.join("-");
}

/** La configuración que se manda: como la consola (horas ordenadas, ganchos limpios). */
export function configParaEnviar(c: ConfigLocal): ConfigLocal {
  const copia = JSON.parse(JSON.stringify(c)) as ConfigLocal;
  delete copia.cambiado_en_equipo;
  return copia;
}

export const TIPOS_DESTINO: Record<DestinoLocal["tipo"], string> = {
  local: "Carpeta o disco de este equipo",
  rest: "Servidor de copias (rest-server)",
  s3: "S3 (Amazon, Wasabi, MinIO…)",
  b2: "Backblaze B2",
  sftp: "SFTP",
};
