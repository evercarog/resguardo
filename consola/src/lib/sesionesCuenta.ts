// 16k: las sesiones de la consola de cada cuenta (lista en «Mi cuenta», aviso de una
// sesión nueva y textos de los plazos). Solo lógica, con sus pruebas en
// scripts/vectores-083-servidor.ts.
import type { PlazosSesion, SesionCuenta } from "./tipos";
import { cuentaAtras, fechaLarga, relativo } from "./formato";

/** ¿Es un móvil o tableta? (para el icono). */
export function esMovil(navegador: string): boolean {
  return /Android|iPhone|iPad/.test(navegador);
}

/** La segunda línea de cada sesión: «Activa ahora · entró hace 3 días · desde 192.0.2.10 · recordada». */
export function lineaSesion(s: SesionCuenta, ahora = Date.now()): string {
  const partes: string[] = [];
  partes.push(s.actual ? "Esta sesión" : `Usada ${relativo(s.usada, ahora)}`);
  partes.push(`entró ${relativo(s.creada, ahora)}`);
  if (s.ip && s.ip !== "?") partes.push(`desde ${s.ip}`);
  partes.push(s.recordar ? `recordada hasta el ${fechaLarga(s.maximo)}` : `se cierra si no se usa (${relativo(s.caduca, ahora)})`);
  return partes.join(" · ");
}

/** Ordenadas: la actual primero, luego las más usadas recientemente. */
export function ordenarSesiones(ss: SesionCuenta[]): SesionCuenta[] {
  return [...ss].sort((a, b) => (a.actual === b.actual ? Date.parse(b.usada) - Date.parse(a.usada) : a.actual ? -1 : 1));
}

/** Por qué no se pueden cerrar las demás desde aquí (o null si se puede). */
export function motivoNoCerrar(puede: boolean, desde: string | null, ahora = Date.now()): string | null {
  if (puede) return null;
  const cuando = desde ? ` Podrás dentro de ${cuentaAtras(desde, ahora)}.` : "";
  return `Esta sesión es nueva: por seguridad, durante sus primeras 24 horas no puede cerrar las demás ni cambiar la contraseña, el segundo paso o las llaves de acceso.${cuando} Desde un dispositivo donde ya tenías la sesión abierta sí puedes.`;
}

/** Las sesiones nuevas de las que aún no se avisó en este navegador. */
export function nuevasSinVer<T extends { id: string }>(nuevas: T[] | undefined, vistas: string[]): T[] {
  return (nuevas ?? []).filter((n) => !vistas.includes(n.id));
}

/** El aviso de una sesión nueva: «Nueva sesión en Chrome · Android, hace 5 min. ¿Eras tú?». */
export function textoSesionNueva(n: { navegador: string; creada: string }, ahora = Date.now()): string {
  return `Nueva sesión en ${n.navegador || "un navegador"}, ${relativo(n.creada, ahora)}. ¿Eras tú? Si no, ciérrala en «Mi cuenta» y cambia la contraseña.`;
}

/** Los plazos en una frase, para el formulario del propietario. */
export function frasePlazos(p: PlazosSesion): string {
  const recordar = p.recordar_dias > 0 ? `con «Recordar este dispositivo», hasta ${p.recordar_dias} ${p.recordar_dias === 1 ? "día" : "días"}` : "sin «Recordar este dispositivo»";
  return `Se cierra tras ${p.inactividad_horas} h sin usarla y, aunque se use, a las ${p.maximo_horas} h; ${recordar}.`;
}

/** Los mismos topes que el servidor (inactividad 1–24 h, máximo hasta 7 días, recordar 0–30 días). */
export function conTopes(p: PlazosSesion): PlazosSesion {
  const ent = (n: number, d: number) => (Number.isFinite(n) ? Math.round(n) : d);
  const inactividad_horas = Math.min(24, Math.max(1, ent(p.inactividad_horas, 8)));
  return {
    inactividad_horas,
    maximo_horas: Math.min(168, Math.max(inactividad_horas, ent(p.maximo_horas, 24))),
    recordar_dias: Math.min(30, Math.max(0, ent(p.recordar_dias, 14))),
  };
}
