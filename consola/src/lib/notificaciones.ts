// Notificaciones (api-servidor.md §13): textos y reglas que comparten la
// sección del servidor, la de cada cliente, Personas y «Mis notificaciones».
import type { CambioCanal, CanalNotif, ConfigCanal, EnvioNotif, ReglasCanal, Severidad, TipoCanal } from "./tipos";
import type { Tono } from "./salud";

export const SEVERIDADES: Severidad[] = ["critico", "importante", "informativo"];

export const SEVERIDAD: Record<Severidad, { texto: string; tono: Tono; que: string }> = {
  critico: { texto: "Crítico", tono: "bad", que: "Copias o verificaciones que fallan, intentos con la clave, bloqueos, órdenes destructivas, cambio de clave." },
  importante: { texto: "Importante", tono: "warn", que: "Equipos sin contacto, copias atrasadas, copia externa, espejo o prueba de restauración que fallan." },
  informativo: { texto: "Informativo", tono: "info", que: "Lo demás, y cuando algo vuelve a funcionar." },
};

export const TIPO_CANAL: Record<TipoCanal, { texto: string; que: string }> = {
  correo: { texto: "Correo", que: "Por SMTP, a cada persona según sus preferencias (Personas)." },
  webhook: { texto: "Webhook", que: "Un POST con JSON (firmado si pones un secreto) a la dirección que digas: Slack, Teams, tu sistema de tickets…" },
  ntfy: { texto: "ntfy", que: "Notificaciones en el móvil con la app ntfy (ntfy.sh o tu propio servidor)." },
  telegram: { texto: "Telegram", que: "Mensajes de un bot de Telegram a un chat o grupo." },
};

/** Los secretos de cada tipo, con su etiqueta y si hacen falta. */
export const SECRETOS: Record<TipoCanal, { campo: string; etiqueta: string; requerido: boolean; ayuda: string }[]> = {
  correo: [{ campo: "contrasena", etiqueta: "Contraseña", requerido: false, ayuda: "La del usuario del servidor de correo (o una contraseña de aplicación)." }],
  webhook: [
    { campo: "url", etiqueta: "Dirección", requerido: true, ayuda: "Empieza por https://. Se guarda cifrada: solo se enseña su servidor." },
    { campo: "secreto", etiqueta: "Secreto para firmar", requerido: false, ayuda: "Opcional (16 caracteres o más): cada envío lleva la cabecera X-Resguardo-Firma." },
  ],
  ntfy: [
    { campo: "url", etiqueta: "Dirección del tema", requerido: true, ayuda: "Por ejemplo https://ntfy.sh/copias-empresa-x7k2 (un tema difícil de adivinar)." },
    { campo: "token", etiqueta: "Token", requerido: false, ayuda: "Opcional, si el tema está protegido." },
  ],
  telegram: [{ campo: "token", etiqueta: "Token del bot", requerido: true, ayuda: "Te lo da @BotFather al crear el bot (123456789:AA…)." }],
};

export const reglasPorDefecto = (): ReglasCanal => ({ severidades: ["critico", "importante"], clientes: null, silencio: null, resumen_diario: false, resumen_semanal: false });

/** ¿Cambia a dónde va el canal o algún secreto? Entonces el servidor pide un código de la aplicación de autenticación. */
export function cambioSensible(previo: CanalNotif | null, c: CambioCanal): boolean {
  if (!previo) return true;
  if (c.secretos && Object.keys(c.secretos).length) return true;
  if (!c.config) return false;
  const norm = (x: ConfigCanal) => JSON.stringify({ ...x, servidor: undefined, host: x.host?.trim().toLowerCase(), usuario: x.usuario?.trim() || undefined, remitente: x.remitente?.trim() });
  return norm(c.config) !== norm(previo.config);
}

/** El estado de un envío, en palabras. */
export function estadoEnvio(e: EnvioNotif): { texto: string; tono: Tono } {
  switch (e.estado) {
    case "enviado":
      return { texto: "Enviado", tono: "ok" };
    case "fallido":
      return { texto: "Falló", tono: "bad" };
    case "descartado":
      return { texto: "No se envió", tono: "neutral" };
    default:
      return { texto: e.intentos ? "Reintentando" : "En cola", tono: e.intentos ? "warn" : "info" };
  }
}

export const TIPO_ENVIO: Record<EnvioNotif["tipo"], string> = { aviso: "Aviso", recuperacion: "Volvió a funcionar", resumen: "Resumen", prueba: "Prueba" };

export const DIAS_SEMANA = ["lunes", "martes", "miércoles", "jueves", "viernes", "sábado", "domingo"];

/** «Crítico e importante», «Nada»… */
export function textoSeveridades(s: Severidad[]): string {
  const xs = SEVERIDADES.filter((x) => s.includes(x)).map((x) => SEVERIDAD[x].texto.toLowerCase());
  if (!xs.length) return "Nada al momento";
  if (xs.length === 3) return "Todo";
  const t = xs.length === 1 ? xs[0] : `${xs.slice(0, -1).join(", ")} e ${xs.at(-1)}`;
  return t.charAt(0).toUpperCase() + t.slice(1);
}
