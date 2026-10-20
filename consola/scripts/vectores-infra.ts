// Pruebas del plan 0.8.1, bloque C: el tipo de equipo (C3, src/lib/tipoEquipo.ts),
// la infraestructura (C4, src/lib/infra.ts: leer, proponer, la línea de cada
// destino y la regla con los soportes reales) y lo que el agente ve de su
// almacenamiento (src/lib/datosBasicos.ts). `npm run test:vectores` (con las demás).
import type { Equipo } from "../src/lib/tipos";
import { cuerpoTipo, grupoTipo, textoTipo, tipoEquipo } from "../src/lib/tipoEquipo";
import { fisicoDe, formaConjunto, idDe, leerInfra, lineaFisica, paraGuardar, propuestas, sitioDeDestino, sitioDeEquipo, textoConjunto, tipoDeConjunto } from "../src/lib/infra";
import { datosBasicos, discoDeRuta } from "../src/lib/datosBasicos";
import { leerOrganizacion } from "../src/lib/sedes";
import { ponerInfra, type PasoVista } from "../src/lib/regla321";

let fallos = 0;
let total = 0;
function igual(nombre: string, obtenido: unknown, esperado: unknown) {
  total++;
  const ok = JSON.stringify(obtenido) === JSON.stringify(esperado);
  if (!ok) fallos++;
  console.log(`${ok ? "ok  " : "MAL "} ${nombre}${ok ? "" : `\n       obtenido: ${JSON.stringify(obtenido)}\n       esperado: ${JSON.stringify(esperado)}`}`);
}
const cierto = (nombre: string, v: boolean) => igual(nombre, v, true);
/** Sin los espacios finos de las cifras («2 TB»), para comparar. */
const sinFinos = (t: string | undefined) => t?.replace(/[   ]/g, " ");

const base = { so: "Linux x86_64", version_agente: "0.8.1", box_pub: "", sign_pub: "", sal_equipo: "", etiqueta: null, modo: "gestionado" as const, confirmado: true, conectado: true, ultimo_contacto: null, estado_servicio: null, siguiente_seq: 1, rol: "agente" as const };
const eq = (id: string, nombre: string, resumen: Equipo["resumen"]): Equipo => ({ ...base, id, nombre, resumen });

console.log("\n· Tipo de equipo (C3)");
const fisico = eq("e1", "SRV-01", { datos_basicos: { fabricante: "Marca X", modelo: "Torre 1" } as never });
const vm = eq("e2", "VM-01", { entorno: { virtual: "hyperv" }, datos_basicos: {} as never });
const ct = eq("e3", "CT-01", { entorno: { contenedor: "lxc", virtual: "kvm" } });
const viejo = eq("e4", "PC-VIEJO", { copias: [] });
const aMano = eq("e5", "PC-05", { entorno: { virtual: "kvm" }, datos_equipo: { tipo: { valor: { clase: "fisico" }, cuando: "2026-10-01T00:00:00Z" } } });
igual(
  "detectado: físico (datos básicos sin entorno), máquina virtual, contenedor (manda sobre la VM), sin datos",
  [fisico, vm, ct, viejo].map((e) => [tipoEquipo(e).clase, tipoEquipo(e).plataforma, tipoEquipo(e).origen]),
  [
    ["fisico", null, "detectado"],
    ["virtual", "hyperv", "detectado"],
    ["contenedor", "lxc", "detectado"],
    ["desconocido", null, "sin_datos"],
  ],
);
igual("corregido a mano: manda sobre lo detectado", [tipoEquipo(aMano).clase, tipoEquipo(aMano).origen], ["fisico", "a_mano"]);
igual("en palabras", [textoTipo(tipoEquipo(vm)), textoTipo(tipoEquipo(ct)), textoTipo(tipoEquipo(fisico))], ["Máquina virtual · Hyper-V", "Contenedor · LXC", "Equipo físico"]);
igual("para agrupar y filtrar", [grupoTipo(vm), grupoTipo(viejo)], ["Máquina virtual (Hyper-V)", "Tipo sin saber"]);
igual("el cuerpo de la orden: plataforma solo si es de su clase", [cuerpoTipo("virtual", "vmware"), cuerpoTipo("fisico", "kvm"), cuerpoTipo("contenedor", "kvm"), cuerpoTipo("", "")], [{ clase: "virtual", plataforma: "vmware" }, { clase: "fisico" }, { clase: "contenedor" }, { clase: "" }]);

console.log("\n· Lo que el agente ve de su almacenamiento (datos básicos)");
const alm = eq("alm", "ALMACEN-01", {
  guarda_copias: { activo: true, carpeta: "/tank/copias", zonas: [{ id: "z0a0b0c", nombre: "Disco USB", carpeta: "/mnt/usb/copias", puerto: 8002, usuarios: 1 }] },
  datos_basicos: {
    discos: [
      { punto: "/", total: 100e9, libre: 50e9, sistema_archivos: "ext4" },
      { punto: "/tank", total: 1.2e12, libre: 0.42e12, sistema_archivos: "zfs", volumen: "tank" },
      { punto: "/mnt/usb", total: 2e12, libre: 1e12, sistema_archivos: "ext4", volumen: 5 },
    ],
    almacenamiento: {
      discos: [{ id: "sda", total: 1.2e12, clase: "hdd", modelo: "Disco X", serial: "NO" }, { id: "x" }],
      conjuntos: [{ id: "tank", tipo: "zfs", estado: "bien", total: 1.2e12 }, { id: "md0", tipo: "mdadm", forma: "raid1", discos: 2, estado: "roto" }],
    },
  } as never,
});
const db = datosBasicos(alm)!;
igual("el conjunto de cada disco (lo que no es texto, fuera)", db.discos!.map((d) => d.volumen), [null, "tank", null]);
igual("discos y conjuntos comprobados (sin campos de más)", [db.almacenamiento!.discos.length, Object.keys(db.almacenamiento!.discos[0]).includes("serial"), db.almacenamiento!.conjuntos.map((c) => [c.id, c.estado])], [1, false, [["tank", "bien"], ["md0", null]]]);
igual("el disco de una ruta: el punto más largo que la contiene", [discoDeRuta(db, "/tank/copias/pc-01")?.punto, discoDeRuta(db, "/srv")?.punto, discoDeRuta(db, "/tankx")?.punto], ["/tank", "/", "/"]);
igual("en Windows, por la letra", discoDeRuta({ discos: [{ punto: "D:", total: 1, libre: 0 }] }, "d:\\Copias")?.punto, "D:");

console.log("\n· Infraestructura (C4): leer, proponer y la línea de cada destino");
const infra = leerInfra({
  servidores: [
    {
      id: "hv-01",
      nombre: "hv-01",
      hipervisor: "proxmox",
      sede: "central",
      conjuntos: [{ id: "tank", nombre: "tank", tipo: "zfs_espejo", discos: 2, capacidad: 1.2e12, estado: "bien" }, { id: "x", nombre: "", tipo: "raid1" }, { id: "y", nombre: "raro", tipo: "raid99" }],
    },
    { id: "HV 2", nombre: "mal" },
    { id: "nas-01", nombre: "nas-01", conjuntos: [] },
  ],
  equipos: { e2: { servidor: "hv-01", conjunto: "nada" }, e9: { servidor: "no-existe" } },
  destinos: { "zona:alm:principal": { servidor: "hv-01", conjunto: "tank" } },
  por: "Ana",
});
igual("lo que no cuadra, fuera (ids, tipos, referencias)", [infra.servidores.map((s) => s.id), infra.servidores[0].conjuntos.map((c) => c.id), infra.equipos], [["hv-01", "nas-01"], ["tank"], { e2: { servidor: "hv-01" } }]);
const org = leerOrganizacion({ sedes: [{ id: "central", nombre: "Oficina central", principal: true }] });
const sz = sitioDeDestino(infra, "zona:alm:principal", "alm");
igual("una línea por destino", lineaFisica(sz, org), "hv-01 · tank, espejo de 2 discos · Oficina central");
igual("un destino sin anotar: el servidor de quien lo guarda, sin conjunto", lineaFisica(sitioDeDestino(infra, "zona:alm:z0a0b0c", "e2"), org), "hv-01 · Oficina central");
igual("sin nada: nada", [sitioDeDestino(infra, "otro", null), sitioDeEquipo(infra, "e1"), lineaFisica(null)], [null, null, null]);
igual("formas", [formaConjunto({ tipo: "raid5", discos: 4 }), formaConjunto({ tipo: "disco", discos: 1 }), textoConjunto({ id: "s", nombre: "RAID 1", tipo: "raid1" })], ["RAID 5 de 4 discos", "disco suelto", "RAID 1"]);
igual("lo que ve el agente → el tipo de la consola", [tipoDeConjunto({ id: "md0", tipo: "mdadm", forma: "raid10" }), tipoDeConjunto({ id: "t", tipo: "zfs" }), tipoDeConjunto({ id: "d1", tipo: "storage_spaces" })], ["raid10", "otro", "storage_spaces"]);
igual("ids cortos y sin repetir", [idDe("HV-01 (sala)"), idDe("hv-01", ["hv-01"]), idDe("Ñandú")], ["hv-01-sala", "hv-01-2", "nandu"]);
const g = paraGuardar(infra);
cierto("para guardar: sin nulos, sin quién ni cuándo", !JSON.stringify(g).includes("null") && !("por" in g));
const fis = eq("srv", "SRV-02", { datos_basicos: { fabricante: "Marca Y", almacenamiento: { discos: [{ id: "sda", total: 1e12 }, { id: "sdb", total: 1e12 }], conjuntos: [{ id: "md0", tipo: "mdadm", forma: "raid1", discos: 2 }] } } as never });
const ps = propuestas([alm, vm, fis, fisico], infra);
igual(
  "lo que se propone (el almacén, sin entorno y con datos básicos, es un equipo físico)",
  ps.map((p) => [p.id, p.clase]),
  [
    ["servidor:alm", "servidor"],
    ["zona:zona:alm:z0a0b0c", "zona"],
    ["servidor:srv", "servidor"],
  ],
);
cierto("un puesto físico sin conjuntos ni varios discos no se propone (se anota a mano)", !ps.some((p) => p.equipo.id === "e1"));
igual("la frase de una zona: dónde guarda, cuánto y cuánto usado", sinFinos(ps[1].frase), "El almacén ALMACEN-01 (Disco USB) guarda en el disco /mnt/usb (ext4) (2 TB, 50 % usado). ¿En qué servidor está?");
igual("la de un equipo físico", ps[0].frase, "ALMACEN-01 es un equipo físico con 1 disco y 2 conjuntos. ¿Lo anotas como servidor?");
igual("un equipo físico: su servidor ya armado con lo que ve", [ps[2].servidor?.nombre, ps[2].servidor?.marca, ps[2].servidor?.conjuntos.map((c) => [c.nombre, c.tipo, c.discos])], ["SRV-02", "Marca Y", [["md0", "raid1", 2]]]);
cierto("una máquina virtual ya anotada no se propone", !ps.some((p) => p.equipo.id === "e2"));
const sinInfra = propuestas([alm, vm], null);
igual("sin nada anotado: también la zona principal (ZFS «tank») y la máquina virtual", sinInfra.map((p) => sinFinos(p.frase)).slice(1), [
  "El almacén ALMACEN-01 guarda en ZFS «tank» (1,2 TB, 65 % usado). ¿En qué servidor está?",
  "El almacén ALMACEN-01 (Disco USB) guarda en el disco /mnt/usb (ext4) (2 TB, 50 % usado). ¿En qué servidor está?",
  "VM-01 es una máquina virtual. ¿En qué servidor físico está?",
]);

console.log("\n· La regla 3-2-1-1-0 con los soportes reales (ponerInfra)");
const paso = (id: string, soporte: string, equipo: string | null, clave: string | null): PasoVista => ({ id, soporte, equipo, clave, nombre: id, tipo: "copia", lugar: "oficina", inmutable: "no", marcado: null } as unknown as PasoVista);
const pasos = [paso("destino", "equipo:alm:principal", "alm", "zona:alm:principal"), paso("local", "equipo:e2:origen", "e2", "d-local"), paso("nube", "nube:b2:x", null, "d-b2")];
const o = ponerInfra("e2", pasos, infra, org);
igual("los originales: su servidor (y sin conjunto, su soporte de siempre)", o, { soporte: "equipo:e2:origen", fisico: "hv-01" });
igual(
  "cada paso: el servidor físico y el soporte real si se sabe el conjunto",
  pasos.map((p) => [p.id, p.soporte, p.fisico ?? null]),
  [
    ["destino", "conjunto:hv-01:tank", "hv-01"],
    ["local", "equipo:e2:origen", "hv-01"],
    ["nube", "nube:b2:x", null],
  ],
);
igual("…con su línea para enseñarla", pasos[0].lineaFisica, "hv-01 · tank, espejo de 2 discos · Oficina central");
igual("fisicoDe", [fisicoDe(sz), fisicoDe(null)], [{ fisico: "hv-01", soporte: "conjunto:hv-01:tank" }, null]);
const intactos = [paso("destino", "equipo:alm:principal", "alm", "zona:alm:principal")];
igual("sin infraestructura, nada cambia", [ponerInfra("e2", intactos, null), intactos[0].soporte, intactos[0].fisico ?? null], [{ soporte: "equipo:e2:origen", fisico: null }, "equipo:alm:principal", null]);

console.log(`\n${total - fallos}/${total} bien`);
if (fallos) process.exit(1);
