import { Cloud, Database, Globe, HardDrive, Server } from "@lucide/svelte";

/** Proveedor de un destino compatible con S3 según su servidor. */
function s3Label(location: string) {
  const host = /^s3:(?:https?:\/\/)?([^/]+)/i.exec(location.trim())?.[1]?.toLowerCase() ?? "";
  if (host.endsWith("backblazeb2.com")) return "Backblaze B2";
  if (host.endsWith("wasabisys.com")) return "Wasabi";
  if (host.endsWith("r2.cloudflarestorage.com")) return "Cloudflare R2";
  if (host.endsWith("amazonaws.com") || !host.includes(".")) return "Amazon S3";
  return "Nube S3";
}

/** Tipo de almacenamiento según el prefijo de la ubicación de restic. */
export function repoKind(location: string) {
  const l = location.trim().toLowerCase();
  if (l.startsWith("sftp:")) return { label: "SFTP", icon: Server, cloud: false };
  if (l.startsWith("rest:")) return { label: "Servidor REST", icon: Globe, cloud: false };
  if (l.startsWith("s3:")) return { label: s3Label(location), icon: Cloud, cloud: true };
  if (l.startsWith("b2:")) return { label: "Backblaze B2", icon: Cloud, cloud: true };
  if (l.startsWith("azure:")) return { label: "Azure Blob", icon: Cloud, cloud: true };
  if (l.startsWith("gs:")) return { label: "Google Cloud Storage", icon: Cloud, cloud: true };
  if (l.startsWith("swift:")) return { label: "OpenStack Swift", icon: Cloud, cloud: true };
  if (l.startsWith("rclone:")) return { label: "rclone", icon: Database, cloud: false };
  return { label: "Carpeta o disco", icon: HardDrive, cloud: false };
}
