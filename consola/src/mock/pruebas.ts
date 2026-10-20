// Pruebas funcionales simuladas (plan 0.8.0, E3, 14k): como el servidor (registro,
// política, plazo, adjuntos comprobados por su contenido), en memoria. Con unas de
// ejemplo la primera vez que se piden, para ver la página y las cifras.
import { randomUUID } from "node:crypto";
import type * as T from "../lib/tipos";
import type { EntradaPruebaFuncional, PlazoPrueba, PoliticaPruebas, PruebaFuncional, PruebasFuncionales } from "../lib/tiposPruebas";
import { estado } from "./estado";

type Err = (estado: number, codigo: string, mensaje: string) => Error;
type Ctx = { cuerpo: Record<string, unknown> };
type Ayudas<C> = {
  err: Err;
  miembro: (ctx: C, cliente: string, minimo: T.Rol) => { cuenta: { nombre: string } };
  archivo: (ctx: C, tipo: string, nombre: string, datos: Buffer) => unknown;
};

const C = "/api/clientes/([^/]+)";
const MES = 30 * 86_400_000;
const porCliente = new Map<string, { politica: PoliticaPruebas; pruebas: PruebaFuncional[]; visto: number; datos: Map<string, Buffer> }>();

const dia = (t: number) => new Date(t).toISOString().slice(0, 10);

function de(c: string) {
  let x = porCliente.get(c);
  if (!x) {
    x = { politica: { activo: true, meses: 6, aviso_dias: 30, rto_objetivo_min: 240, rpo_objetivo_min: 1440, repos: [] }, pruebas: [], visto: Date.now(), datos: new Map() };
    porCliente.set(c, x);
    // Unas de ejemplo (deterministas), con el primer repositorio de los primeros equipos.
    const eqs = estado.equipos.filter((e) => e.cliente === c && (e.resumen?.repositorios?.length ?? 0) > 0).slice(0, 2);
    const ahora = Date.now();
    const ej: Partial<PruebaFuncional>[] = [
      { fecha: dia(ahora - 20 * 86_400_000), motivo: "programada", alcance: "cliente", que: "Las carpetas compartidas y la base de datos de ventas", donde: "Máquina virtual de pruebas", quien: "Luis Gómez", resultado: "ok", rto_min: 150, rpo_min: 600, notas: "Sin sorpresas." },
      {
        fecha: dia(ahora - 200 * 86_400_000),
        motivo: "auditoria",
        alcance: "repos",
        repos: eqs.map((e) => ({ equipo: e.id, repo: e.resumen!.repositorios![0].id })),
        que: "Documentos del último trimestre",
        donde: "Otra carpeta del servidor",
        quien: "Ana Pérez",
        resultado: "parcial",
        incidencias: "Faltaba el permiso de escritura en la carpeta restaurada; se arregló a mano.",
        rto_min: 320,
        rpo_min: 1500,
        // 0.8.1 (H4): registrada en otra consola del cliente, con su acta allí.
        origen: { consola: "En línea" },
        adjuntos_fuera: [{ n: 1, nombre: "Acta del simulacro.pdf", tipo: "application/pdf", tamano: 184_320, consola: "En línea" }],
      },
      { fecha: dia(ahora - 390 * 86_400_000), motivo: "cambio", alcance: "cliente", que: "Todo el servidor de archivos", donde: "Equipo de repuesto", quien: "Luis Gómez", resultado: "ok", rto_min: 480, rpo_min: 900 },
    ];
    for (const p of ej) {
      x.pruebas.push({
        id: randomUUID(),
        fecha: p.fecha!,
        motivo: p.motivo!,
        alcance: p.alcance!,
        repos: p.repos ?? [],
        que: p.que ?? "",
        donde: p.donde ?? "",
        quien: p.quien ?? "",
        resultado: p.resultado!,
        incidencias: p.incidencias ?? null,
        rto_min: p.rto_min ?? null,
        rpo_min: p.rpo_min ?? null,
        notas: p.notas ?? null,
        desde: null,
        restauracion: null,
        creada: new Date(Date.parse(`${p.fecha}T15:00:00`)).toISOString(),
        actualizada: new Date(Date.parse(`${p.fecha}T15:00:00`)).toISOString(),
        por: p.origen ? `${p.quien ?? "Ana"} (desde la consola «${p.origen.consola}»)` : (p.quien ?? "Ana"),
        autor: p.quien ?? "Ana",
        origen: p.origen ?? null,
        adjuntos: [],
        adjuntos_fuera: p.adjuntos_fuera ?? [],
      });
    }
  }
  return x;
}

function plazo(meses: number, aviso: number, ultima: number | null, visto: number): PlazoPrueba {
  const vence = (ultima ?? visto) + meses * MES;
  const ahora = Date.now();
  const fase = ahora >= vence ? "vencida" : ahora >= vence - aviso * 86_400_000 ? "pronto" : "al_dia";
  return { ultima: ultima ? new Date(ultima).toISOString() : null, vence: new Date(vence).toISOString(), fase, meses };
}

function todo(c: string): PruebasFuncionales {
  const x = de(c);
  const cuenta = x.pruebas.filter((p) => p.resultado !== "fallo");
  const t = (p: PruebaFuncional) => Date.parse(`${p.fecha}T12:00:00Z`);
  const ultima = cuenta.length ? Math.max(...cuenta.map(t)) : null;
  const pol = x.politica;
  const pruebas = [...x.pruebas]
    .sort((a, b) => b.fecha.localeCompare(a.fecha))
    .map((p) => ({
      ...p,
      cumple_rto: pol.rto_objetivo_min && p.rto_min != null ? p.rto_min <= pol.rto_objetivo_min : null,
      cumple_rpo: pol.rpo_objetivo_min && p.rpo_min != null ? p.rpo_min <= pol.rpo_objetivo_min : null,
    }));
  return {
    ajustes: { ...pol, actualizada: null, por: null },
    estado: pol.activo
      ? {
          cliente: plazo(pol.meses, pol.aviso_dias, ultima, x.visto),
          repos: pol.repos.map((r) => {
            const u = cuenta.filter((p) => p.alcance === "cliente" || p.repos.some((q) => q.equipo === r.equipo && q.repo === r.repo)).map(t);
            return { equipo: r.equipo, repo: r.repo, plazo: plazo(r.meses, pol.aviso_dias, u.length ? Math.max(...u) : null, x.visto) };
          }),
        }
      : null,
    pruebas,
    limites: { adjunto: 2 * 1024 * 1024, adjuntos: 5 },
  };
}

function validar(err: Err, e: EntradaPruebaFuncional): EntradaPruebaFuncional {
  if (!/^\d{4}-\d{2}-\d{2}$/.test(e.fecha ?? "")) throw err(422, "datos", "Fecha no válida: AAAA-MM-DD.");
  if (e.fecha > dia(Date.now() + 2 * 86_400_000)) throw err(422, "datos", "La fecha de la prueba no puede ser futura.");
  if (!["programada", "auditoria", "cambio", "incidente", "otro"].includes(e.motivo)) throw err(422, "datos", "Motivo no válido.");
  if (!["ok", "parcial", "fallo"].includes(e.resultado)) throw err(422, "datos", "Resultado no válido.");
  if (e.alcance === "repos" && !e.repos?.length) throw err(422, "datos", "Elige qué repositorios se probaron (o «Todo el cliente»).");
  return { ...e, repos: e.alcance === "cliente" ? [] : (e.repos ?? []), incidencias: e.incidencias?.trim() || null, notas: e.notas?.trim() || null };
}

function tipoDe(b: Buffer): string | null {
  if (b.subarray(0, 5).toString("latin1") === "%PDF-") return "application/pdf";
  if (b[0] === 0x89 && b.subarray(1, 4).toString("latin1") === "PNG") return "image/png";
  if (b[0] === 0xff && b[1] === 0xd8 && b[2] === 0xff) return "image/jpeg";
  return null;
}

export function rutasPruebas<C2 extends Ctx>(h: Ayudas<C2>): [string, RegExp, (ctx: C2, m: string[]) => unknown][] {
  const { err, miembro } = h;
  const prueba = (c: string, id: string) => {
    const p = de(c).pruebas.find((x) => x.id === id);
    if (!p) throw err(404, "no_existe", "Esa prueba no existe.");
    return p;
  };
  return [
    ["GET", new RegExp(`^${C}/pruebas-funcionales$`), (ctx, [c]) => (miembro(ctx, c, "lectura"), todo(c))],
    [
      "POST",
      new RegExp(`^${C}/pruebas-funcionales$`),
      (ctx, [c]) => {
        const { cuenta } = miembro(ctx, c, "tecnico");
        const e = validar(err, ctx.cuerpo as unknown as EntradaPruebaFuncional);
        const id = randomUUID();
        const ahora = new Date().toISOString();
        de(c).pruebas.push({ ...e, quien: e.quien?.trim() || cuenta.nombre, id, creada: ahora, actualizada: ahora, por: cuenta.nombre, adjuntos: [] });
        return { ...todo(c), id };
      },
    ],
    [
      "PUT",
      new RegExp(`^${C}/pruebas-funcionales/ajustes$`),
      (ctx, [c]) => {
        miembro(ctx, c, "administrador");
        const p = ctx.cuerpo as unknown as PoliticaPruebas;
        if (!(p.meses >= 1 && p.meses <= 36)) throw err(422, "datos", "Cada 1 a 36 meses.");
        de(c).politica = { activo: !!p.activo, meses: p.meses, aviso_dias: p.aviso_dias ?? 30, rto_objetivo_min: p.rto_objetivo_min ?? null, rpo_objetivo_min: p.rpo_objetivo_min ?? null, repos: p.repos ?? [] };
        return todo(c);
      },
    ],
    [
      "PUT",
      new RegExp(`^${C}/pruebas-funcionales/([^/]+)$`),
      (ctx, [c, id]) => {
        const { cuenta } = miembro(ctx, c, "tecnico");
        const p = prueba(c, id);
        const e = validar(err, ctx.cuerpo as unknown as EntradaPruebaFuncional);
        Object.assign(p, e, { quien: e.quien?.trim() || cuenta.nombre, actualizada: new Date().toISOString(), por: cuenta.nombre });
        return { ...todo(c), id };
      },
    ],
    [
      "DELETE",
      new RegExp(`^${C}/pruebas-funcionales/([^/]+)$`),
      (ctx, [c, id]) => {
        miembro(ctx, c, "administrador");
        prueba(c, id);
        de(c).pruebas = de(c).pruebas.filter((x) => x.id !== id);
        return todo(c);
      },
    ],
    [
      "POST",
      new RegExp(`^${C}/pruebas-funcionales/([^/]+)/adjuntos$`),
      (ctx, [c, id]) => {
        const { cuenta } = miembro(ctx, c, "tecnico");
        const p = prueba(c, id);
        const b = Buffer.from(String(ctx.cuerpo.datos ?? ""), "base64");
        if (!b.length || b.length > 2 * 1024 * 1024) throw err(422, "datos", "El adjunto pesa demasiado: como mucho 2 MB.");
        const tipo = tipoDe(b);
        if (!tipo) throw err(422, "datos", "Solo PDF, PNG o JPG.");
        if (p.adjuntos.length >= 5) throw err(422, "datos", "Como mucho 5 adjuntos por prueba.");
        const n = Math.max(0, ...p.adjuntos.map((a) => a.n)) + 1;
        const base = String(ctx.cuerpo.nombre ?? "adjunto").replace(/^.*[\\/]/, "").replace(/\.[^.]*$/, "").replace(/[^\p{L}\p{N} _()-]/gu, "").slice(0, 80) || "adjunto";
        p.adjuntos.push({ n, nombre: `${base}.${tipo === "application/pdf" ? "pdf" : tipo === "image/png" ? "png" : "jpg"}`, tipo: tipo as PruebaFuncional["adjuntos"][number]["tipo"], tamano: b.length });
        de(c).datos.set(`${id}/${n}`, b);
        void cuenta;
        return todo(c);
      },
    ],
    [
      "GET",
      new RegExp(`^${C}/pruebas-funcionales/([^/]+)/adjuntos/(\\d+)$`),
      (ctx, [c, id, n]) => {
        miembro(ctx, c, "lectura");
        const a = prueba(c, id).adjuntos.find((x) => x.n === Number(n));
        const b = de(c).datos.get(`${id}/${n}`);
        if (!a || !b) throw err(404, "no_existe", "Ese adjunto no existe.");
        return h.archivo(ctx, a.tipo, a.nombre, b);
      },
    ],
    [
      "DELETE",
      new RegExp(`^${C}/pruebas-funcionales/([^/]+)/adjuntos/(\\d+)$`),
      (ctx, [c, id, n]) => {
        miembro(ctx, c, "tecnico");
        const p = prueba(c, id);
        if (!p.adjuntos.some((x) => x.n === Number(n))) throw err(404, "no_existe", "Ese adjunto no existe.");
        p.adjuntos = p.adjuntos.filter((x) => x.n !== Number(n));
        de(c).datos.delete(`${id}/${n}`);
        return todo(c);
      },
    ],
  ];
}

/** Al reiniciar el simulador. */
export function reiniciarPruebas() {
  porCliente.clear();
}
