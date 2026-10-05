// Observaciones y comentarios simulados (v1.40, api-servidor.md §6). Como el
// servidor: leer, cualquier miembro; escribir, técnico o más; un comentario se
// cambia o borra en sus 15 minutos y un propietario borra cualquiera; hasta
// 2000 caracteres; la auditoría lo anota sin el texto.
import { randomUUID } from "node:crypto";
import type * as T from "../lib/tipos";
import { auditar, ID } from "./estado";

type Err = (estado: number, codigo: string, mensaje: string) => Error;
type Ctx = { cuerpo: Record<string, unknown>; url: URL };
type Ayudas<C> = {
  err: Err;
  miembro: (ctx: C, cliente: string, minimo: T.Rol) => { cuenta: { id: string; nombre: string }; rol: T.Rol };
};

type Obs = { tipo: T.TipoNota; objeto: string; texto: string; actualizada: string; por: string };
type Com = { id: string; tipo: T.TipoNota; objeto: string; texto: string; autor_id: string; autor: string; creado: string; editado: string | null };

const MAX = 2000;
const MINUTOS = 15;
const TIPOS: T.TipoNota[] = ["cliente", "equipo", "repositorio", "copia", "destino"];
let obs = new Map<string, Obs[]>();
let coms = new Map<string, Com[]>();

const hace = (min: number) => new Date(Date.now() - min * 60_000).toISOString();

/** Unos ejemplos en Altamar (ficticios). */
export function sembrarNotas() {
  obs = new Map([
    [
      ID.altamar,
      [
        { tipo: "cliente", objeto: ID.altamar, texto: "**Contrato de soporte** hasta diciembre de 2027.\nContacto: Marta (gerencia), ext. 204.", actualizada: hace(60 * 24 * 9), por: "Ana" },
        { tipo: "equipo", objeto: ID.contabilidad, texto: "Equipo de *Siigo*: no reiniciar en horario de caja.\n- Disco cambiado en octubre\n- Si falla la copia, llamar a Luis", actualizada: hace(60 * 30), por: "Ana" },
        { tipo: "copia", objeto: `${ID.contabilidad}/siigo`, texto: "La base de Siigo se copia cerrada: el programa debe estar cerrado a las 18:30.", actualizada: hace(60 * 24 * 2), por: "Ana" },
      ],
    ],
  ]);
  coms = new Map([
    [
      ID.altamar,
      [
        { id: randomUUID(), tipo: "equipo", objeto: ID.contabilidad, texto: "Cambié el disco por uno de 1 TB.", autor_id: "otra", autor: "Tomás", creado: hace(60 * 24 * 3), editado: null },
        { id: randomUUID(), tipo: "equipo", objeto: ID.contabilidad, texto: "Revisado: las copias vuelven a ir bien.", autor_id: "otra", autor: "Tomás", creado: hace(60 * 24 * 2), editado: null },
      ],
    ],
  ]);
}
sembrarNotas();

const limpio = (h: Err, t: unknown) => {
  const s = String(t ?? "").replace(/\r\n?/g, "\n").trim();
  if ([...s].length > MAX) throw h(422, "datos", `Como mucho ${MAX} caracteres.`);
  return s;
};
const titulo = (t: string) => {
  const l = (t.split("\n").find((x) => x.trim()) ?? "").trim().replace(/^[#>*+\-\s]+/, "").replace(/\[([^\]]+)\]\([^)]*\)/g, "$1").replace(/[*_`]/g, "").trim();
  return l.length > 80 ? `${l.slice(0, 79).trimEnd()}…` : l;
};

export function rutasNotas<C extends Ctx>(h: Ayudas<C>): [string, RegExp, (ctx: C, m: string[]) => unknown][] {
  const C = "/api/clientes/([^/]+)";
  const deObs = (c: string) => obs.get(c) ?? (obs.set(c, []), obs.get(c)!);
  const deComs = (c: string) => coms.get(c) ?? (coms.set(c, []), coms.get(c)!);
  const json = (k: Com, yo: string, rol: T.Rol): T.ComentarioNota => {
    const aTiempo = Date.now() - Date.parse(k.creado) < MINUTOS * 60_000;
    const mio = k.autor_id === yo && aTiempo && rol !== "lectura";
    return { id: k.id, texto: k.texto, autor: { id: k.autor_id, nombre: k.autor }, creado: k.creado, editado: k.editado, editable: mio, borrable: mio || rol === "propietario" };
  };
  const tipoOk = (t: unknown, o: unknown) => {
    if (!TIPOS.includes(t as T.TipoNota) || typeof o !== "string" || !o || o.length > 200) throw h.err(422, "datos", "Objeto no válido.");
  };
  return [
    [
      "GET",
      new RegExp(`^${C}/notas$`),
      (ctx, [c]) => {
        h.miembro(ctx, c, "lectura");
        const m = new Map<string, T.IndiceNota>();
        for (const o of deObs(c)) m.set(`${o.tipo}:${o.objeto}`, { tipo: o.tipo, objeto: o.objeto, titulo: titulo(o.texto) || null, observacion: true, comentarios: 0, actualizada: o.actualizada });
        for (const k of deComs(c)) {
          const x = m.get(`${k.tipo}:${k.objeto}`) ?? { tipo: k.tipo, objeto: k.objeto, titulo: null, observacion: false, comentarios: 0, actualizada: k.creado };
          x.comentarios++;
          m.set(`${k.tipo}:${k.objeto}`, x);
        }
        return { objetos: [...m.values()] };
      },
    ],
    [
      "GET",
      new RegExp(`^${C}/notas/objeto$`),
      (ctx, [c]) => {
        const { cuenta, rol } = h.miembro(ctx, c, "lectura");
        const [tipo, objeto] = [ctx.url.searchParams.get("tipo"), ctx.url.searchParams.get("objeto")];
        tipoOk(tipo, objeto);
        const o = deObs(c).find((x) => x.tipo === tipo && x.objeto === objeto);
        return {
          tipo,
          objeto,
          observacion: o ? { texto: o.texto, actualizada: o.actualizada, por: o.por } : null,
          comentarios: deComs(c)
            .filter((k) => k.tipo === tipo && k.objeto === objeto)
            .slice(-200)
            .map((k) => json(k, cuenta.id, rol)),
          minutos_cambio: MINUTOS,
        };
      },
    ],
    [
      "GET",
      new RegExp(`^${C}/notas/todas$`),
      (ctx, [c]) => {
        h.miembro(ctx, c, "administrador");
        return { observaciones: deObs(c), comentarios: deComs(c).map(({ autor_id: _, ...k }) => k) };
      },
    ],
    [
      "PUT",
      new RegExp(`^${C}/notas/observacion$`),
      (ctx, [c]) => {
        const { cuenta } = h.miembro(ctx, c, "tecnico");
        const b = ctx.cuerpo as { tipo?: T.TipoNota; objeto?: string; texto?: string };
        tipoOk(b.tipo, b.objeto);
        const texto = limpio(h.err, b.texto);
        const l = deObs(c).filter((x) => !(x.tipo === b.tipo && x.objeto === b.objeto));
        const nueva = texto ? { tipo: b.tipo!, objeto: b.objeto!, texto, actualizada: new Date().toISOString(), por: cuenta.nombre } : null;
        obs.set(c, nueva ? [...l, nueva] : l);
        auditar(c, cuenta.id, "poner_observacion", `${b.tipo}:${b.objeto}`, { tipo: b.tipo, caracteres: [...texto].length, borrada: !texto });
        return nueva ? { texto: nueva.texto, actualizada: nueva.actualizada, por: nueva.por } : null;
      },
    ],
    [
      "POST",
      new RegExp(`^${C}/notas/comentarios$`),
      (ctx, [c]) => {
        const { cuenta, rol } = h.miembro(ctx, c, "tecnico");
        const b = ctx.cuerpo as { tipo?: T.TipoNota; objeto?: string; texto?: string };
        tipoOk(b.tipo, b.objeto);
        const texto = limpio(h.err, b.texto);
        if (!texto) throw h.err(422, "datos", "Escribe algo.");
        const k: Com = { id: randomUUID(), tipo: b.tipo!, objeto: b.objeto!, texto, autor_id: cuenta.id, autor: cuenta.nombre, creado: new Date().toISOString(), editado: null };
        deComs(c).push(k);
        auditar(c, cuenta.id, "comentar", `${k.tipo}:${k.objeto}`, { id: k.id, caracteres: [...texto].length });
        return json(k, cuenta.id, rol);
      },
    ],
    [
      "PATCH",
      new RegExp(`^${C}/notas/comentarios/([^/]+)$`),
      (ctx, [c, id]) => {
        const { cuenta, rol } = h.miembro(ctx, c, "tecnico");
        const k = deComs(c).find((x) => x.id === id);
        if (!k) throw h.err(404, "no_existe", "Ese comentario ya no existe.");
        if (!json(k, cuenta.id, rol).editable) throw h.err(403, "prohibido", `Solo se puede cambiar durante ${MINUTOS} minutos. Añade otro comentario.`);
        const texto = limpio(h.err, (ctx.cuerpo as { texto?: string }).texto);
        if (!texto) throw h.err(422, "datos", "Escribe algo (o bórralo).");
        k.texto = texto;
        k.editado = new Date().toISOString();
        auditar(c, cuenta.id, "editar_comentario", `${k.tipo}:${k.objeto}`, { id: k.id, caracteres: [...texto].length });
        return json(k, cuenta.id, rol);
      },
    ],
    [
      "DELETE",
      new RegExp(`^${C}/notas/comentarios/([^/]+)$`),
      (ctx, [c, id]) => {
        const { cuenta, rol } = h.miembro(ctx, c, "tecnico");
        const k = deComs(c).find((x) => x.id === id);
        if (!k) throw h.err(404, "no_existe", "Ese comentario ya no existe.");
        if (!json(k, cuenta.id, rol).borrable) throw h.err(403, "prohibido", "Solo quien lo escribió (o una persona propietaria, para borrarlo).");
        coms.set(
          c,
          deComs(c).filter((x) => x.id !== id),
        );
        auditar(c, cuenta.id, "borrar_comentario", `${k.tipo}:${k.objeto}`, { id: k.id, de_otra_persona: k.autor_id !== cuenta.id });
        return undefined;
      },
    ],
  ];
}

/** Al importar un paquete (`POST …/importar`, campo `notas`): sin pisar lo que haya. */
export function importarNotas(c: string, n: T.NotasExportadas | undefined): { observaciones: number; comentarios: number } {
  if (!n) return { observaciones: 0, comentarios: 0 };
  const o = obs.get(c) ?? [];
  const k = coms.get(c) ?? [];
  let no = 0;
  let nk = 0;
  for (const x of n.observaciones ?? [])
    if (!o.some((y) => y.tipo === x.tipo && y.objeto === x.objeto)) {
      o.push({ ...x });
      no++;
    }
  for (const x of n.comentarios ?? [])
    if (!k.some((y) => y.id === x.id)) {
      k.push({ ...x, autor_id: "importado" });
      nk++;
    }
  obs.set(c, o);
  coms.set(c, k);
  return { observaciones: no, comentarios: nk };
}
