// Informe ejecutivo simulado (plan 0.8.0, G1): el programa de cada cliente, la
// regla que manda la consola, «Enviar ahora» (no manda nada: lo apunta en el
// registro de envíos simulado) y el prestador del servidor. El PDF es uno de
// ejemplo, pequeño, hecho aquí (el de verdad lo escribe el servidor).
import type * as T from "../lib/tipos";
import { estado } from "./estado";

type Err = (estado: number, codigo: string, mensaje: string) => Error;
type Ctx = { cuerpo: Record<string, unknown> };
type Ayudas<C> = {
  err: Err;
  cuenta: (ctx: C) => { id: string; correo: string; nombre: string; superusuario: boolean };
  miembro: (ctx: C, cliente: string, minimo: T.Rol) => unknown;
  /** Contesta con un archivo (tipo, nombre y bytes). */
  archivo: (ctx: C, tipo: string, nombre: string, datos: Buffer) => unknown;
};

const C = "/api/clientes/([^/]+)";

let prestador: T.PrestadorInforme = { nombre: "Soporte Ejemplo", logo: null, actualizado: null, por: null };
const programas = new Map<string, T.ProgramaInforme>();
const reglas = new Map<string, { calculada: string; por: string; copias: number }>();
const ultimos = new Map<string, NonNullable<T.InformeEjecutivo["ultimo"]>>();

const programa = (c: string): T.ProgramaInforme =>
  programas.get(c) ?? { activo: false, periodo: "semanal", dia_semana: 1, destinatarios: [], logo: "prestador", actualizado: null, por: null };

/** Un PDF de una página con un par de líneas (con sus tablas de referencias bien calculadas). */
function pdfDeEjemplo(lineas: string[]): Buffer {
  const texto = lineas.map((l, i) => `BT /F1 ${i ? 11 : 18} Tf 56 ${780 - i * 24} Td (${l.replace(/[()\\]/g, "\\$&")}) Tj ET`).join("\n");
  const objetos = [
    "<< /Type /Catalog /Pages 2 0 R >>",
    "<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
    "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> >>",
    `<< /Length ${Buffer.byteLength(texto, "latin1")} >>\nstream\n${texto}\nendstream`,
    "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>",
  ];
  let out = "%PDF-1.7\n";
  const pos: number[] = [];
  objetos.forEach((o, i) => {
    pos.push(Buffer.byteLength(out, "latin1"));
    out += `${i + 1} 0 obj\n${o}\nendobj\n`;
  });
  const xref = Buffer.byteLength(out, "latin1");
  out += `xref\n0 ${objetos.length + 1}\n0000000000 65535 f \n${pos.map((p) => `${String(p).padStart(10, "0")} 00000 n \n`).join("")}`;
  out += `trailer\n<< /Size ${objetos.length + 1} /Root 1 0 R >>\nstartxref\n${xref}\n%%EOF\n`;
  return Buffer.from(out, "latin1");
}

export function rutasInforme<C2 extends Ctx>(h: Ayudas<C2>): [string, RegExp, (ctx: C2, m: string[]) => unknown][] {
  const soloServidor = (ctx: C2) => {
    if (!h.cuenta(ctx).superusuario) throw h.err(403, "prohibido", "Solo quien administra el servidor.");
  };
  const vista = (c: string): T.InformeEjecutivo => ({
    programa: programa(c),
    prestador,
    logo_cliente: !!estado.clientes.find((x) => x.id === c)?.marca?.logo,
    regla: reglas.get(c) ?? null,
    ultimo: ultimos.get(c) ?? null,
    correo: true,
    hora: "08:00",
  });
  return [
    ["GET", new RegExp(`^${C}/informe-ejecutivo$`), (ctx, [c]) => (h.miembro(ctx, c, "lectura"), vista(c))],
    [
      "PUT",
      new RegExp(`^${C}/informe-ejecutivo$`),
      (ctx, [c]) => {
        h.miembro(ctx, c, "administrador");
        const b = ctx.cuerpo as Partial<T.ProgramaInforme>;
        const p = { ...programa(c), ...b };
        p.destinatarios = [...new Set((p.destinatarios ?? []).map((d) => d.trim()).filter(Boolean))];
        for (const d of p.destinatarios) if (!/^[^\s@<>,;"]+@[^\s@<>,;"]+\.[^\s@<>,;"]+$/.test(d)) throw h.err(422, "datos", `«${d}» no es una dirección de correo válida.`);
        if (p.destinatarios.length > 20) throw h.err(422, "datos", "Como mucho 20 destinatarios.");
        if (p.activo && !p.destinatarios.length) throw h.err(422, "datos", "Añade al menos un destinatario para programar el informe.");
        p.actualizado = new Date().toISOString();
        p.por = h.cuenta(ctx).nombre;
        programas.set(c, p);
        return p;
      },
    ],
    [
      "PUT",
      new RegExp(`^${C}/informe-ejecutivo/regla$`),
      (ctx, [c]) => {
        h.miembro(ctx, c, "tecnico");
        const copias = (ctx.cuerpo as { copias?: unknown[] }).copias ?? [];
        reglas.set(c, { calculada: new Date().toISOString(), por: h.cuenta(ctx).nombre, copias: copias.length });
        return undefined;
      },
    ],
    [
      "GET",
      new RegExp(`^${C}/informe-ejecutivo/pdf$`),
      (ctx, [c]) => {
        h.miembro(ctx, c, "lectura");
        const nombre = estado.clientes.find((x) => x.id === c)?.nombre ?? "Cliente";
        const pdf = pdfDeEjemplo(["Informe de copias (simulador)", nombre, "El servidor de verdad escribe aqui el informe completo."]);
        return h.archivo(ctx, "application/pdf", "informe-semanal-simulado.pdf", pdf);
      },
    ],
    [
      "POST",
      new RegExp(`^${C}/informe-ejecutivo/enviar$`),
      (ctx, [c]) => {
        h.miembro(ctx, c, "administrador");
        const p = programa(c);
        if (!p.destinatarios.length) throw h.err(422, "datos", "Añade al menos un destinatario.");
        const b = ctx.cuerpo as { periodo?: T.PeriodoInforme };
        const ahora = new Date();
        ultimos.set(c, {
          cuando: ahora.toISOString(),
          periodo: b.periodo ?? p.periodo,
          desde: new Date(ahora.getTime() - 7 * 86_400_000).toISOString(),
          hasta: ahora.toISOString(),
          destinatarios: p.destinatarios.length,
          a_mano: true,
          por: h.cuenta(ctx).nombre,
        });
        return { enviados: p.destinatarios.length };
      },
    ],
    ["GET", /^\/api\/servidor\/informes$/, (ctx) => (soloServidor(ctx), prestador)],
    [
      "PUT",
      /^\/api\/servidor\/informes$/,
      (ctx) => {
        soloServidor(ctx);
        const b = ctx.cuerpo as { nombre?: string; logo?: string; quitar_logo?: boolean };
        if (b.nombre !== undefined && b.nombre.length > 80) throw h.err(422, "datos", "El nombre, como mucho 80 caracteres y sin saltos de línea.");
        prestador = {
          nombre: b.nombre !== undefined ? b.nombre.trim() || null : prestador.nombre,
          // En el simulador, el PNG va como data: (el servidor de verdad lo sirve aparte).
          logo: b.quitar_logo ? null : b.logo ? `data:image/png;base64,${b.logo}` : prestador.logo,
          actualizado: new Date().toISOString(),
          por: h.cuenta(ctx).nombre,
        };
        return prestador;
      },
    ],
  ];
}
