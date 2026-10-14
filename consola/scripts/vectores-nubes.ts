// Pruebas de «Nubes conectadas» en la página de cada equipo (src/lib/nubesEquipo.ts,
// plan 0.7.26, 1.1; docs/destinos.md «Desconectar una nube»).
// `npm run test:vectores` (con las demás).
import type { Equipo } from "../src/lib/tipos";
import { admiteRevocar, motivoNoDesconectar, nubesDelEquipo, nubesPorAnular, queHaceAlDesconectar, usosDeNube } from "../src/lib/nubesEquipo";

let fallos = 0;
let total = 0;
function igual(nombre: string, obtenido: unknown, esperado: unknown) {
  total++;
  const ok = JSON.stringify(obtenido) === JSON.stringify(esperado);
  if (!ok) fallos++;
  console.log(`${ok ? "ok  " : "MAL "} ${nombre}${ok ? "" : `\n       obtenido: ${JSON.stringify(obtenido)}\n       esperado: ${JSON.stringify(esperado)}`}`);
}

const base = { so: "Windows 11", version_agente: "0.7.26", box_pub: "", sign_pub: "", sal_equipo: "", etiqueta: null, modo: "gestionado" as const, confirmado: true, conectado: true, ultimo_contacto: null, estado_servicio: "en_marcha" as const, siguiente_seq: 1, rol: "agente" as const };

console.log("\n· Nubes de un equipo que no es almacén y de un almacén (lib/nubesEquipo.ts)");
{
  const recepcion: Equipo = {
    ...base,
    id: "e1",
    nombre: "RECEPCION",
    resumen: {
      admite: ["nube_equipo", "repo_en_nube", "nube_revocar"],
      nubes: [
        { nombre: "Dropbox Oficina", tipo: "dropbox" },
        { nombre: "B2 Norte", tipo: "b2" },
      ],
      destinos: [
        { id: "d1", nombre: "Dropbox Oficina · Resguardo", tipo: "nube", nube: "Dropbox Oficina", donde: "Resguardo" },
        { id: "d2", nombre: "Disco D", tipo: "local" },
      ],
      repositorios: [
        { id: "r1", nombre: "Contabilidad", destino: "Dropbox Oficina · Resguardo" } as never,
        { id: "r2", nombre: "Fotos", destino: "Disco D", derivadas: [{ id: "nube", destino: "Dropbox Oficina · Resguardo", destino_id: "d1" }] } as never,
      ],
      nubes_por_anular: [{ nombre: "Dropbox Antigua", tipo: "dropbox", desde: "2026-10-07T10:00:00Z", hasta: "2026-10-14T10:00:00Z", intentos: 2 }],
    },
  };
  const l = nubesDelEquipo(recepcion);
  igual("las dos nubes, también sin ser almacén", l.map((n) => [n.nombre, n.tipo]), [["Dropbox Oficina", "dropbox"], ["B2 Norte", "b2"]]);
  igual("qué usa la Dropbox: el repositorio y la derivada", l[0].usos, ["el repositorio «Contabilidad»", "una copia derivada de «Fotos»"]);
  igual("B2 sin uso", l[1].usos, []);
  igual("agente con nube_revocar", admiteRevocar(recepcion), true);
  igual("usada: no se ofrece desconectar y se dice por qué", motivoNoDesconectar(recepcion, l[0]), "La usa el repositorio «Contabilidad», una copia derivada de «Fotos». Quítala antes de ahí para poder desconectarla.");
  igual("sin uso: se puede", motivoNoDesconectar(recepcion, l[1]), null);
  igual("Dropbox: anula el permiso de este equipo y reintenta", queHaceAlDesconectar(recepcion, "dropbox").includes("anula ese permiso en Dropbox (solo el de este equipo)"), true);
  igual("B2: la clave sigue valiendo", queHaceAlDesconectar(recepcion, "b2").includes("siguen valiendo en el proveedor"), true);
  igual("las pendientes de anular", nubesPorAnular(recepcion).map((n) => n.nombre), ["Dropbox Antigua"]);

  const almacen: Equipo = {
    ...base,
    id: "e2",
    nombre: "SERVIDOR-APPS",
    resumen: {
      admite: ["espejo_flexible"],
      nubes: [{ nombre: "Dropbox Oficina", tipo: "dropbox" }],
      guarda_copias: {
        activo: true,
        nubes: [{ nombre: "Dropbox Oficina", tipo: "dropbox" }, { nombre: "Drive", tipo: "drive" }],
        espejo: { hora: "02:00", destinos: [{ tipo: "nube", nube: "Dropbox Oficina", carpeta: "Sur" }] },
      },
    },
  };
  const a = nubesDelEquipo(almacen);
  igual("una vez cada una (resumen.nubes y guarda_copias.nubes)", a.map((n) => n.nombre), ["Dropbox Oficina", "Drive"]);
  igual("la usa el espejo", usosDeNube(almacen, "Dropbox Oficina"), ["el espejo de este almacén"]);
  igual("agente anterior: no se bloquea en la consola (decide él)", motivoNoDesconectar(almacen, a[0]), null);
  igual(
    "agente anterior con Dropbox: el permiso sigue vivo",
    queHaceAlDesconectar(almacen, "dropbox"),
    "Este equipo tiene un agente anterior: solo olvida el permiso. El permiso sigue vivo en Dropbox: quítalo desde la web de Dropbox → Aplicaciones conectadas (ojo: desconecta todos los equipos).",
  );
  igual("sin resumen: nada", [nubesDelEquipo(null), nubesPorAnular(undefined), usosDeNube(null, "x")], [[], [], []]);

  // Plan 0.8.0 (B2): con trabajos de espejo, cada uno por su nombre (también en pausa), y bloquea
  // aunque el resumen de los destinos de antes no lo traiga (solo lleva los activos).
  const trabajo = (id: string, nombre: string, nube: string, activo: boolean, quien: "almacen" | "equipo" = "almacen") => ({
    id, nombre, activo, quien, que: { tipo: "todos" as const }, adonde: { tipo: "nube" as const, nube, carpeta: "mi-bucket/Resguardo" },
    cuando: { tras_copia: true }, retencion: { modo: "nunca" as const }, freno: { pct: 10, min_archivos: 100, min_faltan: 20, accion: "confirmar" as const }, orden: 0,
  });
  const conTrabajos: Equipo = {
    ...almacen,
    resumen: {
      admite: ["espejo_flexible", "espejo_trabajos"],
      guarda_copias: {
        activo: true,
        nubes: [{ nombre: "B2 Oficina", tipo: "b2" }, { nombre: "Dropbox Oficina", tipo: "dropbox" }],
        espejo: { hora: "02:00", destinos: [], trabajos: [trabajo("t1", "Ventas a B2", "B2 Oficina", false), trabajo("t2", "Todo a B2", "B2 Oficina", true)] },
      },
      espejo_equipo: { trabajos: [trabajo("t3", "Discos a Dropbox", "Dropbox Oficina", true, "equipo")] },
    },
  };
  const t = nubesDelEquipo(conTrabajos);
  igual("los espejos por su nombre, también en pausa", usosDeNube(conTrabajos, "B2 Oficina"), ["el espejo «Ventas a B2» (en pausa)", "el espejo «Todo a B2»"]);
  igual("el espejo del propio equipo", usosDeNube(conTrabajos, "Dropbox Oficina"), ["el espejo «Discos a Dropbox» de este equipo"]);
  igual(
    "con trabajos, la consola no deja desconectarla (aunque el agente no diga nube_revocar)",
    motivoNoDesconectar(conTrabajos, t[0]),
    "La usa el espejo «Ventas a B2» (en pausa), el espejo «Todo a B2». Quítala antes de ahí para poder desconectarla.",
  );
}

console.log(`\n${total - fallos}/${total} bien`);
if (fallos) process.exit(1);
