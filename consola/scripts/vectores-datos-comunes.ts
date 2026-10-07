// Pruebas de «lo mismo en todas las consolas» (0.7.26, bloque 8; src/lib/datosComunes.ts,
// docs/consolas-multiples.md §6.5): el orden entre dos cambios (con los vectores de Rust,
// crates/protocolo/vectors/datos-cliente.json), juntar lo de varios equipos, qué queda por
// hacer y las diferencias en palabras. `npm run test:vectores` (con las demás).
import { readFileSync } from "node:fs";
import {
  claseDe,
  diferencia,
  equiposQueGuardan,
  fusionar,
  juntar,
  pendiente,
  pideClave,
  textoAviso,
  textoValor,
  trozos,
  claveColor,
  cambiadoPor,
  type DatosComunesCliente,
  type EntradaComun,
  type FilaComun,
} from "../src/lib/datosComunes";
import { NIVEL, SOLO_ADMIN_ROL } from "../src/lib/cripto/ordenes";
import type { Equipo } from "../src/lib/tipos";

let fallos = 0;
let total = 0;
function igual(nombre: string, obtenido: unknown, esperado: unknown) {
  total++;
  const ok = JSON.stringify(obtenido) === JSON.stringify(esperado);
  if (!ok) fallos++;
  console.log(`${ok ? "ok  " : "MAL "} ${nombre}${ok ? "" : `\n       obtenido: ${JSON.stringify(obtenido)}\n       esperado: ${JSON.stringify(esperado)}`}`);
}

console.log("\n· Vectores compartidos con Rust (crates/protocolo/vectors/datos-cliente.json)");
{
  const doc = JSON.parse(readFileSync(new URL("../../crates/protocolo/vectors/datos-cliente.json", import.meta.url), "utf8")) as {
    fusion: { nombre: string; entradas: EntradaComun[]; gana: unknown }[];
    claves: { clave: string; valida: boolean; pide_admin: boolean }[];
  };
  for (const c of doc.fusion) {
    const d: Record<string, EntradaComun> = {};
    for (const e of c.entradas) fusionar(d, "etiqueta.color:servidor", e);
    igual(c.nombre, d["etiqueta.color:servidor"].valor, c.gana);
  }
  for (const c of doc.claves) igual(`clave ${c.clave}`, [!!claseDe(c.clave), pideClave(c.clave)], [c.valida, c.pide_admin]);
}

console.log("\n· Juntar lo de varios equipos (cada uno puede ir por una versión)");
{
  const a = { "etiqueta.color:servidor": { valor: { nombre: "Servidor", color: 0 }, cambiado: "2026-10-07T10:00:00Z", identidad: "oficina" } };
  const b = {
    "etiqueta.color:servidor": { valor: { nombre: "Servidor", color: 2 }, cambiado: "2026-10-07T11:00:00Z", identidad: "en-linea" },
    "destino:zona:e1:principal": { valor: { nombre: "Almacén · Disco D", clase: "zona" }, cambiado: "2026-10-07T09:00:00Z", identidad: "oficina" },
    "inventado:x": { valor: 1, cambiado: "2026-10-07T09:00:00Z" },
  };
  const j = juntar([a, b]);
  igual("gana el más reciente de todos", (j["etiqueta.color:servidor"].valor as { color: number }).color, 2);
  igual("y lo que solo tiene uno", Object.keys(j).sort(), ["destino:zona:e1:principal", "etiqueta.color:servidor"]);
  igual("el orden de los equipos no importa", JSON.stringify(juntar([b, a])), JSON.stringify(j));
  igual("clave del color, como la del servidor", claveColor("  Sede   Norte "), "etiqueta.color:sede norte");
}

const fila = (x: Partial<FilaComun> & { clave: string }): FilaComun => ({
  valor: null,
  cambiado: "2026-10-07T10:00:00Z",
  consola: null,
  esta: false,
  por: null,
  semilla: false,
  estado: "aplicado",
  local: null,
  por_enviar: false,
  ...x,
});

console.log("\n· Qué queda por hacer");
{
  const d: DatosComunesCliente = {
    filas: [
      fila({ clave: "etiqueta.color:servidor", valor: { nombre: "Servidor", color: 0 }, local: null, estado: "conflicto", consola: "Oficina" }),
      fila({ clave: "etiqueta.color:oficina", valor: { nombre: "Oficina", color: 2 }, esta: true, por_enviar: true, por: "Ana" }),
      fila({ clave: "destino.regla:zona:e1:principal", valor: { clase: "zona", atributos: { tipo: "fuera" } }, esta: true, por_enviar: true }),
      fila({ clave: "plantilla:pla-1", valor: { cifrado: "x", sal: "y", cliente: "c" }, estado: "por_traer" }),
      fila({ clave: "etiqueta.color:sede", valor: { nombre: "Sede", color: 1 }, esta: true }),
    ],
    sin_compartir: [
      { clave: "destino:destino-1", valor: { nombre: "Nube sur", clase: "b2", donde: "copias-sur" } },
      { clave: "destino.regla:destino-1", valor: { clase: "b2", atributos: { tipo: "nube" } } },
    ],
  };
  const p = pendiente(d);
  igual("diferencias", p.diferencias.map((f) => f.clave), ["etiqueta.color:servidor"]);
  igual("por enviar sin clave", p.porEnviar, [{ clave: "etiqueta.color:oficina", valor: { nombre: "Oficina", color: 2 }, cambiado: "2026-10-07T10:00:00Z", por: "Ana" }]);
  igual("por enviar con clave", p.porEnviarConClave.map((x) => x.clave), ["destino.regla:zona:e1:principal"]);
  igual("sin compartir, separado por clave", [p.sinCompartir.map((x) => x.clave), p.sinCompartirConClave.map((x) => x.clave)], [["destino:destino-1"], ["destino.regla:destino-1"]]);
  igual("plantillas por traer", p.porTraer.map((x) => x.clave), ["plantilla:pla-1"]);
  igual("el aviso", textoAviso(p), "1 dato distinto entre consolas · 2 tipos de destino por repartir (piden la clave) · 1 plantilla de otra consola por traer");
  igual("sin nada a mano: sin aviso (lo demás se manda solo)", textoAviso(pendiente({ filas: [d.filas[1]], sin_compartir: [d.sin_compartir[0]] })), null);
  igual("un servidor anterior (null): nada", textoAviso(pendiente(null)), null);
  igual("una diferencia no se manda sola aunque se marcara por enviar", pendiente({ filas: [fila({ clave: "etiqueta.color:x", estado: "conflicto", por_enviar: true })], sin_compartir: [] }).porEnviar, []);
}

console.log("\n· Las diferencias, en palabras (8.3)");
{
  const nombres = { destino: (id: string) => (id === "zona:e1:principal" ? "Almacén · Disco D" : null) };
  const d1 = diferencia(fila({ clave: "etiqueta.color:servidor", valor: null, local: { nombre: "Servidor", color: 0 }, estado: "conflicto", consola: null }));
  igual("color: «En esta consola: azul · En la otra: sin color»", [d1.titulo, d1.frase], ["Color de «Servidor»", "En esta consola: azul · En la otra: sin color"]);
  igual("los valores de cada lado", [d1.valorAqui, d1.valorOtra, d1.conClave], [{ nombre: "Servidor", color: 0 }, null, false]);
  const d2 = diferencia(
    fila({ clave: "destino.regla:zona:e1:principal", valor: { clase: "zona", atributos: { tipo: "fuera", inmutable: "object_lock", bloqueo_dias: 30 } }, local: { clase: "zona", atributos: { tipo: "local", aislado: true } }, estado: "conflicto", consola: "Oficina" }),
    nombres,
  );
  igual("tipo y marcas, con el nombre de aquí y la clave", [d2.titulo, d2.frase, d2.conClave], ["Tipo y marcas de «Almacén · Disco D»", "En esta consola: Local · Aislado · En «Oficina»: Fuera del sitio · Inmutable 30 días", true]);
  const d3 = diferencia(fila({ clave: "destino:destino-1", valor: { nombre: "Nube sur", clase: "b2" }, local: { nombre: "Nube norte", clase: "b2" }, estado: "conflicto" }));
  igual("nombre de un destino", d3.frase, "En esta consola: «Nube norte» · En la otra: «Nube sur»");
  const d4 = diferencia(fila({ clave: "plantilla:pla-1", valor: null, local: { aqui: true }, estado: "conflicto" }));
  igual("una plantilla borrada en la otra: elegir esta la reparte tal cual", [d4.frase, d4.valorAqui, d4.valorOtra], ["En esta consola: la tiene · En la otra: borrada", true, null]);
  igual("valor de un color", textoValor("etiqueta.color:x", { nombre: "x", color: 4 }), "bermellón");
  igual("quién lo cambió", cambiadoPor({ esta: false, consola: "Oficina", por: "Ana" }), "Lo cambió Ana desde la consola «Oficina»");
  igual("lo cambió esta: nada", cambiadoPor({ esta: true, consola: null, por: "Ana" }), null);
}

console.log("\n· Órdenes y equipos");
{
  const muchas = Array.from({ length: 230 }, (_, i) => ({ clave: `etiqueta.color:e${i}`, valor: { nombre: `e${i}`, color: 1 }, cambiado: "2026-10-07T10:00:00Z" }));
  igual("como mucho 100 por orden", trozos(muchas).map((t) => t.length), [100, 100, 30]);
  const grande = { clave: "plantilla:pla-1", valor: { cifrado: "A".repeat(30_000), sal: "c2Fs", cliente: "c" }, cambiado: "2026-10-07T10:00:00Z" };
  igual("y lo que cabe en un sobre", trozos([grande, grande, muchas[0]]).map((t) => t.length), [1, 2]);
  igual("niveles (como protocolo/ordenes.rs)", [NIVEL.datos_cliente, NIVEL.datos_cliente_admin], ["sesion", "admin"]);
  igual("solo administradores", [SOLO_ADMIN_ROL.has("datos_cliente"), SOLO_ADMIN_ROL.has("datos_cliente_admin")], [true, true]);
  const base = { so: "Windows 11", version_agente: "0.7.26", box_pub: "", sign_pub: "", sal_equipo: "", etiqueta: null, conectado: true, ultimo_contacto: null, estado_servicio: "en_marcha" as const, siguiente_seq: 1, rol: "agente" as const };
  const eqs: Equipo[] = [
    { ...base, id: "a", nombre: "A", modo: "gestionado", confirmado: true, resumen: { admite: ["datos_cliente"] } },
    { ...base, id: "b", nombre: "B (anterior)", modo: "gestionado", confirmado: true, resumen: { admite: ["datos_equipo"] } },
    { ...base, id: "c", nombre: "C (sin confirmar)", modo: "gestionado", confirmado: false, resumen: { admite: ["datos_cliente"] } },
    { ...base, id: "d", nombre: "D (local)", modo: "local", confirmado: true, resumen: { admite: ["datos_cliente"] } },
  ];
  igual("solo a los que lo guardan (agente nuevo, confirmado, gestionado)", equiposQueGuardan(eqs).map((e) => e.id), ["a"]);
}

console.log(`\n${total - fallos}/${total} bien`);
if (fallos) process.exit(1);
