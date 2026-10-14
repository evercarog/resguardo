// Pruebas de las secciones de «Todos los clientes» (plan 0.8.0, D4, 14xx;
// src/lib/todosSecciones.ts): juntar lo de varios clientes con su cliente, sin
// mezclar ids iguales de clientes distintos. `npm run test:vectores` (con las demás).
import { avisosTodos, copiasTodos, equiposTodos, esperaTodos, restauracionesTodos } from "../src/lib/todosSecciones";
import type { PanelCliente, PanelDe } from "../src/lib/global";
import type { Aviso, Equipo, Orden, Restauraciones } from "../src/lib/tipos";

let fallos = 0;
let total = 0;
function igual(nombre: string, obtenido: unknown, esperado: unknown) {
  total++;
  const ok = JSON.stringify(obtenido) === JSON.stringify(esperado);
  if (!ok) fallos++;
  console.log(`${ok ? "ok  " : "MAL "} ${nombre}${ok ? "" : `\n       obtenido: ${JSON.stringify(obtenido)}\n       esperado: ${JSON.stringify(esperado)}`}`);
}

const AHORA = Date.parse("2026-10-14T12:00:00Z");
const hace = (h: number) => new Date(AHORA - h * 3600_000).toISOString();
const equipo = (id: string, nombre: string, ultima: number, extra: Partial<Equipo> = {}): Equipo =>
  ({
    id,
    nombre,
    so: "windows",
    confirmado: true,
    modo: "gestionado",
    conectado: true,
    ultimo_contacto: hace(0),
    resumen: {
      repositorios: [{ id: "r1", nombre: "Docs", destino: "Nube" }],
      copias: [{ id: "k1", nombre: "Documentos", repo: "r1", activa: true, horario: { tipo: "diario", horas: ["21:00"] }, ultima: { cuando: hace(ultima), estado: ultima > 100 ? "fallo" : "ok" } }],
    },
    ...extra,
  }) as unknown as Equipo;
const cliente = (id: string, nombre: string, equipos: Equipo[], organizacion?: unknown): PanelCliente => ({ id, nombre, rol: "administrador", marca: null, equipos, avisos_abiertos: 0, pendientes: 0, informes: [], informes_completos: true, organizacion });
const clientes = [
  cliente("c1", "Oficina Norte", [equipo("e1", "PC-01", 3)], { sedes: [{ id: "n", nombre: "Planta baja", principal: true }], equipos: {}, destinos: {}, categorias: [], orden: [] }),
  cliente("c2", "Planta Sur", [equipo("e1", "PC-01", 200), equipo("e2", "SERVIDOR", 2)]),
];

console.log("\n· Copias y equipos de todos");
const cs = copiasTodos(clientes, AHORA);
igual("una fila por copia, con su cliente", cs.map((x) => `${x.cliente.nombre}|${x.equipo.nombre}`), ["Oficina Norte|PC-01", "Planta Sur|PC-01", "Planta Sur|SERVIDOR"]);
igual("claves distintas aunque el equipo se llame igual en dos clientes", new Set(cs.map((x) => x.clave)).size, 3);
const es = equiposTodos(clientes, AHORA);
igual("lo que necesita atención primero", es[0].cliente.nombre, "Planta Sur");
igual("la sede, si el cliente tiene (la principal sin asignar)", es.find((x) => x.cliente.id === "c1")!.sede, "Planta baja");
igual("…y sin sedes, ninguna", es.find((x) => x.cliente.id === "c2" && x.equipo.id === "e2")!.sede, null);

console.log("\n· Avisos, órdenes y restauraciones");
const av: PanelDe<Aviso[]> = {
  generado: hace(0),
  omitidos: 0,
  clientes: [
    { id: "c1", nombre: "Oficina Norte", rol: "administrador", datos: [{ id: "a1", equipo: "e1", tipo: "copia_fallida", mensaje: "x", creado: hace(5), visto_por: null }] },
    { id: "c2", nombre: "Planta Sur", rol: "lectura", datos: [{ id: "a1", equipo: "e2", tipo: "copia_atrasada", mensaje: "y", creado: hace(1), visto_por: null }, { id: "a2", equipo: null, tipo: "cambio_clave", mensaje: "z", creado: hace(2), visto_por: "Ana" }] },
  ],
};
const as = avisosTodos(av, clientes);
igual("sin los ya vistos, el más reciente primero", as.map((a) => `${a.cliente.id}|${a.id}|${a.equipoNombre}`), ["c2|a1|SERVIDOR", "c1|a1|PC-01"]);
igual("con el papel de su cliente (para «Visto»)", as.map((a) => a.cliente.rol), ["administrador", "administrador"]);
const conEspera = cliente("c3", "Almacenes", [equipo("e9", "ALMACEN", 1, { resumen: { en_espera: [{ id: "o-otra", tipo: "quitar_repositorio", descripcion: "Quitar «Docs»", consola: { nombre: "Consola en línea", identidad: "x", esta: false }, emitida: hace(2), aplica: hace(-20), caduca: hace(-40) }] } as never })]);
const ords: PanelDe<Orden[]> = { generado: hace(0), omitidos: 0, clientes: [{ id: "c1", nombre: "Oficina Norte", rol: "administrador", datos: [{ id: "o1", equipo: "e1", tipo: "quitar_repositorio", not_before: hace(-2), emitida_por: { id: "u", nombre: "Ana" } } as unknown as Orden] }] };
const ws = esperaTodos([...clientes, conEspera], ords, AHORA);
igual("las de esta consola y las de otras, la que antes se aplica primero", ws.map((o) => `${o.cliente.id}|${o.id}|${o.cancelar}`), ["c1|o1|servidor", "c3|o-otra|orden"]);
const rs: PanelDe<Restauraciones> = {
  generado: hace(0),
  omitidos: 0,
  clientes: [
    { id: "c1", nombre: "Oficina Norte", rol: "administrador", datos: { ajustes: { motivo_obligatorio: false }, entradas: [{ equipo: "e1", id: "restauracion-1", hora: hace(30), repo: "Docs", resultado: "ok" }], notas: [] } },
    { id: "c2", nombre: "Planta Sur", rol: "lectura", datos: { ajustes: { motivo_obligatorio: false }, entradas: [{ equipo: "e2", id: "restauracion-1", hora: hace(3), repo: "Docs", resultado: "fallo" }], notas: [{ equipo: "e2", id: "restauracion-1", motivo: "prueba", por: "Ana", actualizada: hace(1) }] } },
  ],
};
const r = restauracionesTodos(rs, clientes);
igual("la más reciente primero, con su motivo", r.map((x) => [x.cliente.id, x.equipoNombre, x.motivoFinal?.tipo ?? null]), [["c2", "SERVIDOR", "prueba"], ["c1", "PC-01", null]]);
igual("un servidor sin nada: vacío", [avisosTodos(null, clientes), restauracionesTodos(null, clientes)], [[], []]);

console.log(`\n${total - fallos} de ${total} bien.`);
if (fallos) process.exit(1);
