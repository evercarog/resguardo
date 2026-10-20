// 0.8.1 (H5, plan-mejoras 15tt): el «Informe de la configuración de copias» de un
// cliente, para leer, imprimir y mandar al cliente (PDF) o abrir en Excel (CSV).
//
// Por equipo (con su sede y su tipo), cada copia: qué carpetas copia, qué deja fuera
// (reglas y grupos), qué hace antes de copiar, cada cuánto, dónde se guarda y su camino
// (repositorio → copias derivadas → espejos, cada uno con su horario, su destino y su
// ruta), la retención de cada sitio, la verificación y la prueba de restauración
// programadas, las versiones conservadas para siempre, cómo está ahora y la regla
// 3-2-1-1-0.
//
// Las carpetas, las exclusiones y los volcados van **cifrados** en el servidor (con la
// clave de administración: el servidor nunca ve rutas de los equipos). Con la clave,
// el navegador descifra la configuración de cada equipo y el informe las trae; sin
// ella, el informe dice cuántas carpetas hay y que hace falta la clave para verlas.
// Nada de contraseñas, claves ni credenciales: el informe se arma solo con lo que ya
// se ve en la consola. Lógica pura (sin runas): la prueban los vectores
// (scripts/vectores-informe-config.ts).

import type { Configuracion, CopiaConfig, Equipo, Informe, RepositorioResumen, DestinoCatalogo } from "./tipos";
import type { PruebaFuncional } from "./tiposPruebas";
import type { Organizacion } from "./sedes";
import type { Infraestructura } from "./infra";
import { reglasDelCliente, fraseRegla, PARTES, type ReglaCopia, type PasoVista } from "./regla321";
import { caminoDeCopia, type Eslabon } from "./camino";
import { sitiosDe } from "./dondeEsta";
import { cuandoCopiaEnFrase, ultimaVuelta } from "./copia";
import { ganchosDe, fraseGancho } from "./ganchos";
import { resumenRegla, almacenDe, almacenDeDerivada, reglaDe } from "./retencion";
import { destinoDe } from "./repo";
import { textoCuando, textoRetencion, trabajosDelAlmacen, trabajosDelEquipo } from "./espejoTrabajos";
import { cuandoEnFrase } from "./cadenas";
import { cuandoEspejo, textoRetencion as textoRetencionEspejo } from "./espejo";
import { fraseVerificacion } from "./verificacion";
import { sedeDeEquipo } from "./sedes";
import { textoTipo, tipoEquipo } from "./tipoEquipo";
import { fechaLarga, plural, relativo } from "./formato";
import type { Bloque, DocumentoPdf } from "./pdfTexto";
import { csv } from "./csv";

/** Un sitio del camino de una copia (su repositorio, un espejo o una copia derivada). */
export interface SitioInforme {
  clase: "repositorio" | "espejo" | "derivada";
  /** «Repositorio», «Espejo», «Copia derivada». */
  que: string;
  nombre: string;
  /** Quién lo hace, si no es el propio equipo (el almacén). */
  quien: string | null;
  ruta: string | null;
  cuando: string;
  retencion: string;
  estado: string;
}

/** Una copia, con todo lo del informe. */
export interface CopiaInforme {
  equipo: string;
  equipoId: string;
  sede: string;
  tipo: string;
  copia: string;
  repositorio: string;
  /** `null`: no se sabe (sin la clave de administración). */
  carpetas: string[] | null;
  nCarpetas: number | null;
  /** Las reglas sueltas (sin las de los grupos reconocidos). `null`: sin la clave. */
  exclusiones: string[] | null;
  /** Los grupos de exclusiones que lleva enteros. */
  grupos: string[];
  /** Lo de «antes de copiar», en palabras. `null`: sin la clave. */
  antes: string[] | null;
  cuando: string;
  activa: boolean;
  sitios: SitioInforme[];
  verificacion: string;
  prueba: string;
  conservadas: string[];
  estado: string;
  ultima: string;
  regla: string;
  /** Cada parte de la regla: «3 copias: cumple». */
  partes: string[];
}

export interface EntradaInforme {
  /** Los equipos que entran (ya filtrados). */
  equipos: Equipo[];
  /** Todos los del cliente (para el almacén, los espejos…). */
  todos: Equipo[];
  informes: Record<string, Informe | null | undefined>;
  catalogo: DestinoCatalogo[];
  organizacion: Organizacion | null;
  funcionales: PruebaFuncional[];
  infra: Infraestructura | null;
  /** La configuración descifrada de cada equipo (con la clave de administración), si la hay. */
  configs: Record<string, Configuracion | null | undefined>;
  /** Los grupos de exclusiones del cliente, para reconocerlos en las reglas de una copia. */
  grupos: { nombre: string; reglas: string[] }[];
  ahora: number;
}

const QUE: Record<SitioInforme["clase"], string> = { repositorio: "Repositorio", espejo: "Espejo", derivada: "Copia derivada" };

/** Las reglas de una copia separadas en grupos enteros y reglas sueltas. */
export function separarGrupos(reglas: string[], grupos: { nombre: string; reglas: string[] }[]): { grupos: string[]; sueltas: string[] } {
  const usadas = new Set<string>();
  const nombres: string[] = [];
  for (const g of grupos) {
    if (g.reglas.length && g.reglas.every((r) => reglas.includes(r))) {
      nombres.push(g.nombre);
      for (const r of g.reglas) usadas.add(r);
    }
  }
  return { grupos: nombres, sueltas: reglas.filter((r) => !usadas.has(r)) };
}

/** Cuándo y con qué retención va un paso del camino. */
function cuandoYRetencion(p: PasoVista, rc: ReglaCopia, todos: Equipo[]): { cuando: string; retencion: string } {
  const repo = rc.repo;
  if (p.tipo === "espejo") {
    const hace = todos.find((e) => e.id === p.equipoId);
    const t = [...trabajosDelAlmacen(hace), ...trabajosDelEquipo(hace)].find((x) => x.id === p.trabajo);
    if (t) return { cuando: textoCuando(t), retencion: textoRetencion(t) };
    // El espejo de antes del almacén (por destino, sin trabajos).
    const esp = hace?.resumen?.guarda_copias?.espejo;
    const dd = esp?.destinos?.find((x) => (p.espejo?.nube ? x.nube === p.espejo.nube : x.carpeta === p.espejo?.carpeta));
    if (esp && dd) return { cuando: cuandoEspejo(dd, esp.hora), retencion: textoRetencionEspejo(dd) };
    return { cuando: "Lo decide quien lo hace", retencion: "Sin datos de su retención aquí" };
  }
  if (p.tipo === "derivada" || p.tipo === "externa") {
    const id = p.tipo === "externa" ? "externa" : p.id.replace(/^derivada-/, "");
    const d = id === "externa" ? repo.externa : repo.derivadas?.find((x) => x.id === id);
    const cuando = d ? (id === "externa" ? `Cada día a las ${(d as NonNullable<RepositorioResumen["externa"]>).hora}` : cuandoEnFrase((d as NonNullable<RepositorioResumen["derivadas"]>[number]).cuando)) : "";
    const alm = d ? almacenDeDerivada(repo, { id, destino_id: (d as { destino_id?: string | null }).destino_id, ruta: (d as { ruta?: string | null }).ruta }, rc.equipo.resumen?.destinos, todos) : null;
    let retencion = d?.con_retencion ? "Con su propia retención" : "Sin retención propia: guarda todas las versiones";
    if (alm?.retencion) retencion = `En el almacén (${alm.almacen.nombre}): ${resumenRegla(alm.retencion.retencion)}`;
    if (d?.bloqueo_dias) retencion += ` · Bloqueo de objetos ${d.bloqueo_dias} días`;
    return { cuando: cuando || "Con su horario", retencion };
  }
  // El repositorio de la copia: la retención del equipo y, si está en un almacén, la del almacén.
  const partes: string[] = [];
  const r = reglaDe(repo);
  if (r) partes.push(`En el equipo: ${resumenRegla(r)}`);
  else if (repo.retencion) partes.push(`En el equipo: ${repo.retencion}`);
  const alm = almacenDe(repo, destinoDe(rc.equipo.resumen?.destinos, repo), todos);
  if (alm?.retencion) partes.push(`En el almacén (${alm.almacen.nombre}): ${resumenRegla(alm.retencion.retencion)}`);
  if (!partes.length) partes.push(repo.solo_anadir ? "Ninguna aquí: el destino es de solo añadir (la aplica quien lo gestiona)" : "Sin retención: guarda todas las versiones");
  return { cuando: cuandoCopiaEnFrase(rc.copia), retencion: partes.join(" · ") };
}

/** Las copias del informe, por equipo (en el orden de la lista de equipos) y por nombre. */
export function copiasInforme(x: EntradaInforme): CopiaInforme[] {
  const reglas = reglasDelCliente(x.equipos, x.informes, x.catalogo, x.ahora, x.todos, x.organizacion, x.funcionales, x.infra);
  const out: CopiaInforme[] = [];
  for (const e of x.equipos) {
    const cfg = x.configs[e.id] ?? null;
    const sede = sedeDeEquipo(x.organizacion, e.id).sede?.nombre ?? "";
    const tipo = textoTipo(tipoEquipo(e));
    const suyas = reglas.filter((rc) => rc.equipo.id === e.id).sort((a, b) => a.copia.nombre.localeCompare(b.copia.nombre, "es"));
    for (const rc of suyas) {
      const k = rc.copia;
      const kc: CopiaConfig | undefined = cfg?.copias.find((c) => c.id === k.id);
      const camino: Eslabon[] = caminoDeCopia(rc, { cliente: "", ahora: x.ahora, equipos: x.todos, informe: x.informes[e.id] });
      const sitios = sitiosDe(rc, camino, x.todos, x.informes[e.id]);
      const sep = kc ? separarGrupos(kc.exclusiones ?? [], x.grupos) : { grupos: [], sueltas: [] };
      const repo = rc.repo;
      const vuelta = ultimaVuelta(k, x.informes[e.id]);
      const conservadas = (e.resumen?.conservadas ?? []).filter((v) => v.repo === repo.id).map((v) => `${v.version.slice(0, 8)} · ${v.motivo} (${fechaLarga(v.cuando)})`);
      out.push({
        equipo: e.nombre,
        equipoId: e.id,
        sede,
        tipo,
        copia: k.nombre,
        repositorio: repo.nombre,
        carpetas: kc ? [...kc.carpetas] : null,
        nCarpetas: kc ? kc.carpetas.length : (k.carpetas ?? null),
        exclusiones: kc ? sep.sueltas : null,
        grupos: sep.grupos,
        antes: kc ? ganchosDe(kc.gancho).map(fraseGancho) : null,
        cuando: cuandoCopiaEnFrase(k, x.ahora),
        activa: k.activa !== false,
        sitios: rc.pasos.map((p, i) => {
          const s = sitios.find((y) => y.id === p.id);
          const cr = cuandoYRetencion(p, rc, x.todos);
          const clase = s?.clase ?? (p.tipo === "copia" ? "repositorio" : p.tipo === "espejo" ? "espejo" : "derivada");
          return {
            clase,
            que: QUE[clase],
            nombre: p.nombre,
            quien: s?.quien ?? null,
            ruta: s?.ruta ?? null,
            cuando: i === 0 ? cuandoCopiaEnFrase(k, x.ahora) : cr.cuando,
            retencion: cr.retencion,
            estado: s?.estado ?? camino.find((c) => c.id === p.id)?.estado ?? "",
          };
        }),
        verificacion: repo.verificacion_auto ? fraseVerificacion(repo.verificacion_auto) : "No programada",
        prueba: repo.prueba_auto ? `Automática cada ${plural(repo.prueba_auto.cada_dias, "día", "días")}` : "No programada",
        conservadas,
        estado: camino[0]?.estado ?? "",
        ultima: vuelta ? `${fechaLarga(vuelta.cuando)} (${relativo(vuelta.cuando, x.ahora)})` : "Ninguna todavía",
        regla: fraseRegla(rc.regla),
        partes: rc.regla.partes.map((p) => `${PARTES[p.id].cifra} ${PARTES[p.id].titulo}: ${p.cumple ? "cumple" : p.cumple_config ? "no está al día" : "falta"}`),
      });
    }
  }
  return out;
}

/** Lo que dice el informe de las carpetas, las exclusiones y lo de antes de copiar. */
function textoCarpetas(c: CopiaInforme): string {
  if (c.carpetas) return c.carpetas.length ? c.carpetas.join("\n") : "Ninguna";
  return `${c.nCarpetas != null ? plural(c.nCarpetas, "carpeta", "carpetas") : "Sus carpetas"} (escribe la clave de administración para verlas)`;
}
function textoExclusiones(c: CopiaInforme): string {
  if (!c.exclusiones) return "Escribe la clave de administración para verlas";
  const partes = [...c.grupos.map((g) => `Grupo «${g}»`), ...c.exclusiones];
  return partes.length ? partes.join("\n") : "Nada";
}
const textoAntes = (c: CopiaInforme) => (!c.antes ? "Escribe la clave de administración para verlo" : c.antes.length ? c.antes.join("\n") : "Nada");

export interface OpcionesDoc {
  cliente: string;
  /** «Generado el 20 de octubre de 2026 por Ana». */
  generado: string;
  /** Qué equipos entran («Todos los equipos», «Sede Norte»…). */
  alcance: string;
  conClave: boolean;
}

/** Los bloques del informe (para el PDF y, en el paquete para auditoría, como una parte más). */
export function bloquesConfig(copias: CopiaInforme[], o: OpcionesDoc, nivel: "h1" | "h2" = "h1"): Bloque[] {
  const sub = nivel === "h1" ? "h2" : "h2";
  const b: Bloque[] = [];
  if (!o.conClave) b.push({ t: "p", texto: "Sin la clave de administración: las carpetas, las exclusiones y lo de «antes de copiar» van cifrados y no salen en este informe.", tenue: true });
  if (!copias.length) b.push({ t: "p", texto: "No hay copias en estos equipos." });
  let equipo = "";
  for (const c of copias) {
    if (c.equipoId !== equipo) {
      equipo = c.equipoId;
      b.push({ t: nivel, texto: `${c.equipo}` });
      b.push({ t: "p", texto: [c.sede ? `Sede: ${c.sede}` : "", c.tipo ? `Tipo: ${c.tipo}` : ""].filter(Boolean).join(" · ") || "Sin sede ni tipo anotados", tenue: true });
    }
    b.push({ t: sub, texto: `Copia «${c.copia}»${c.activa ? "" : " (desactivada)"}` });
    const filas: [string, string][] = [
      ["Qué copia", textoCarpetas(c)],
      ["No copia", textoExclusiones(c)],
      ["Antes de copiar", textoAntes(c)],
      ["Cada cuánto", c.cuando],
      ["Repositorio", c.repositorio],
    ];
    c.sitios.forEach((s, i) => {
      const titulo = i === 0 ? "Dónde se guarda" : `${s.que}`;
      filas.push([titulo, [`${s.nombre}${s.quien ? ` (lo hace ${s.quien})` : ""}`, s.ruta ? `Ruta: ${s.ruta}` : "", i === 0 ? "" : `Cuándo: ${s.cuando}`, `Retención: ${s.retencion}`, s.estado ? `Estado: ${s.estado}` : ""].filter(Boolean).join("\n")]);
    });
    filas.push(["Verificación", c.verificacion], ["Prueba de restauración", c.prueba]);
    if (c.conservadas.length) filas.push(["Conservadas para siempre", c.conservadas.join("\n")]);
    filas.push(["Estado", `${c.estado}${c.estado ? " · " : ""}última copia: ${c.ultima}`], ["Regla 3-2-1-1-0", `${c.regla}\n${c.partes.join("\n")}`]);
    b.push({ t: "kv", filas });
  }
  return b;
}

/** El documento PDF entero. */
export function documentoConfig(copias: CopiaInforme[], o: OpcionesDoc): DocumentoPdf {
  const equipos = new Set(copias.map((c) => c.equipoId)).size;
  return {
    titulo: `Configuración de copias · ${o.cliente}`,
    pie: `${o.cliente} · Configuración de copias · ${o.generado}`,
    bloques: [
      { t: "titulo", texto: "Configuración de copias" },
      { t: "p", texto: `${o.cliente} · ${o.alcance} · ${plural(equipos, "equipo", "equipos")} · ${plural(copias.length, "copia", "copias")}` },
      { t: "p", texto: o.generado, tenue: true },
      { t: "p", texto: "Qué se copia de cada equipo, cuándo, adónde y qué se conserva. No lleva contraseñas, claves ni credenciales.", tenue: true },
      ...bloquesConfig(copias, o),
    ],
  };
}

export const CABECERA_CSV = [
  "Equipo",
  "Sede",
  "Tipo de equipo",
  "Copia",
  "Activa",
  "Carpetas",
  "Grupos de exclusiones",
  "Otras exclusiones",
  "Antes de copiar",
  "Cada cuánto (copia)",
  "Repositorio",
  "Paso",
  "Destino",
  "Lo hace",
  "Ruta",
  "Cuándo (paso)",
  "Retención",
  "Estado del paso",
  "Verificación",
  "Prueba de restauración",
  "Conservadas para siempre",
  "Última copia",
  "Estado de la copia",
  "Regla 3-2-1-1-0",
];

/** El CSV: una fila por copia y sitio de su camino (su repositorio, cada espejo y cada derivada). */
export function csvConfig(copias: CopiaInforme[]): string {
  const sinClave = "(con la clave de administración)";
  const filas = copias.flatMap((c) =>
    c.sitios.map((s) => [
      c.equipo,
      c.sede,
      c.tipo,
      c.copia,
      c.activa,
      c.carpetas ? c.carpetas.join(" | ") : sinClave,
      c.grupos.join(" | "),
      c.exclusiones ? c.exclusiones.join(" | ") : sinClave,
      c.antes ? c.antes.join(" | ") : sinClave,
      c.cuando,
      c.repositorio,
      s.que,
      s.nombre,
      s.quien ?? "",
      s.ruta ?? "",
      s.cuando,
      s.retencion,
      s.estado,
      c.verificacion,
      c.prueba,
      c.conservadas.join(" | "),
      c.ultima,
      c.estado,
      c.regla,
    ]),
  );
  return csv(CABECERA_CSV, filas);
}
