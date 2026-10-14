// Pruebas de las sedes, las categorías de etiqueta, agrupar por niveles y el orden a mano
// (plan 0.8.0, D1 y D2: src/lib/sedes.ts, docs/sedes.md), y de la regla 3-2-1-1-0 de una
// copia con sedes (reglaDeCopia). `npm run test:vectores` (con las demás).
import {
  agruparPorNiveles,
  conUnaPorCategoria,
  enOrdenManual,
  equiposConVariasDe,
  equiposSinSede,
  errorSede,
  idNuevo,
  leerNiveles,
  leerOrganizacion,
  moverAntesDe,
  moverEnGrupo,
  nivelesDisponibles,
  sedeDeDestino,
  sedeDeEquipo,
  sedePorDefecto,
  type Organizacion,
} from "../src/lib/sedes";
import { reglaDeCopia, esFuera } from "../src/lib/regla321";
import { claseDe, esDeSedes, pideClave, textoAviso, pendiente } from "../src/lib/datosComunes";
import type { Equipo } from "../src/lib/tipos";

let fallos = 0;
let total = 0;
function igual(nombre: string, obtenido: unknown, esperado: unknown) {
  total++;
  const ok = JSON.stringify(obtenido) === JSON.stringify(esperado);
  if (!ok) fallos++;
  console.log(`${ok ? "ok  " : "MAL "} ${nombre}${ok ? "" : `\n       obtenido: ${JSON.stringify(obtenido)}\n       esperado: ${JSON.stringify(esperado)}`}`);
}

const org: Organizacion = leerOrganizacion({
  sedes: [
    { id: "sur", nombre: "Planta Sur", notas: null, principal: false },
    { id: "norte", nombre: "Oficina Norte", notas: "Planta baja", principal: true },
  ],
  equipos: { pc1: "sur", alm: "norte" },
  destinos: { "zona:alm:principal": "norte", srv: "sur" },
  categorias: [{ id: "tipo", nombre: "Tipo", etiquetas: ["Servidor", "Puesto"] }],
  orden: ["pc2", "pc1"],
});

console.log("\n· Sedes");
igual("por defecto, la principal", sedePorDefecto(org)?.id, "norte");
igual("sin principal, la primera por nombre", sedePorDefecto({ ...org, sedes: org.sedes.map((s) => ({ ...s, principal: false })) })?.id, "norte");
igual("sin sedes, ninguna", sedePorDefecto(leerOrganizacion(null)), null);
igual("un equipo asignado", [sedeDeEquipo(org, "pc1").sede?.id, sedeDeEquipo(org, "pc1").asignada], ["sur", true]);
igual("uno sin asignar: la de por defecto (y avisar)", [sedeDeEquipo(org, "pc2").sede?.id, sedeDeEquipo(org, "pc2").asignada], ["norte", false]);
igual("equipos sin sede", equiposSinSede(org, [{ id: "pc1" }, { id: "pc2" }]).map((e) => e.id), ["pc2"]);
igual("sin sedes en el cliente, nadie «sin sede»", equiposSinSede(leerOrganizacion(null), [{ id: "pc2" }]), []);
igual("destino asignado", sedeDeDestino(org, { clave: "srv" }).sede?.id, "sur");
igual("destino de un equipo sin asignar: la de su equipo", sedeDeDestino(org, { clave: "zona:x:principal", equipo: "pc1" }).sede?.id, "sur");
igual("…salvo que una persona marcara su tipo", sedeDeDestino(org, { clave: "x", equipo: "pc1", tipoMarcado: true }).sede, null);
igual("un servidor de fuera sin asignar: ninguna (manda su tipo)", sedeDeDestino(org, { clave: "otro" }).sede, null);
igual("la nube no es sede", sedeDeDestino(org, { clave: "srv", nube: true }).sede, null);
igual("id nuevo legible", idNuevo("Oficina Norte (sótano)", () => 0.5), "oficina-norte-sotano-8000");
igual("validar la sede", [errorSede("", ""), errorSede("Norte", ""), errorSede("a".repeat(61), ""), errorSede("Norte", "x".repeat(501)), errorSede("Norte", "dos\nlíneas")], [
  "El nombre: de 1 a 60 caracteres.",
  null,
  "El nombre: de 1 a 60 caracteres.",
  "Las notas: hasta 500 caracteres.",
  null,
]);

console.log("\n· Datos comunes (las claves, como el agente)");
igual("claves", ["sede:norte", "sede.equipo:pc1", "sede.destino:zona:a:principal", "etiqueta.categoria:tipo", "orden.equipos:todos", "orden.equipos:x"].map((k) => !!claseDe(k)), [true, true, true, true, true, false]);
igual("la sede de un equipo o destino pide la clave; la lista de sedes no", ["sede:norte", "sede.equipo:pc1", "sede.destino:d", "etiqueta.categoria:t"].map(pideClave), [false, true, true, false]);
igual("solo a agentes que las guardan", ["sede:norte", "etiqueta.color:x", "orden.equipos:todos"].map(esDeSedes), [true, false, true]);
igual(
  "el aviso de lo que pide la clave",
  textoAviso(
    pendiente({
      filas: [
        { clave: "sede.equipo:pc1", valor: { sede: "sur" }, cambiado: "2026-10-14T10:00:00Z", consola: null, esta: true, por: null, semilla: false, estado: "aplicado", local: null, por_enviar: true },
        { clave: "destino.regla:d", valor: { clase: "b2", atributos: { tipo: "nube" } }, cambiado: "2026-10-14T10:00:00Z", consola: null, esta: true, por: null, semilla: false, estado: "aplicado", local: null, por_enviar: true },
      ],
      sin_compartir: [],
    }),
  ),
  "1 tipo de destino por repartir (pide la clave) · 1 sede de un equipo o destino por repartir (pide la clave)",
);

console.log("\n· Categorías");
igual("un solo valor por categoría al añadir", conUnaPorCategoria(org, ["Servidor", "Contabilidad"], "puesto"), { etiquetas: ["Contabilidad", "puesto"], quitadas: ["Servidor"] });
igual("una suelta no quita nada", conUnaPorCategoria(org, ["Servidor"], "Contabilidad"), { etiquetas: ["Servidor", "Contabilidad"], quitadas: [] });
igual("la que ya tiene no se repite", conUnaPorCategoria(org, ["Servidor"], "servidor").etiquetas, ["Servidor"]);
igual("equipos con dos de la misma (de antes)", equiposConVariasDe(org.categorias[0], [{ id: "a", etiquetas: ["Servidor", "Puesto"] }, { id: "b", etiquetas: ["Servidor"] }]).map((e) => e.id), ["a"]);

console.log("\n· Agrupar por niveles, sin duplicados");
const eq = (id: string, nombre: string, etiquetas: string[], so = "Windows 11"): Equipo => ({ id, nombre, etiquetas, so }) as unknown as Equipo;
const equipos = [eq("pc1", "PC-01", ["Puesto", "Contabilidad", "Ventas"]), eq("pc2", "PC-02", ["Servidor"]), eq("alm", "ALMACEN", ["Servidor"], "Debian 12"), eq("pc3", "PC-03", [])];
const ctx = { organizacion: org, sistema: (e: Equipo) => e.so };
const disp = nivelesDisponibles(org, true);
igual("niveles que se ofrecen", disp.map((d) => d.nivel), ["sede", "categoria:tipo", "sistema", "etiqueta"]);
igual("lo recordado, sin lo que ya no existe ni repetidos", leerNiveles("sede>sede>categoria:vieja>categoria:tipo>sistema", disp), ["sede", "categoria:tipo"]);
const g = agruparPorNiveles(equipos, ["sede", "categoria:tipo"], ctx);
igual("primer nivel: por sede (los sin asignar, en la de por defecto)", g.map((x) => [x.titulo, x.equipos.map((e) => e.id)]), [["Oficina Norte", ["pc2", "alm", "pc3"]], ["Planta Sur", ["pc1"]]]);
igual("segundo nivel: por Tipo, con «Sin Tipo» al final", g[0].hijos.map((x) => [x.titulo, x.sin, x.equipos.length]), [["Servidor", false, 2], ["Sin Tipo", true, 1]]);
igual("claves únicas para plegar", g[0].hijos.map((x) => x.clave), ["sede=Oficina Norte/categoria:tipo=Servidor", "sede=Oficina Norte/categoria:tipo="]);
const porEtiqueta = agruparPorNiveles(equipos, ["etiqueta"], ctx);
igual("por etiquetas sueltas: cada equipo una sola vez", porEtiqueta.map((x) => [x.titulo, x.equipos.map((e) => e.id)]), [["Contabilidad · Ventas", ["pc1"]], ["Sin etiqueta", ["pc2", "alm", "pc3"]]]);
igual("cada equipo, en un solo grupo", porEtiqueta.flatMap((x) => x.equipos).length, equipos.length);
igual("por sistema", agruparPorNiveles(equipos, ["sistema"], ctx).map((x) => x.titulo), ["Debian 12", "Windows 11"]);
igual("sin sedes no hay «Sede»", nivelesDisponibles(leerOrganizacion(null), false).map((d) => d.nivel), ["etiqueta"]);

console.log("\n· Orden a mano");
igual("en el orden guardado; los demás, al final por nombre", enOrdenManual(equipos, org.orden).map((e) => e.id), ["pc2", "pc1", "alm", "pc3"]);
const todos = ["pc2", "pc1", "alm", "pc3"];
igual("subir dentro de un grupo deja a los demás donde estaban", moverEnGrupo(todos, ["pc2", "alm", "pc3"], "alm", -1), ["alm", "pc1", "pc2", "pc3"]);
igual("el primero no sube", moverEnGrupo(todos, ["pc2", "alm"], "pc2", -1), null);
igual("arrastrar antes de otro", moverAntesDe(todos, ["pc2", "alm", "pc3"], "pc3", "pc2"), ["pc3", "pc1", "pc2", "alm"]);
igual("arrastrar al final", moverAntesDe(todos, ["pc2", "alm", "pc3"], "pc2", null), ["alm", "pc1", "pc3", "pc2"]);
igual("soltar en su sitio: nada", moverAntesDe(todos, ["pc2", "alm", "pc3"], "pc2", "alm"), null);

console.log("\n· La regla de una copia con sedes (14ll)");
{
  const ahora = Date.parse("2026-10-14T12:00:00Z");
  const hace = (h: number) => new Date(ahora - h * 3600_000).toISOString();
  const almacen = {
    id: "alm",
    nombre: "ALMACEN",
    so: "linux",
    confirmado: true,
    modo: "gestionado",
    rol: "almacenamiento",
    resumen: { guarda_copias: { activo: true, carpeta: "/srv/copias", puerto: 8000, usuarios: [{ usuario: "pc1" }] } },
  } as unknown as Equipo;
  const pc = (id: string) =>
    ({
      id,
      nombre: id.toUpperCase(),
      so: "windows",
      confirmado: true,
      modo: "gestionado",
      resumen: {
        destinos: [{ id: "d1", nombre: "ALMACEN", tipo: "rest", donde: "https://almacen:8000/", equipo_almacen: "alm" }],
        repositorios: [{ id: "r1", nombre: "Docs", destino: "ALMACEN", destino_id: "d1" }],
        copias: [{ id: "k1", nombre: "Docs", repo: "r1", activa: true, horario: { tipo: "diario", horas: ["21:00"] }, ultima: { cuando: hace(3), estado: "ok" } }],
      },
    }) as unknown as Equipo;
  const planta = pc("pc1");
  const oficina = pc("pc2");
  const es = [almacen, planta, oficina];
  const fueraDe = (e: Equipo, o: Organizacion | null) => {
    const rc = reglaDeCopia(e, e.resumen!.copias![0], es, null, [], ahora, o);
    return rc ? [rc.entrada.origen.sede ?? null, rc.pasos[0].sede ?? null, rc.regla.partes.find((p) => p.id === "fuera")!.valor] : null;
  };
  const o2: Organizacion = { ...org, destinos: {} };
  igual("sin sedes: el almacén de la oficina es Local, como antes", fueraDe(planta, null), [null, null, 0]);
  igual("el equipo de la planta copia al almacén de la oficina: fuera", fueraDe(planta, o2), ["sur", "norte", 1]);
  igual("el de la oficina, al mismo almacén: en el sitio", fueraDe(oficina, o2), ["norte", "norte", 0]);
  igual("esFuera sin sede del destino: su tipo", [esFuera({ tipo_destino: "local", sede: null }, { sede: "sur" }), esFuera({ tipo_destino: "fuera" }, { sede: "sur" })], [false, true]);
}

console.log(`\n${total - fallos} de ${total} bien.`);
if (fallos) process.exit(1);
