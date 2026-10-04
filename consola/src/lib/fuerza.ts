// Cómo de fuerte es una clave que se elige aquí (la de respaldo de la consola).
// Una estimación sencilla y prudente: largo, variedad de caracteres, palabras
// sueltas y repeticiones. No pretende ser exacta: solo avisar de lo flojo.

export interface Fuerza {
  /** 0 (muy débil) a 4 (muy fuerte). */
  nivel: 0 | 1 | 2 | 3 | 4;
  texto: string;
  consejo: string | null;
}

const TEXTOS = ["Muy débil", "Débil", "Aceptable", "Fuerte", "Muy fuerte"] as const;
const COMUNES = ["contraseña", "contrasena", "password", "resguardo", "123456", "qwerty", "admin", "clave", "copia", "servidor"];

export function fuerza(clave: string): Fuerza {
  const c = clave.normalize("NFC");
  const largo = [...c].length;
  const clases = [/[a-zñáéíóúü]/, /[A-ZÑÁÉÍÓÚÜ]/, /[0-9]/, /[^a-zA-Z0-9ñÑáéíóúüÁÉÍÓÚÜ\s]/, /\s/].filter((r) => r.test(c)).length;
  const palabras = c.split(/\s+/).filter((p) => p.length >= 3).length;
  const distintos = new Set(c.toLowerCase()).size;
  // Bits aproximados: por carácter según la variedad; una frase de varias palabras cuenta por palabras.
  let bits = largo * Math.log2(Math.min(95, [0, 26, 52, 62, 80, 95][clases] || 26));
  if (palabras >= 4) bits = Math.max(bits * 0.6, palabras * 11);
  if (distintos < largo / 3) bits *= 0.5;
  if (COMUNES.some((p) => c.toLowerCase().includes(p))) bits -= 20;
  const nivel = (largo < 12 ? Math.min(1, bits >= 40 ? 1 : 0) : bits >= 110 ? 4 : bits >= 80 ? 3 : bits >= 60 ? 2 : bits >= 40 ? 1 : 0) as Fuerza["nivel"];
  const consejo =
    largo < 12
      ? "Al menos 12 caracteres."
      : nivel < 2
        ? "Mejor una frase de cuatro o cinco palabras que no vayan juntas."
        : nivel < 3
          ? "Vale; con una palabra más estaría mejor."
          : null;
  return { nivel, texto: TEXTOS[nivel], consejo };
}
