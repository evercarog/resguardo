// 0.8.1 (H2, plan-mejoras 15qq): el «Paquete para auditoría» de un cliente y un
// periodo, en PDF y CSV. Es **evidencia** de lo que se protege y de cómo se ha
// protegido; no es una certificación: el cumplimiento lo evalúa quien audita.
//
// Cada parte dice a qué control responde: ISO/IEC 27001:2022 A.5.9 (inventario, en lo
// que toca a los activos con datos), A.8.13 (copias de seguridad), A.5.30 (preparación
// de las TIC para la continuidad), A.8.14 (redundancia) y, para el acceso a la consola,
// A.5.15 y A.5.18; CIS Controls v8, control 11 («Recuperación de datos»); y la
// protección de datos personales en general (sin citar la ley de ningún país).
//
// Se arma en el navegador con lo que ya ve la consola (el estado de los equipos, su
// historial, las restauraciones, las pruebas funcionales y las personas del cliente) y,
// con la clave de administración, la configuración de cada copia (lib/informeConfig.ts).
// Nada de contraseñas, claves, números de serie, usuarios del sistema ni programas
// instalados. Lógica pura: la prueban los vectores (scripts/vectores-informe-config.ts).

import type { EntradaHistorial, Equipo, Miembro } from "./tipos";
import type { PruebaFuncional, PoliticaPruebas } from "./tiposPruebas";
import type { ConMotivo } from "./restauraciones";
import { textoMotivo, textoTardo } from "./restauraciones";
import type { Bloque, DocumentoPdf } from "./pdfTexto";
import { csv, type Celda } from "./csv";
import { bloquesConfig, type CopiaInforme } from "./informeConfig";
import { fichaDe, objetoRepo, textoCriticidad, textoDatosPersonales, textoMin, type FilaInventario } from "./inventario";
import { textoMotivoPrueba, resultadoPrueba, textoMinutos } from "./pruebasFuncionales";
import { bytes, fechaLarga, plural } from "./formato";
import type { Organizacion } from "./sedes";
import { sedeDeEquipo } from "./sedes";
import { textoTipo, tipoEquipo } from "./tipoEquipo";

/** Los controles, con su nombre. */
export const CONTROLES: Record<string, string> = {
  "A.5.9": "ISO/IEC 27001:2022 A.5.9 · Inventario de información y otros activos asociados",
  "A.5.15": "ISO/IEC 27001:2022 A.5.15 · Control de acceso",
  "A.5.18": "ISO/IEC 27001:2022 A.5.18 · Derechos de acceso",
  "A.5.30": "ISO/IEC 27001:2022 A.5.30 · Preparación de las TIC para la continuidad del negocio",
  "A.8.13": "ISO/IEC 27001:2022 A.8.13 · Copias de seguridad de la información",
  "A.8.14": "ISO/IEC 27001:2022 A.8.14 · Redundancia de las instalaciones de tratamiento de la información",
  "CIS 11": "CIS Controls v8 · Control 11, Recuperación de datos",
  DP: "Protección de datos personales (disponibilidad, integridad y acceso a las copias)",
};

/** Las partes del paquete y los controles a los que responde cada una. */
export const PARTES_PAQUETE: { id: string; titulo: string; controles: string[] }[] = [
  { id: "inventario", titulo: "Inventario de equipos y datos protegidos", controles: ["A.5.9", "DP"] },
  { id: "donde", titulo: "Dónde está cada copia y regla 3-2-1-1-0", controles: ["A.8.13", "A.8.14", "CIS 11"] },
  { id: "retencion", titulo: "Retención aplicada", controles: ["A.8.13", "CIS 11"] },
  { id: "verificaciones", titulo: "Verificaciones de las copias", controles: ["A.8.13", "CIS 11"] },
  { id: "pruebas", titulo: "Pruebas de restauración y pruebas funcionales", controles: ["A.5.30", "A.8.13", "CIS 11"] },
  { id: "restauraciones", titulo: "Restauraciones y su motivo", controles: ["A.8.13", "A.5.30", "DP"] },
  { id: "incidentes", titulo: "Incidentes: copias fallidas y cómo se resolvieron", controles: ["A.8.13", "CIS 11"] },
  { id: "acceso", titulo: "Quién tiene acceso a la consola", controles: ["A.5.15", "A.5.18", "CIS 11", "DP"] },
  { id: "proteccion", titulo: "Cómo se protegen las copias", controles: ["A.8.13", "CIS 11", "DP"] },
  { id: "configuracion", titulo: "Configuración de cada copia", controles: ["A.8.13", "CIS 11"] },
];

export const AVISO_EVIDENCIA =
  "Este paquete es evidencia de lo que se protege y de cómo se ha protegido en el periodo. No es una certificación ni garantiza el cumplimiento de ninguna norma: el cumplimiento lo evalúa quien audita, con esta y otras evidencias. Las referencias a controles indican a qué control aporta evidencia cada parte.";

export interface DatosPaquete {
  cliente: string;
  desde: Date;
  hasta: Date;
  /** «Generado el … por …». */
  generado: string;
  equipos: Equipo[];
  organizacion: Organizacion | null;
  /** El informe de la configuración (lib/informeConfig.ts), con o sin la clave. */
  copias: CopiaInforme[];
  conClave: boolean;
  /** Del periodo: copias, verificaciones y pruebas de restauración de cada equipo. */
  historial: Record<string, EntradaHistorial[]>;
  restauraciones: ConMotivo[];
  funcionales: PruebaFuncional[];
  politica: PoliticaPruebas | null;
  miembros: Miembro[];
  /** El nombre de cada papel («Administrador»…). */
  nombreRol: (rol: string) => string;
  ahora: number;
}

const ctrl = (ids: string[]) => `Responde a: ${ids.map((i) => CONTROLES[i] ?? i).join(" · ")}.`;
const dentro = (d: DatosPaquete, hora: string) => {
  const t = Date.parse(hora);
  return t >= d.desde.getTime() && t < d.hasta.getTime();
};

/** El inventario: cada equipo y sus repositorios, con lo anotado. */
export function filasInventario(equipos: Equipo[], o: Organizacion | null): FilaInventario[] {
  const inv = o?.inventario ?? {};
  const out: FilaInventario[] = [];
  for (const e of equipos) {
    const fe = fichaDe(inv, "equipo", e.id);
    const sede = sedeDeEquipo(o, e.id).sede?.nombre ?? "";
    const tipoEq = textoTipo(tipoEquipo(e));
    const base = { equipo: e.nombre, idExterno: fe.id_externo ?? "", sede, tipoEquipo: tipoEq, sistema: e.so ?? "" };
    out.push({ ...base, tipo: "equipo", repositorio: "", ficha: fe, copias: e.resumen?.copias?.length ?? 0, versiones: null, bytes: null, ultima: null });
    for (const r of e.resumen?.repositorios ?? []) {
      out.push({
        ...base,
        tipo: "repositorio",
        repositorio: r.nombre,
        ficha: fichaDe(inv, "repositorio", objetoRepo(e.id, r.id)),
        copias: (e.resumen?.copias ?? []).filter((k) => k.repo === r.id).length,
        versiones: r.versiones ?? null,
        bytes: r.bytes ?? null,
        ultima: r.ultima_version ?? null,
      });
    }
  }
  return out;
}

/** Una copia fallida y si se resolvió (la siguiente de la misma copia salió bien). */
export interface Incidente {
  equipo: string;
  copia: string;
  hora: string;
  mensaje: string;
  resuelta: string | null;
}

/** Las copias fallidas del periodo y cómo se resolvieron. */
export function incidentes(d: DatosPaquete): Incidente[] {
  const out: Incidente[] = [];
  for (const e of d.equipos) {
    const hs = (d.historial[e.id] ?? []).filter((x) => x.tipo === "copia" && x.resultado).sort((a, b) => Date.parse(a.hora) - Date.parse(b.hora));
    const nombreCopia = (id: string | undefined) => e.resumen?.copias?.find((k) => k.id === id)?.nombre ?? "Una copia";
    hs.forEach((x, i) => {
      if (x.resultado !== "fallo" || !dentro(d, x.hora)) return;
      const bien = hs.slice(i + 1).find((y) => y.copia === x.copia && y.resultado !== "fallo");
      out.push({ equipo: e.nombre, copia: nombreCopia(x.copia), hora: x.hora, mensaje: (x.mensaje ?? "").slice(0, 300), resuelta: bien ? bien.hora : null });
    });
  }
  return out.sort((a, b) => Date.parse(b.hora) - Date.parse(a.hora));
}

/** Cuántas verificaciones o pruebas automáticas hubo por repositorio, y las que fallaron. */
function porRepo(d: DatosPaquete, tipo: "verificacion" | "prueba_restauracion") {
  const out: { equipo: string; repo: string; total: number; mal: EntradaHistorial[]; ultima: string | null }[] = [];
  for (const e of d.equipos) {
    const hs = (d.historial[e.id] ?? []).filter((x) => x.tipo === tipo && dentro(d, x.hora));
    const repos = new Map<string, EntradaHistorial[]>();
    for (const x of hs) repos.set(x.repo ?? "", [...(repos.get(x.repo ?? "") ?? []), x]);
    for (const [repo, xs] of repos) {
      const nombre = e.resumen?.repositorios?.find((r) => r.id === repo)?.nombre ?? "Repositorio quitado";
      const ultima = xs.map((x) => x.hora).sort().at(-1) ?? null;
      out.push({ equipo: e.nombre, repo: nombre, total: xs.length, mal: xs.filter((x) => x.resultado === "fallo"), ultima });
    }
  }
  return out;
}

const fecha = (h: string | null | undefined) => (h ? fechaLarga(h) : "—");

/** Los bloques del paquete (para el PDF). */
export function bloquesPaquete(d: DatosPaquete): Bloque[] {
  const b: Bloque[] = [];
  const titulo = (id: string) => {
    const p = PARTES_PAQUETE.find((x) => x.id === id)!;
    b.push({ t: "h1", texto: p.titulo }, { t: "p", texto: ctrl(p.controles), tenue: true });
  };
  const periodo = `del ${fechaLarga(d.desde.toISOString())} al ${fechaLarga(new Date(d.hasta.getTime() - 1).toISOString())}`;
  b.push(
    { t: "titulo", texto: "Paquete para auditoría" },
    { t: "p", texto: `${d.cliente} · Periodo ${periodo}` },
    { t: "p", texto: d.generado, tenue: true },
    { t: "h2", texto: "Qué es y qué no es" },
    { t: "p", texto: AVISO_EVIDENCIA },
  );

  // 1. Inventario.
  titulo("inventario");
  const filas = filasInventario(d.equipos, d.organizacion);
  const conDp = filas.filter((f) => f.ficha.datos_personales === true);
  b.push({
    t: "p",
    texto: `${plural(d.equipos.length, "equipo", "equipos")} y ${plural(filas.length - d.equipos.length, "repositorio", "repositorios")}. ${conDp.length ? `${plural(conDp.length, "lleva", "llevan")} datos personales según lo anotado.` : "Ninguno anotado con datos personales."} Solo se anota si hay datos personales, nunca su contenido.`,
  });
  for (const f of filas) {
    const n = f.ficha;
    const extra = [
      n.responsable ? `Responsable: ${n.responsable}` : "",
      n.criticidad ? `Criticidad: ${textoCriticidad(n.criticidad)}` : "",
      `Datos personales: ${textoDatosPersonales(n.datos_personales)}`,
      n.rto_min ? `RTO objetivo: ${textoMin(n.rto_min)}` : "",
      n.rpo_min ? `RPO objetivo: ${textoMin(n.rpo_min)}` : "",
    ].filter(Boolean);
    if (f.tipo === "equipo") {
      b.push({ t: "h2", texto: `${f.equipo}${f.idExterno ? ` (${f.idExterno})` : ""}` });
      b.push({ t: "kv", filas: [["Sede", f.sede || "—"], ["Tipo", f.tipoEquipo || "—"], ["Sistema", f.sistema || "—"], ["Copias", String(f.copias ?? 0)], ["Anotado", extra.join("\n")]] });
    } else {
      b.push({
        t: "kv",
        filas: [[`Repositorio «${f.repositorio}»`, [`${f.versiones ?? "—"} versiones · ${bytes(f.bytes)} · última ${fecha(f.ultima)}`, ...extra].join("\n")]],
      });
    }
  }

  // 2. Dónde está cada copia y la regla.
  titulo("donde");
  const cumplen = d.copias.filter((c) => c.regla.startsWith("Cumple")).length;
  b.push({ t: "p", texto: `${cumplen} de ${plural(d.copias.length, "copia cumple", "copias cumplen")} la regla 3-2-1-1-0 (3 copias, 2 soportes, 1 fuera del sitio, 1 inmutable o aislada y 0 errores al verificar y probar).` });
  for (const c of d.copias) {
    b.push({
      t: "kv",
      filas: [
        [`${c.equipo} · «${c.copia}»`, c.sitios.map((s) => `${s.que}: ${s.nombre}${s.ruta ? ` · ${s.ruta}` : ""}${s.estado ? ` · ${s.estado}` : ""}`).join("\n")],
        ["Regla 3-2-1-1-0", c.regla],
      ],
    });
  }

  // 3. Retención.
  titulo("retencion");
  for (const c of d.copias) b.push({ t: "kv", filas: [[`${c.equipo} · «${c.copia}»`, c.sitios.map((s) => `${s.que} (${s.nombre}): ${s.retencion}`).join("\n") + (c.conservadas.length ? `\nConservadas para siempre: ${c.conservadas.join("; ")}` : "")]] });

  // 4. Verificaciones.
  titulo("verificaciones");
  const ver = porRepo(d, "verificacion");
  if (!ver.length) b.push({ t: "p", texto: "Ninguna verificación en el periodo." });
  for (const v of ver)
    b.push({ t: "kv", filas: [[`${v.equipo} · «${v.repo}»`, `${plural(v.total, "verificación", "verificaciones")}${v.mal.length ? `, ${plural(v.mal.length, "con error", "con error")}` : ", todas correctas"} · última ${fecha(v.ultima)}${v.mal.map((m) => `\nCon error el ${fecha(m.hora)}${m.mensaje ? `: ${m.mensaje.slice(0, 200)}` : ""}`).join("")}`]] });
  b.push({ t: "p", texto: `Programadas: ${d.copias.map((c) => `${c.equipo} · «${c.copia}»: ${c.verificacion}`).join(" · ") || "ninguna"}.`, tenue: true });

  // 5. Pruebas.
  titulo("pruebas");
  b.push({ t: "h2", texto: "Pruebas de restauración automáticas" });
  const pr = porRepo(d, "prueba_restauracion");
  if (!pr.length) b.push({ t: "p", texto: "Ninguna en el periodo." });
  for (const v of pr) b.push({ t: "kv", filas: [[`${v.equipo} · «${v.repo}»`, `${plural(v.total, "prueba", "pruebas")}${v.mal.length ? `, ${plural(v.mal.length, "fallida", "fallidas")}` : ", todas correctas"} · última ${fecha(v.ultima)}`]] });
  b.push({ t: "h2", texto: "Pruebas funcionales (simulacros de recuperación)" });
  if (d.politica) b.push({ t: "p", texto: d.politica.activo ? `Política: una cada ${plural(d.politica.meses, "mes", "meses")}${d.politica.rto_objetivo_min ? `; RTO objetivo ${textoMinutos(d.politica.rto_objetivo_min)}` : ""}${d.politica.rpo_objetivo_min ? `; RPO objetivo ${textoMinutos(d.politica.rpo_objetivo_min)}` : ""}.` : "Sin política activa.", tenue: true });
  const fs = d.funcionales.filter((p) => dentro(d, `${p.fecha}T12:00:00Z`));
  if (!fs.length) b.push({ t: "p", texto: "Ninguna en el periodo." });
  for (const p of fs)
    b.push({
      t: "kv",
      filas: [[`${p.fecha} · ${resultadoPrueba(p.resultado).texto}`, [`${textoMotivoPrueba(p.motivo)} · por ${p.quien || p.autor || p.por}`, p.que ? `Qué: ${p.que}` : "", `RTO real: ${textoMinutos(p.rto_min)} · RPO real: ${textoMinutos(p.rpo_min)}`, p.incidencias ? `Incidencias: ${p.incidencias}` : ""].filter(Boolean).join("\n")]],
    });

  // 6. Restauraciones.
  titulo("restauraciones");
  const rs = d.restauraciones.filter((r) => dentro(d, r.hora));
  if (!rs.length) b.push({ t: "p", texto: "Ninguna restauración en el periodo." });
  const nombreEq = (id: string) => d.equipos.find((e) => e.id === id)?.nombre ?? "Equipo quitado";
  for (const r of rs)
    b.push({
      t: "kv",
      filas: [[`${fecha(r.hora)} · ${nombreEq(r.equipo)}`, [`${r.resultado === "ok" ? "Correcta" : "Falló"} · ${textoTardo(r.duracion_s)}${r.archivos != null ? ` · ${plural(r.archivos, "archivo", "archivos")}` : ""}`, `Motivo: ${textoMotivo(r.motivoFinal?.tipo) ?? "Sin motivo"}${r.motivoFinal?.notas ? ` (${r.motivoFinal.notas})` : ""}`, r.por ? `Pedida por ${r.por}` : ""].filter(Boolean).join("\n")]],
    });

  // 7. Incidentes.
  titulo("incidentes");
  const inc = incidentes(d);
  if (!inc.length) b.push({ t: "p", texto: "Ninguna copia fallida en el periodo." });
  else b.push({ t: "p", texto: `${plural(inc.length, "copia fallida", "copias fallidas")}; ${inc.filter((i) => i.resuelta).length} resueltas con una copia correcta después.` });
  for (const i of inc) b.push({ t: "kv", filas: [[`${fecha(i.hora)} · ${i.equipo} · «${i.copia}»`, `${i.mensaje || "Sin detalle"}\n${i.resuelta ? `Resuelta: la siguiente copia salió bien el ${fecha(i.resuelta)}` : "Sin resolver a la fecha del informe"}`]] });

  // 8. Acceso.
  titulo("acceso");
  b.push({ t: "p", texto: "Las personas con acceso a la consola de este cliente y su papel. Entran con contraseña y un segundo factor (código TOTP o llave de acceso). Ver no da acceso a las copias: restaurar o cambiar algo delicado pide además la contraseña del repositorio o la clave de administración, que el servidor no guarda." });
  b.push({ t: "kv", filas: d.miembros.map((m) => [m.nombre || "Sin nombre", d.nombreRol(m.rol)] as [string, string]) });

  // 9. Protección.
  titulo("proteccion");
  b.push({
    t: "lista",
    items: [
      "Las copias se cifran en el propio equipo (AES-256) antes de salir; la contraseña de cada repositorio solo la tienen el equipo y su kit de recuperación.",
      "El servidor de la consola no guarda contraseñas de repositorios ni la clave de administración, y no ve las rutas ni los nombres de los archivos.",
      "Los almacenes de la oficina aceptan copias en modo de solo añadir: un equipo no puede borrar lo que ya guardó.",
      "Lo delicado (restaurar en otro sitio, quitar repositorios, cambiar la retención) pide la clave de administración y, si borra, espera un plazo visible desde todas las consolas.",
      "Cada cambio queda en el registro de actividad, encadenado para que no se pueda rehacer sin que se note.",
    ],
  });

  // 10. Configuración.
  titulo("configuracion");
  b.push(...bloquesConfig(d.copias, { cliente: d.cliente, generado: d.generado, alcance: "", conClave: d.conClave }, "h2"));

  // La correspondencia.
  b.push({ t: "h1", texto: "Correspondencia con los controles" });
  b.push({ t: "kv", filas: Object.entries(CONTROLES).map(([id, nombre]) => [nombre, PARTES_PAQUETE.filter((p) => p.controles.includes(id)).map((p) => p.titulo).join("\n")] as [string, string]) });
  b.push({ t: "p", texto: AVISO_EVIDENCIA, tenue: true });
  return b;
}

export function documentoPaquete(d: DatosPaquete): DocumentoPdf {
  return { titulo: `Paquete para auditoría · ${d.cliente}`, pie: `${d.cliente} · Paquete para auditoría · Evidencia, no certificación`, bloques: bloquesPaquete(d) };
}

export const CABECERA_PAQUETE = ["Parte", "Controles", "Equipo", "Objeto", "Fecha", "Qué", "Detalle", "Resultado"];

/** El CSV del paquete: una fila por dato, con su parte y sus controles (para filtrar en Excel). */
export function csvPaquete(d: DatosPaquete): string {
  const filas: Celda[][] = [];
  const parte = (id: string) => {
    const p = PARTES_PAQUETE.find((x) => x.id === id)!;
    return [p.titulo, p.controles.join(" ")];
  };
  for (const f of filasInventario(d.equipos, d.organizacion))
    filas.push([
      ...parte("inventario"),
      f.equipo,
      f.tipo === "equipo" ? `Equipo${f.idExterno ? ` ${f.idExterno}` : ""}` : `Repositorio ${f.repositorio}`,
      "",
      "Inventario",
      [f.ficha.responsable ? `Responsable: ${f.ficha.responsable}` : "", f.ficha.criticidad ? `Criticidad: ${textoCriticidad(f.ficha.criticidad)}` : "", `Datos personales: ${textoDatosPersonales(f.ficha.datos_personales)}`, f.ficha.rto_min ? `RTO: ${textoMin(f.ficha.rto_min)}` : "", f.ficha.rpo_min ? `RPO: ${textoMin(f.ficha.rpo_min)}` : ""].filter(Boolean).join(" · "),
      "",
    ]);
  for (const c of d.copias) {
    for (const s of c.sitios) filas.push([...parte("donde"), c.equipo, c.copia, "", s.que, `${s.nombre}${s.ruta ? ` · ${s.ruta}` : ""}`, s.estado]);
    filas.push([...parte("donde"), c.equipo, c.copia, "", "Regla 3-2-1-1-0", c.partes.join(" · "), c.regla]);
    for (const s of c.sitios) filas.push([...parte("retencion"), c.equipo, c.copia, "", `Retención · ${s.que}`, s.retencion, ""]);
  }
  for (const e of d.equipos)
    for (const x of d.historial[e.id] ?? []) {
      if (!dentro(d, x.hora)) continue;
      const repo = e.resumen?.repositorios?.find((r) => r.id === x.repo)?.nombre ?? "";
      if (x.tipo === "verificacion") filas.push([...parte("verificaciones"), e.nombre, repo, x.hora, "Verificación", x.mensaje ?? "", x.resultado ?? ""]);
      if (x.tipo === "prueba_restauracion") filas.push([...parte("pruebas"), e.nombre, repo, x.hora, "Prueba de restauración automática", x.mensaje ?? "", x.resultado ?? ""]);
    }
  for (const p of d.funcionales.filter((x) => dentro(d, `${x.fecha}T12:00:00Z`)))
    filas.push([...parte("pruebas"), "", p.alcance === "cliente" ? "Todo el cliente" : plural(p.repos.length, "repositorio", "repositorios"), p.fecha, "Prueba funcional", `RTO ${textoMinutos(p.rto_min)} · RPO ${textoMinutos(p.rpo_min)}${p.incidencias ? ` · ${p.incidencias}` : ""}`, p.resultado]);
  const nombreEq = (id: string) => d.equipos.find((e) => e.id === id)?.nombre ?? "Equipo quitado";
  for (const r of d.restauraciones.filter((x) => dentro(d, x.hora)))
    filas.push([...parte("restauraciones"), nombreEq(r.equipo), r.repo, r.hora, "Restauración", `Motivo: ${textoMotivo(r.motivoFinal?.tipo) ?? "Sin motivo"}${r.motivoFinal?.notas ? ` (${r.motivoFinal.notas})` : ""} · ${textoTardo(r.duracion_s)}`, r.resultado]);
  for (const i of incidentes(d)) filas.push([...parte("incidentes"), i.equipo, i.copia, i.hora, "Copia fallida", i.mensaje, i.resuelta ? `Resuelta el ${i.resuelta}` : "Sin resolver"]);
  for (const m of d.miembros) filas.push([...parte("acceso"), "", m.nombre, "", "Persona con acceso", d.nombreRol(m.rol), ""]);
  return csv(CABECERA_PAQUETE, filas);
}
