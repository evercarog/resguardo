// 0.8.1 (H3, plan-mejoras 12c): llaves de acceso (passkeys) como segundo factor para
// entrar en la consola, en vez del código TOTP (docs/passkeys.md). Con la API del
// navegador (`navigator.credentials`), sin bibliotecas. Lo que se comprueba (reto,
// origen, firma, contador) lo hace el servidor; aquí solo se pasan los datos de un
// lado a otro en base64url.

/** Base64url sin relleno → bytes. */
export function deB64url(t: string): Uint8Array<ArrayBuffer> {
  const b = atob(t.replace(/-/g, "+").replace(/_/g, "/").padEnd(Math.ceil(t.length / 4) * 4, "="));
  const out = new Uint8Array(b.length);
  for (let i = 0; i < b.length; i++) out[i] = b.charCodeAt(i);
  return out;
}

/** Bytes → base64url sin relleno. */
export function aB64url(b: ArrayBuffer | Uint8Array): string {
  const u = b instanceof Uint8Array ? b : new Uint8Array(b);
  let s = "";
  for (const x of u) s += String.fromCharCode(x);
  return btoa(s).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
}

/** ¿Es una IP (v4 o v6) el nombre con el que se entra? Con una IP no hay llaves de acceso. */
export const esIp = (host: string) => /^\d{1,3}(\.\d{1,3}){3}$/.test(host) || host.includes(":") || host.startsWith("[");

/** Si este navegador y esta dirección admiten llaves de acceso, y si no, por qué (en palabras). */
export function soporte(o: { hostname: string; seguro: boolean; api: boolean }): { ok: true } | { ok: false; motivo: string } {
  if (!o.api) return { ok: false, motivo: "Este navegador no admite llaves de acceso. Usa uno actualizado (Edge, Chrome, Firefox o Safari)." };
  if (esIp(o.hostname)) return { ok: false, motivo: "Entras por una IP: las llaves de acceso solo funcionan entrando por el nombre de la consola (por ejemplo, https://consola.oficina.lan:8443)." };
  if (!o.seguro) return { ok: false, motivo: "La conexión no es segura (o el certificado no es de confianza en este equipo): el navegador no deja usar llaves de acceso aquí." };
  return { ok: true };
}

/** Lo de este navegador. */
export const soporteAqui = () =>
  soporte({
    hostname: typeof location === "undefined" ? "" : location.hostname,
    seguro: typeof window !== "undefined" && window.isSecureContext,
    api: typeof window !== "undefined" && typeof window.PublicKeyCredential === "function" && !!navigator.credentials,
  });

/** El error del navegador en palabras. */
export function errorNavegador(e: unknown): string {
  const n = (e as { name?: string })?.name;
  if (n === "NotAllowedError") return "Se canceló o pasó el tiempo. Vuelve a intentarlo.";
  if (n === "InvalidStateError") return "Esa llave ya está dada de alta en tu cuenta.";
  if (n === "SecurityError") return "El navegador no deja usar llaves de acceso aquí: entra por el nombre de la consola y con un certificado de confianza.";
  if (n === "NotSupportedError") return "Esta llave no admite ninguno de los métodos que acepta la consola.";
  if (n === "AbortError") return "Se canceló.";
  return (e as Error)?.message || "No se pudo usar la llave de acceso.";
}

interface OpcionesAlta {
  challenge: string;
  rp: { id: string; name: string };
  user: { id: string; name: string; displayName: string };
  pubKeyCredParams: { type: "public-key"; alg: number }[];
  excludeCredentials: { type: "public-key"; id: string }[];
  authenticatorSelection: AuthenticatorSelectionCriteria;
  attestation: AttestationConveyancePreference;
  timeout: number;
}

/** `navigator.credentials.create` con las opciones del servidor; lo que hay que mandarle de vuelta. */
export async function crear(o: OpcionesAlta): Promise<{ id: string; client_data: string; attestation: string }> {
  const c = (await navigator.credentials.create({
    publicKey: {
      challenge: deB64url(o.challenge),
      rp: o.rp,
      user: { id: deB64url(o.user.id), name: o.user.name, displayName: o.user.displayName },
      pubKeyCredParams: o.pubKeyCredParams,
      excludeCredentials: o.excludeCredentials.map((x) => ({ type: x.type, id: deB64url(x.id) })),
      authenticatorSelection: o.authenticatorSelection,
      attestation: o.attestation,
      timeout: o.timeout,
    },
  })) as PublicKeyCredential | null;
  if (!c) throw new Error("No se creó ninguna llave.");
  const r = c.response as AuthenticatorAttestationResponse;
  return { id: aB64url(c.rawId), client_data: aB64url(r.clientDataJSON), attestation: aB64url(r.attestationObject) };
}

interface OpcionesEntrar {
  challenge: string;
  rpId: string;
  allowCredentials: { type: "public-key"; id: string }[];
  userVerification: UserVerificationRequirement;
  timeout: number;
}

/** `navigator.credentials.get` con las opciones del servidor; lo que hay que mandarle de vuelta. */
export async function firmar(o: OpcionesEntrar): Promise<{ id: string; client_data: string; authenticator_data: string; firma: string }> {
  const c = (await navigator.credentials.get({
    publicKey: {
      challenge: deB64url(o.challenge),
      rpId: o.rpId,
      allowCredentials: o.allowCredentials.map((x) => ({ type: x.type, id: deB64url(x.id) })),
      userVerification: o.userVerification,
      timeout: o.timeout,
    },
  })) as PublicKeyCredential | null;
  if (!c) throw new Error("No se usó ninguna llave.");
  const r = c.response as AuthenticatorAssertionResponse;
  return { id: aB64url(c.rawId), client_data: aB64url(r.clientDataJSON), authenticator_data: aB64url(r.authenticatorData), firma: aB64url(r.signature) };
}

export interface PasskeyVista {
  ref: string;
  nombre: string;
  creada: string;
  usada: string | null;
  dominio: string;
}
export type { OpcionesAlta, OpcionesEntrar };
