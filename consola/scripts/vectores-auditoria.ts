// Pruebas de «Comprobar con un ancla» (src/lib/auditoria.ts; docs/plan-mejoras.md 9b).
// Las huellas son las mismas que en Rust (crates/servidor/src/almacen/sqlite.rs,
// `huellas_de_la_auditoria_como_en_la_consola`). `npm run test:vectores` (con las demás).
import type { EntradaAuditoria } from "../src/lib/tipos";
import { textoRehecha } from "../src/lib/historial";
import { GENESIS, anclaDe, comprobarAncla, hashEntrada, leerAncla, lineaAncla, type Ancla } from "../src/lib/auditoria";

let fallos = 0;
let total = 0;
function igual(nombre: string, obtenido: unknown, esperado: unknown) {
  total++;
  const ok = JSON.stringify(obtenido) === JSON.stringify(esperado);
  if (!ok) fallos++;
  console.log(`${ok ? "ok  " : "MAL "} ${nombre}${ok ? "" : `\n       obtenido: ${JSON.stringify(obtenido)}\n       esperado: ${JSON.stringify(esperado)}`}`);
}

const H1 = "e891152ddde045e2a42c97dce84ab501d4420a91c33548482f6b7d3b85675cde";
const H2 = "db30da97f801c28f8d8b60a40f0d77fe5f3588fbe03e42f2ebd991d7f9d4bc3e";
const CLIENTE = "cl-norte";
const fecha = (s: number) => new Date(s * 1000).toISOString();

/** Una cadena bien hecha a partir de (creado, acción, datos). */
function cadena(pasos: [number, string, string][]): EntradaAuditoria[] {
  const out: EntradaAuditoria[] = [];
  let prev = GENESIS;
  pasos.forEach(([creado, accion, datos], i) => {
    const e: EntradaAuditoria = { n: i + 1, creado: fecha(creado), actor: "cuenta:ana@ejemplo.com", accion, objetivo: CLIENTE, datos, prev_hash: prev, hash: "" };
    e.hash = hashEntrada(e);
    out.push(e);
    prev = e.hash;
  });
  return out;
}

const PASOS: [number, string, string][] = [
  [1_790_000_000, "crear_cliente", '{"nombre":"Ferretería Rambla"}'],
  [1_790_000_060, "renombrar_cliente", "{}"],
  [1_790_000_120, "invitar", '{"rol":"tecnico"}'],
  [1_790_000_180, "entrar", "{}"],
];

console.log("\n· Huellas como en el servidor");
{
  const c = cadena(PASOS);
  igual("entrada 1", c[0].hash, H1);
  igual("entrada 2", c[1].hash, H2);
}

console.log("\n· Leer el ancla de lo pegado (lib/auditoria.ts, leerAncla)");
{
  const linea = `resguardo-ancla:1:${CLIENTE}:2:1790000060:${H2}`;
  const a: Ancla = { cliente: CLIENTE, n: 2, creado: 1_790_000_060, hash: H2 };
  igual("la línea sola", leerAncla(linea), a);
  igual("y de vuelta", lineaAncla(a), linea);
  igual("en medio del correo", leerAncla(`Ancla de la actividad (n.º 2, 4 oct, 09:00): ${linea}\n\nGuarda este correo: …`), a);
  igual("partida por el correo en varias líneas", leerAncla(`resguardo-ancla:1:${CLIENTE}:2:17900\n00060:${H2.slice(0, 30)}\r\n  ${H2.slice(30)}`), a);
  igual("en mayúsculas, la huella en minúsculas", leerAncla(linea.toUpperCase().replace("RESGUARDO-ANCLA", "resguardo-ancla").replace(CLIENTE.toUpperCase(), CLIENTE))?.hash, H2);
  igual("seguida de texto que empieza por letras hexadecimales", leerAncla(`${linea}Ancla`)?.hash, H2);
  igual("sin ancla", leerAncla("Resumen semanal de copias"), null);
  igual("huella corta", leerAncla(`resguardo-ancla:1:${CLIENTE}:2:1790000060:${H2.slice(1)}`), null);
  igual("n.º 0", leerAncla(`resguardo-ancla:1:${CLIENTE}:0:1790000060:${H2}`), null);
  igual("otra versión", leerAncla(`resguardo-ancla:2:${CLIENTE}:2:1790000060:${H2}`), null);
  igual("el ancla de una entrada", anclaDe(CLIENTE, cadena(PASOS)[1]), a);
}

console.log("\n· Comprobar la cadena de hoy con un ancla (comprobarAncla)");
{
  const hoy = cadena(PASOS);
  const ancla2 = anclaDe(CLIENTE, hoy[1]);
  igual("bien: la cadena sigue y la entrada 2 es la misma", comprobarAncla(hoy, ancla2), { estado: "bien", total: 4 });
  igual("bien también con el ancla de la última", comprobarAncla([...hoy].reverse(), anclaDe(CLIENTE, hoy[3])), { estado: "bien", total: 4 });

  // El servidor rehace la cadena entera sin la entrada 2: cuadra consigo misma, pero no con el ancla.
  const rehecha = cadena([PASOS[0], PASOS[2], PASOS[3]]);
  igual("rehecha sin una entrada: otra huella en la 2", comprobarAncla(rehecha, ancla2).estado, "distinta");
  igual("rehecha y más corta que el ancla de la 4", comprobarAncla(rehecha, anclaDe(CLIENTE, hoy[3])), { estado: "falta", total: 3 });
  // Con un cambio en la entrada 1 (otro texto), todo lo de después cambia.
  const cambiada = cadena([[PASOS[0][0], PASOS[0][1], '{"nombre":"Otra"}'], ...PASOS.slice(1)]);
  igual("una entrada antigua cambiada y recalculada", comprobarAncla(cambiada, ancla2).estado, "distinta");
  // Lo que ya detecta la comprobación de siempre: un cambio sin recalcular, un hueco.
  const tocada = hoy.map((e) => ({ ...e }));
  tocada[2].datos = '{"rol":"administrador"}';
  igual("cambiada sin recalcular: rota en la 3", comprobarAncla(tocada, ancla2), { estado: "rota", en: 3 });
  igual("con un hueco: rota donde falta", comprobarAncla([hoy[0], hoy[2], hoy[3]], ancla2), { estado: "rota", en: 2 });
  igual("la primera sin génesis", comprobarAncla(cadena(PASOS).slice(1), ancla2), { estado: "rota", en: 1 });
  igual("sin entradas", comprobarAncla([], ancla2), { estado: "falta", total: 0 });
}

console.log("\n· Lo que cuenta la Historia del equipo (auditoria_rehecha)");
igual("retrocede", textoRehecha({ n: 41, hash: H1 }, { n: 39, hash: H2 }).startsWith("Antes llegaba a la entrada n.º 41 y ahora solo a la 39."), true);
igual("otra huella", textoRehecha({ n: 2, hash: H1 }, { n: 2, hash: H2 }).startsWith("La entrada n.º 2 tenía la huella e891152ddde0… y ahora tiene db30da97f801…"), true);

console.log(`\n${total - fallos}/${total} bien`);
if (fallos) process.exit(1);
