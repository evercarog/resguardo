// 0.8.1 (H5, H2): un CSV para abrir en Excel o LibreOffice.
//
// Separado por «;» (lo que espera Excel con la configuración regional en español,
// donde la coma es el decimal), con BOM para que reconozca UTF-8 (tildes y ñ), saltos
// de línea CRLF y cada celda entre comillas si hace falta. Una celda que empieza por
// `=`, `+`, `-`, `@`, tabulador o retorno se escribe con un apóstrofo delante: así una
// hoja de cálculo nunca la ejecuta como fórmula (inyección de fórmulas en CSV), aunque
// el texto venga de un nombre que escribió otra persona.

export type Celda = string | number | boolean | null | undefined;

/** Una celda, lista para el CSV. */
export function celdaCsv(x: Celda): string {
  if (x == null) return "";
  let t = typeof x === "boolean" ? (x ? "sí" : "no") : String(x);
  if (typeof x === "string" && /^[=+\-@\t\r]/.test(t)) t = `'${t}`;
  return /[";\r\n]/.test(t) || t !== t.trim() ? `"${t.replace(/"/g, '""')}"` : t;
}

const BOM = String.fromCharCode(0xfeff);

/** El CSV entero (con BOM), de una cabecera y sus filas. */
export function csv(cabecera: string[], filas: Celda[][]): string {
  return `${BOM}${[cabecera, ...filas].map((f) => f.map(celdaCsv).join(";")).join("\r\n")}\r\n`;
}

/** Un nombre de archivo sin caracteres raros: «Resguardo - Oficina Sur - configuración - 2026-10-20.csv». */
export function nombreArchivo(partes: string[], ext: string): string {
  const limpio = partes
    .map((p) => p.replace(/[\\/:*?"<>|\u0000-\u001f]+/g, " ").replace(/\s+/g, " ").trim())
    .filter(Boolean)
    .join(" - ")
    .slice(0, 150);
  return `${limpio || "Resguardo"}.${ext}`;
}
