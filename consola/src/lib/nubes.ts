// Conectar una nube (Dropbox) desde la consola, sin CLI ni secreto de app.
//
// OAuth 2 «authorization code» con PKCE y sin redirect_uri: Dropbox muestra
// un código que la persona pega aquí; el navegador lo cambia por el token
// (el endpoint admite CORS) y lo manda SELLADO al equipo que guarda copias
// en la orden `conectar_nube` (clave de administración). El servidor nunca
// ve el token. La app «Resguardo» de Dropbox usa el permiso «App folder»:
// solo puede leer y escribir en Aplicaciones/Resguardo.
import { sha256 } from "@noble/hashes/sha2.js";
import { aB64, aleatorio, utf8 } from "./cripto/bytes";

/**
 * App key pública de la app «Resguardo» de Dropbox (no es un secreto: con
 * PKCE no hay app secret). Si el servidor da `dropbox_app_key` en
 * `GET /api/servidor`, manda la suya (vacía = sin configurar; ver README, «Nubes»).
 */
export const DROPBOX_APP_KEY_POR_DEFECTO = "beobf3c13cvlrup";
export const appKeyConfigurada = (k: string | null | undefined) => !!k?.trim();

const b64url = (b: Uint8Array) => aB64(b).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");

/** code_verifier (43–128 caracteres) y su code_challenge S256. */
export function pkce(): { verifier: string; challenge: string } {
  const verifier = b64url(aleatorio(48)); // 64 caracteres
  return { verifier, challenge: b64url(sha256(utf8(verifier))) };
}

export function urlAutorizar(appKey: string, challenge: string): string {
  const q = new URLSearchParams({
    client_id: appKey,
    response_type: "code",
    code_challenge: challenge,
    code_challenge_method: "S256",
    token_access_type: "offline",
  });
  return `https://www.dropbox.com/oauth2/authorize?${q}`;
}

export interface TokenDropbox {
  refresh_token: string;
  access_token?: string;
  /** RFC 3339, cuándo caduca el access_token. */
  expira?: string;
}

/** Errores de Dropbox, en castellano llano. */
function mensaje(error: string | undefined, descripcion: string | undefined): string {
  const e = `${error ?? ""} ${descripcion ?? ""}`;
  if (/invalid_client/i.test(e)) return "La app «Resguardo» de Dropbox no está configurada en este servidor. Avisa a quien lo administra.";
  if (/invalid_grant/i.test(e) || /code/i.test(descripcion ?? ""))
    return "Ese código no vale: puede que esté mal copiado, que ya se usara o que haya caducado (duran unos minutos). Vuelve a abrir Dropbox y copia el código nuevo.";
  return "Dropbox no aceptó el código. Vuelve a abrir Dropbox y prueba con un código nuevo.";
}

/** Cambia el código por el token, desde el navegador (POST simple, sin secreto). */
export async function cambiarCodigo(appKey: string, codigo: string, verifier: string, signal?: AbortSignal): Promise<TokenDropbox> {
  let res: Response;
  try {
    res = await fetch("https://api.dropboxapi.com/oauth2/token", {
      method: "POST",
      headers: { "Content-Type": "application/x-www-form-urlencoded" },
      body: new URLSearchParams({ grant_type: "authorization_code", code: codigo.trim(), client_id: appKey, code_verifier: verifier }),
      signal,
      credentials: "omit",
      referrerPolicy: "no-referrer",
    });
  } catch {
    throw new Error("No se pudo hablar con Dropbox desde este navegador. Comprueba la conexión a internet y vuelve a intentarlo.");
  }
  let d: { access_token?: string; refresh_token?: string; expires_in?: number; error?: string; error_description?: string } = {};
  try {
    d = await res.json();
  } catch {
    /* sin cuerpo */
  }
  if (!res.ok || !d.refresh_token) throw new Error(mensaje(d.error, d.error_description));
  return {
    refresh_token: d.refresh_token,
    ...(d.access_token ? { access_token: d.access_token } : {}),
    ...(d.expires_in ? { expira: new Date(Date.now() + d.expires_in * 1000).toISOString() } : {}),
  };
}

/** Nombre válido para una nube del equipo (lo usa el espejo para referirse a ella). */
export const nombreNubeValido = (n: string) => /^[\p{L}\p{N} _.-]{1,40}$/u.test(n.trim());
