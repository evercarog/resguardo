// Pruebas de las órdenes en espera de todas las consolas (src/lib/espera.ts, v1.49,
// docs/consolas-multiples.md §5): juntar las de esta consola y las que el equipo tiene
// de otras, sin repetir ni enseñar direcciones, y el historial «Desde otras consolas».
// `npm run test:vectores` (con las demás).
import type { EntradaHistorial, Equipo, Orden, OrdenEnEspera } from "../src/lib/tipos";
import { admiteCancelarEspera, filasEnEspera, nombreConsola, ordenesDeOtras, porQueNoSeAplico, resultadoOrden, sePuedeVolverAMandar } from "../src/lib/espera";
import { NIVEL, sellarOrden } from "../src/lib/cripto/ordenes";
import { abrirB64, parEfimero } from "../src/lib/cripto/sobre";
import { aB64 } from "../src/lib/cripto/bytes";

let fallos = 0;
let total = 0;
function igual(nombre: string, obtenido: unknown, esperado: unknown) {
  total++;
  const ok = JSON.stringify(obtenido) === JSON.stringify(esperado);
  if (!ok) fallos++;
  console.log(`${ok ? "ok  " : "MAL "} ${nombre}${ok ? "" : `\n       obtenido: ${JSON.stringify(obtenido)}\n       esperado: ${JSON.stringify(esperado)}`}`);
}
const cierto = (nombre: string, v: boolean) => igual(nombre, v, true);

const AHORA = Date.parse("2026-10-06T10:00:00Z");
const en = (h: number) => new Date(AHORA + h * 3600_000).toISOString();
const base = { so: "Windows 11", version_agente: "0.7.22", box_pub: "", sign_pub: "", sal_equipo: "", etiqueta: null, modo: "gestionado" as const, confirmado: true, conectado: true, ultimo_contacto: null, estado_servicio: "en_marcha" as const, siguiente_seq: 1, rol: "agente" as const };
const espera = (id: string, consola: OrdenEnEspera["consola"], h: number, extra: Partial<OrdenEnEspera> = {}): OrdenEnEspera => ({
  id,
  tipo: "quitar_repositorio",
  descripcion: "Quitar el repositorio «Facturas»",
  consola,
  por: "Ana",
  emitida: en(0),
  aplica: en(h),
  caduca: en(h + 24),
  ...extra,
});
const ID_ESTA = "SWRlbnRpZGFkIGRlIGVzdGE=";
const ID_OTRA = "SWRlbnRpZGFkIGRlIGxhIG90cmE=";
const equipo: Equipo = {
  ...base,
  id: "e1",
  nombre: "CAJA-1",
  resumen: {
    admite: ["consolas_multiples", "ordenes_en_espera"],
    consolas: [
      { id: "principal", nombre: "Oficina", url: "https://192.168.1.20:8443", identidad: ID_ESTA, sal_cliente: null, ultimo_contacto: null, desde: null, esta: true },
      { id: "b", nombre: "consola.ejemplo.com", url: "https://consola.ejemplo.com", identidad: ID_OTRA, sal_cliente: null, ultimo_contacto: null, desde: null, esta: false },
    ],
    en_espera: [
      espera("o-propia", { nombre: "Oficina", identidad: ID_ESTA, esta: true }, 20),
      espera("o-otra", { nombre: null, identidad: ID_OTRA, esta: false }, 2, { tipo: "pausar", descripcion: "Pausar las copias", por: "Bruno" }),
      espera("o-caducada", { nombre: "En línea", identidad: ID_OTRA, esta: false }, -30),
    ],
  },
};
const viejo: Equipo = { ...base, id: "e2", nombre: "CAJA-2", resumen: { admite: ["consolas_multiples"] } };
const propia: Orden = {
  id: "o-propia",
  equipo: "e1",
  tipo: "quitar_repositorio",
  seq: 7,
  emitida: en(0),
  emitida_por: { id: "u1", nombre: "Ana" },
  not_before: en(20),
  caduca: en(44),
  estado: "entregada",
  mensaje: null,
  detalle: null,
  firma_agente: null,
  actualizada: en(0),
};

console.log("\n· Órdenes esperando su turno, de todas las consolas (lib/espera.ts)");
const filas = filasEnEspera([propia], [equipo, viejo], AHORA);
igual("las dos que siguen esperando, sin repetir la propia ni la caducada, por orden de aplicación", filas.map((f) => f.id), ["o-otra", "o-propia"]);
igual("la de otra consola: desde cuál (sin dirección: su nombre parece una), quién y cómo se cancela", [filas[0].otraConsola, filas[0].por, filas[0].cancelar, filas[0].descripcion], ["otra consola", "Bruno", "orden", "Pausar las copias"]);
igual("la propia: de su servidor, con la descripción que da el equipo", [filas[1].otraConsola, filas[1].cancelar, filas[1].descripcion, filas[1].equipoNombre], [null, "servidor", "Quitar el repositorio «Facturas»", "CAJA-1"]);
cierto("ninguna dirección de consola en la lista", !JSON.stringify(filas).includes("192.168") && !JSON.stringify(filas).includes("ejemplo.com"));
igual("nombre de la consola: el suyo en el equipo", nombreConsola(equipo, { nombre: "En línea", identidad: ID_OTRA, esta: false }), "En línea");
const conNombre: Equipo = { ...equipo, resumen: { ...equipo.resumen, consolas: equipo.resumen!.consolas!.map((c) => (c.esta ? c : { ...c, nombre: "Consola del integrador" })) } };
igual("…o el de `consolas` con esa identidad, si no es una dirección", nombreConsola(conNombre, { nombre: null, identidad: ID_OTRA, esta: false }), "Consola del integrador");
igual("cancelar las de otra consola: si el equipo lo admite", [admiteCancelarEspera(equipo), admiteCancelarEspera(viejo)], [true, false]);
igual("con un equipo o un servidor anterior (sin `en_espera`): solo las propias", filasEnEspera([propia], [viejo], AHORA).map((f) => [f.id, f.cancelar]), [["o-propia", "servidor"]]);

console.log("\n· «Desde otras consolas» (historial común)");
const h = (id: string, identidad: string, hora: number, resultado: string): EntradaHistorial => ({ id, hora: en(hora), tipo: "orden", orden: "pausar", identidad, consola: "En línea", ...({ resultado } as object) });
const otras = ordenesDeOtras(
  [
    { equipo, historial: [h("h1", ID_OTRA, -3, "en_espera"), h("h2", ID_ESTA, -2, "hecha"), h("h3", ID_OTRA, -1, "cancelada"), { id: "h4", hora: en(-1), tipo: "copia" }] },
    { equipo: viejo, historial: [h("h1", ID_OTRA, -3, "en_espera")] },
  ],
  ID_ESTA,
);
igual("solo las de otras consolas, sin repetir, la más reciente primero", otras.map((x) => [x.id, x.equipoNombre, resultadoOrden(x)]), [["h3", "CAJA-1", "cancelada"], ["h1", "CAJA-1", "en_espera"]]);

console.log("\n· La orden nueva y quién la manda (cripto/ordenes.ts)");
igual("cancelar_espera es inofensiva (como protocolo/ordenes.rs)", NIVEL.cancelar_espera, "sesion");
const caja = parEfimero();
const p = sellarOrden({ cliente: "c", equipo: { id: "e1", box_pub: aB64(caja.publica) }, seq: 3, tipo: "cancelar_espera", cuerpo: { id: "o-otra" }, autorizacion: { prueba_admin: null, clave_repo: null }, esperaHoras: 24, por: "  Bruno  " }, new Date(AHORA));
const plana = JSON.parse(new TextDecoder().decode(abrirB64(caja.secreta, p.sellado)));
igual("sin espera, y con `por` dentro del sobre (no en lo que ve el servidor)", [p.meta.not_before, plana.por, JSON.stringify(p.meta).includes("Bruno")], [null, "Bruno", false]);
const sinPor = JSON.parse(new TextDecoder().decode(abrirB64(caja.secreta, sellarOrden({ cliente: "c", equipo: { id: "e1", box_pub: aB64(caja.publica) }, seq: 4, tipo: "copiar_ahora", cuerpo: {}, autorizacion: { prueba_admin: null, clave_repo: null }, esperaHoras: 24 }, new Date(AHORA)).sellado)));
cierto("sin nombre, sin `por` (como una consola anterior)", !("por" in sinPor));

console.log("\n· Órdenes con espera a un agente anterior (v1.58): número reservado y que no desaparezcan");
{
  const sellar = (tipo: string, cuerpo: Record<string, unknown>, seqEspera: number | null, horas = 24) =>
    sellarOrden({ cliente: "c", equipo: { id: "e1", box_pub: aB64(caja.publica) }, seq: 7, tipo, cuerpo, autorizacion: { prueba_admin: null, clave_repo: null }, esperaHoras: horas, seqEspera }, new Date(AHORA));
  const externa = sellar("cambiar_copia_externa", { repo: "docs", hora: null }, 1006);
  const dentro = JSON.parse(new TextDecoder().decode(abrirB64(caja.secreta, externa.sellado)));
  igual("quitar la copia externa: con espera, el número reservado (dentro y fuera)", [externa.meta.seq, dentro.seq], [1006, 1006]);
  igual("y 72 h para entregarla desde su hora", (Date.parse(externa.meta.caduca) - Date.parse(externa.meta.not_before!)) / 3600_000, 72);
  igual("sin espera, el número de siempre aunque haya reservado", sellar("copiar_ahora", {}, 1006).meta.seq, 7);
  const larga = sellar("pausar", { horas: 4 }, null, 120);
  cierto("nunca más de 7 días desde que se emite", Date.parse(larga.meta.caduca) - AHORA <= 7 * 24 * 3600_000);
  const o = (estado: string, extra: Partial<Orden> = {}) => ({ estado: estado as Orden["estado"], mensaje: null, detalle: null, tipo: "cambiar_copia_externa", ...extra });
  igual("caducada sin llegar al equipo: lo dice", porQueNoSeAplico(o("caducada", { motivo: "sin_entregar" }))?.startsWith("Caducó sin aplicarse: no llegó"), true);
  igual("caducada sin respuesta", porQueNoSeAplico(o("caducada", { motivo: "sin_respuesta" }))?.includes("no contestó"), true);
  igual("caducada de un servidor anterior (sin motivo)", porQueNoSeAplico(o("caducada"))?.startsWith("Caducó sin aplicarse"), true);
  igual("antigua: se explica", porQueNoSeAplico(o("rechazada", { mensaje: "Orden repetida o antigua (n.º 12 ≤ 15)." }))?.includes("orden posterior"), true);
  igual("rechazada por otra cosa: su mensaje", porQueNoSeAplico(o("rechazada", { mensaje: "La contraseña del repositorio no es correcta." })), "Rechazada: La contraseña del repositorio no es correcta.");
  igual("cancelada desde otra consola: no es un fallo", porQueNoSeAplico(o("rechazada", { detalle: '{"cancelada":true,"consola":"Oficina"}' })), null);
  igual("hecha o pendiente: nada", [porQueNoSeAplico(o("hecha")), porQueNoSeAplico(o("pendiente"))], [null, null]);
  igual(
    "«Volver a mandar» en las que no se aplicaron (no en las sesiones ni en las canceladas)",
    [sePuedeVolverAMandar(o("caducada")), sePuedeVolverAMandar(o("rechazada")), sePuedeVolverAMandar(o("hecha")), sePuedeVolverAMandar(o("caducada", { tipo: "abrir_sesion" })), sePuedeVolverAMandar(o("rechazada", { detalle: '{"cancelada":true}' }))],
    [true, true, false, false, false],
  );
}

console.log(`\n${total - fallos} de ${total} comprobaciones correctas.`);
if (fallos) process.exit(1);
