// «Instalar muchos equipos» (bloque 7 de la 0.7.26; docs/api-servidor.md §4, «Código para
// varios equipos», y docs/guia-instalacion.md, «Instalar muchos equipos»).
//
// Un código que vale para N equipos (lo genera este navegador; el servidor solo guarda su hash),
// las líneas para pegar en cada equipo (PowerShell en Windows, la de siempre en Linux) y la lista
// de los que esperan confirmación. Nada se da de alta sin que una persona compare el número de
// comprobación de cada equipo y lo marque.
//
// Sin dependencias de Svelte ni de la red (scripts/vectores-despliegue.ts lo prueba en Node).
import { codigoValido, huellaValida, lineaVincular, servidorValido } from "./emparejar";
import { sasV2, sasV3 } from "./cripto/claves";

/** Equipos y días por defecto, y lo más que admite el servidor (`api::lotes`). */
export const USOS_POR_DEFECTO = 10;
export const DIAS_POR_DEFECTO = 7;
export const MAX_USOS = 100;
export const MAX_DIAS = 30;

/** Lo que da el servidor de un código para varios equipos (`GET …/codigos-varios`). */
export interface Lote {
  id: string;
  codigo_hash: string;
  nombre: string | null;
  usos: number;
  usados: number;
  quedan: number;
  caduca: string;
  creado: string;
  estado: "activo" | "agotado" | "caducado" | "anulado";
  anulado: string | null;
  rechazos: number;
  pendientes: number;
}

/** Un equipo que se unió con el código (`GET …/codigos-varios/{l}`, `equipos`). */
export interface EquipoDelLote {
  /** Id del emparejamiento (para confirmar o rechazar). */
  id: string;
  estado: "unido" | "confirmado" | "dado_de_alta" | "cancelado" | "caducado";
  caduca: string;
  unido: string;
  /** La IP desde la que se unió, tal como la vio el servidor. */
  ip: string | null;
  equipo?: { id: string; nombre: string; so: string; version_agente?: string; box_pub: string; sign_pub: string; sal_equipo: string };
  /** El número que calcula el servidor (la consola calcula el suyo y no sigue si no coincide). */
  sas?: string;
  sas_version?: number;
}

export type LoteDetalle = Lote & { equipos: EquipoDelLote[] };

/** Cuántos equipos y días se piden: enteros dentro de los límites, o el motivo. */
export function errorUsosDias(usos: number, dias: number): string | null {
  if (!Number.isInteger(usos) || usos < 1 || usos > MAX_USOS) return `Entre 1 y ${MAX_USOS} equipos.`;
  if (!Number.isInteger(dias) || dias < 1 || dias > MAX_DIAS) return `Entre 1 y ${MAX_DIAS} días.`;
  return null;
}

/** El nombre opcional del código: como el servidor (sin comillas ni saltos, hasta 60). */
export function errorNombreLote(n: string): string | null {
  const t = n.trim();
  if (!t) return null;
  if ([...t].length > 60) return "Hasta 60 caracteres.";
  if (/["\u0000-\u001f\u007f]/.test(t)) return "Sin comillas ni saltos de línea.";
  return null;
}

// --- Las líneas para pegar ----------------------------------------------------
// Cada parte se comprueba antes de componer nada (como `lineaVincular`): un servidor malicioso
// no puede colar en la línea comillas, `;`, `$(…)` ni saltos. Además, todo va entre comillas
// simples de PowerShell (donde `$` no se expande) con las comillas dobladas.

const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
const SHA256 = /^[0-9a-f]{64}$/i;

/** Un texto entre comillas simples de PowerShell (literal: `$` y `` ` `` no se interpretan). */
export const comillasPs = (s: string) => `'${s.replace(/'/g, "''")}'`;

export interface DatosLinea {
  /** La dirección con la que los equipos llegan a este servidor (`https://…`). */
  servidor: string;
  /** Id del lote (la descarga del instalador solo sirve mientras el lote está activo). */
  lote: string;
  /** SHA-256 del instalador que sirve este servidor (`GET …/instalador-agente/huella`). */
  sha256: string;
  codigo: string;
}

/**
 * La línea de PowerShell (como administrador) para un equipo Windows, o `""` si alguna parte no
 * tiene su forma. Baja el instalador de ESTE servidor (sin comprobar el certificado: puede ser de
 * su autoridad propia), comprueba su SHA-256 (si no coincide, lo borra y no instala nada), lo
 * instala en silencio y lo vincula con el código (`/S /CODE=… /SERVIDOR=…`), y enseña el número
 * de comprobación que guarda el instalador. El servidor del equipo queda fijado al vincular y lo
 * autentica el número de comprobación (SAS v3, con la huella de la autoridad TLS).
 */
export function lineaPowerShell(d: DatosLinea): string {
  const servidor = d.servidor.trim().replace(/\/+$/, "");
  if (!servidorValido(servidor) || !servidor.startsWith("https://")) return "";
  if (!UUID.test(d.lote) || !SHA256.test(d.sha256) || !codigoValido(d.codigo)) return "";
  const url = `${servidor}/api/agente/instalador/${d.lote.toLowerCase()}`;
  const args = `/S /CODE=${d.codigo} /SERVIDOR=${servidor}`;
  return [
    "$ErrorActionPreference='Stop'",
    // TLS 1.2 además de lo que ya tuviera (Windows PowerShell 5.1 no siempre lo ofrece).
    "[Net.ServicePointManager]::SecurityProtocol=[Net.ServicePointManager]::SecurityProtocol -bor 3072",
    "$f=Join-Path $env:TEMP 'Resguardo-Agente-setup.exe'",
    // Sin comprobar el certificado solo durante la descarga (la huella SHA-256 manda) y de vuelta
    // a lo normal aunque falle: la sesión de PowerShell no se queda sin comprobar certificados.
    `try{[Net.ServicePointManager]::ServerCertificateValidationCallback={$true};(New-Object Net.WebClient).DownloadFile(${comillasPs(url)},$f)}finally{[Net.ServicePointManager]::ServerCertificateValidationCallback=$null}`,
    `if((Get-FileHash $f -Algorithm SHA256).Hash -ne ${comillasPs(d.sha256.toUpperCase())}){Remove-Item $f;throw 'El instalador descargado no es el esperado: no se ha instalado nada.'}`,
    `$p=Start-Process $f -ArgumentList ${comillasPs(args)} -Wait -PassThru`,
    "Remove-Item $f",
    "if($p.ExitCode -eq 0){Get-Content (Join-Path $env:ProgramFiles 'Resguardo Agente\\emparejamiento.txt')}else{Write-Host ('No se pudo vincular (salida '+$p.ExitCode+'). Mira el registro del agente.')}",
  ].join(";");
}

/**
 * La misma línea para una herramienta de despliegue: `powershell.exe -NoProfile
 * -ExecutionPolicy Bypass -EncodedCommand <UTF-16LE en base64>`, sin comillas que escapar.
 * `""` si la línea no se pudo componer.
 */
export function comandoCodificado(linea: string): string {
  if (!linea) return "";
  const b = new Uint8Array(linea.length * 2);
  for (let i = 0; i < linea.length; i++) {
    const c = linea.charCodeAt(i);
    b[2 * i] = c & 0xff;
    b[2 * i + 1] = c >> 8;
  }
  let s = "";
  for (const x of b) s += String.fromCharCode(x);
  return `powershell.exe -NoProfile -ExecutionPolicy Bypass -EncodedCommand ${btoa(s)}`;
}

/**
 * La línea de Linux (agente ya instalado, como root), con la huella de la autoridad TLS de este
 * servidor: la de siempre, empezando por un espacio (bash, con `HISTCONTROL=ignorespace` o
 * `ignoreboth`, lo habitual en Debian y Ubuntu, no la guarda en el historial). `""` si no vale.
 */
export function lineaLinuxVarios(codigo: string, servidor: string, huellaCa: string): string {
  const s = servidor.trim().replace(/\/+$/, "");
  if (!huellaValida(huellaCa)) return "";
  const l = lineaVincular(codigo, s, huellaCa);
  return l ? ` ${l}` : "";
}

// --- Los que esperan confirmación ----------------------------------------------

/** Lo que la consola enseña de cada equipo: el número que calcula ELLA y si coincide con el del servidor. */
export interface Revision {
  e: EquipoDelLote;
  /** El número calculado aquí (v3 con la huella de la autoridad TLS; v2 en un agente anterior). */
  sas: string | null;
  /** ¿Coincide con el del servidor? Si no, no se deja confirmar (puede haber alguien en medio). */
  coincide: boolean;
  /** Agente anterior a 0.7.10: hay que comprobar también la huella de la autoridad TLS. */
  antiguo: boolean;
}

export function revisar(e: EquipoDelLote, identidad: string, huellaCa: string): Revision {
  const q = e.equipo;
  const antiguo = (e.sas_version ?? 2) < 3;
  const sas = q ? (antiguo ? sasV2(identidad, q.box_pub, q.sign_pub) : sasV3(identidad, q.box_pub, q.sign_pub, huellaCa)) : null;
  return { e, sas, coincide: !!sas && sas === e.sas, antiguo };
}

/** Los que esperan: unidos con su equipo, o confirmados a los que les falta el alta. */
export const esperando = (l: EquipoDelLote[]) => l.filter((e) => !!e.equipo && (e.estado === "unido" || e.estado === "confirmado"));

/**
 * Los que se pueden confirmar en bloque: los MARCADOS por la persona, que siguen esperando y
 * cuyo número coincide. Nunca se marca ninguno solo (la página empieza sin ninguno marcado).
 */
export function aConfirmar(revisiones: Revision[], marcados: ReadonlySet<string>): Revision[] {
  return revisiones.filter((r) => marcados.has(r.e.id) && r.coincide && (r.e.estado === "unido" || r.e.estado === "confirmado"));
}

/** «Quedan 7 de 10 · caduca en 6 d» y similares, para la cabecera del código. */
export function resumenLote(l: Pick<Lote, "estado" | "quedan" | "usos">): string {
  switch (l.estado) {
    case "activo":
      return `Quedan ${l.quedan} de ${l.usos} equipos`;
    case "agotado":
      return `Usado por ${l.usos} equipos (ya no admite más)`;
    case "caducado":
      return "Caducado (ya no admite equipos)";
    case "anulado":
      return "Anulado (ya no admite equipos)";
  }
}
