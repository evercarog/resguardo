// Preferencias pequeñas que se recuerdan en este navegador (filtros de las
// listas). Nunca secretos ni datos de los equipos: solo cómo se quería ver
// la pantalla. Sin almacenamiento (ventana privada, bloqueado), no pasa nada.
const PREFIJO = "resguardo.";

export function leer<T extends string>(clave: string, valores: readonly T[], porDefecto: T): T {
  try {
    const v = localStorage.getItem(PREFIJO + clave);
    return v !== null && (valores as readonly string[]).includes(v) ? (v as T) : porDefecto;
  } catch {
    return porDefecto;
  }
}

export function leerTexto(clave: string): string {
  try {
    return localStorage.getItem(PREFIJO + clave) ?? "";
  } catch {
    return "";
  }
}

export function guardar(clave: string, valor: string) {
  try {
    if (valor) localStorage.setItem(PREFIJO + clave, valor);
    else localStorage.removeItem(PREFIJO + clave);
  } catch {
    /* sin almacenamiento */
  }
}
