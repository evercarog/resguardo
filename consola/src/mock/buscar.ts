// El «agente» simulado para «Buscar archivos» (`buscar_todas`, api-servidor.md
// §7): busca el texto en el nombre de los archivos de mentira de
// mock/detalle.ts, en las versiones del rango, con las mismas versiones que da
// `historial_archivo` (así cuadran) y unos cuantos borrados que solo están en
// versiones antiguas. Valida como el agente (sesiones_v2.rs y motor/buscar.rs)
// y da páginas pequeñas para que se vea la carga por páginas.
import { ARCHIVOS, BORRADOS, historialDe, versionesDe } from "./detalle";

/** Desde qué versión del agente se anuncia (en el simulador: los de «pulsar para ver más»). */
export const VERSION_BUSCAR = "0.7.14";
export const OP_BUSCAR = "buscar_todas";
const PAGINA = 8;
const MAX = 2000;
const POR_ARCHIVO = 500;

const instante = (x: unknown) => (typeof x === "string" && x.length <= 40 && /T\d{2}:\d{2}/.test(x) && !Number.isNaN(Date.parse(x)) ? Date.parse(x) : NaN);

/** Responde `buscar_todas` (o su error, como el agente). */
export async function buscarTodas(m: Record<string, unknown>, s: { equipo: string; repo: string | null }, trabajando: () => void): Promise<Record<string, unknown>> {
  const texto = typeof m.texto === "string" ? m.texto.trim() : "";
  const n = [...texto].length;
  if (n < 2 || n > 100) return { error: "Escribe qué buscar (de 2 a 100 caracteres)." };
  if (/[\u0000-\u001f\u007f/\\]/.test(texto)) return { error: "Busca por el nombre del archivo (sin / ni \\ ni caracteres de control)." };
  const desde = m.desde == null || m.desde === "" ? null : instante(m.desde);
  const hasta = m.hasta == null || m.hasta === "" ? null : instante(m.hasta);
  if (Number.isNaN(desde) || Number.isNaN(hasta)) return { error: "Fecha no válida." };
  if (desde != null && hasta != null && desde > hasta) return { error: "La fecha «desde» es posterior a «hasta»." };
  const max = m.max == null ? MAX : typeof m.max === "number" && Number.isInteger(m.max) && m.max >= 1 && m.max <= MAX ? m.max : -1;
  if (max < 0) return { error: `«max» no válido (de 1 a ${MAX}).` };
  const indice = m.indice == null ? 0 : typeof m.indice === "number" && Number.isInteger(m.indice) && m.indice >= 0 ? m.indice : -1;
  if (indice < 0) return { error: "«indice» no válido." };

  const todas = versionesDe(s.equipo, s.repo);
  const enRango = todas.filter((v) => (desde == null || Date.parse(v.hora) >= desde) && (hasta == null || Date.parse(v.hora) <= hasta));
  if (indice === 0) {
    trabajando();
    await new Promise((r) => setTimeout(r, 900));
  }
  const aguja = texto.toLowerCase();
  const ids = new Set(enRango.map((v) => v.id));
  let coincidencias = 0;
  let lleno = false;
  const archivos: { ruta: string; versiones: ReturnType<typeof historialDe>; recortado?: boolean }[] = [];
  for (const ruta of [...ARCHIVOS, ...BORRADOS]) {
    const nombre = ruta.slice(ruta.lastIndexOf("/") + 1);
    if (!nombre.toLowerCase().includes(aguja)) continue;
    let vs = historialDe(ruta, todas).filter((v) => ids.has(v.version));
    if (!vs.length) continue;
    if (coincidencias + vs.length > max) {
      vs = vs.slice(0, max - coincidencias);
      lleno = true;
    }
    coincidencias += vs.length;
    const recortado = vs.length > POR_ARCHIVO;
    if (vs.length) archivos.push({ ruta, versiones: vs.slice(0, POR_ARCHIVO), ...(recortado ? { recortado } : {}) });
    if (lleno) break;
  }
  archivos.sort((a, b) => Date.parse(b.versiones[0].cuando) - Date.parse(a.versiones[0].cuando) || a.ruta.localeCompare(b.ruta));
  const pagina = archivos.slice(indice, indice + PAGINA);
  return {
    texto,
    total_archivos: archivos.length,
    coincidencias,
    versiones_buscadas: enRango.length,
    versiones_en_rango: enRango.length,
    recortado: lleno || archivos.some((a) => a.recortado),
    motivo: lleno ? "limite" : null,
    indice,
    siguiente: indice + PAGINA < archivos.length ? indice + PAGINA : null,
    archivos: pagina,
  };
}
