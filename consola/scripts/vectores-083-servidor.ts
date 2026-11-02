// Pruebas de la rama `ia/083-servidor` (plan 0.8.3): 16k (sesiones de la consola) y lo
// que venga después en esta rama. `npm run test:vectores`.
import type { Equipo, SesionCuenta } from "../src/lib/tipos";
import { detalleDisco, infraValida, leerInfra, marcaDeModelo, marcaYModelo, sinAnotar, type Infraestructura } from "../src/lib/infra";
import { equiposPorGrupo, grupoInfra, hipervisoresPara, servidoresParaElegir, servidorPropuesto, textoFaltan } from "../src/lib/infraGrupos";
import { fichaSoporte, hrefSoporte } from "../src/lib/infraFicha";
import { agruparArchivos, carpetaDe, claseError, conExclusiones, leerArchivos, nombreDe, resumenRepeticion } from "../src/lib/archivosConError";
import { equiposQueFaltan, faltaEn, noLlega, otrasConsolas } from "../src/lib/consolasCliente";
import { adaptarPara } from "../src/lib/datosComunes";
import { conTopes, esMovil, frasePlazos, lineaSesion, motivoNoCerrar, nuevasSinVer, ordenarSesiones, textoSesionNueva } from "../src/lib/sesionesCuenta";

let fallos = 0;
let total = 0;
function igual(nombre: string, obtenido: unknown, esperado: unknown) {
  total++;
  const ok = JSON.stringify(obtenido) === JSON.stringify(esperado);
  if (!ok) fallos++;
  console.log(`${ok ? "ok  " : "MAL "} ${nombre}${ok ? "" : `\n       obtenido: ${JSON.stringify(obtenido)}\n       esperado: ${JSON.stringify(esperado)}`}`);
}
function cierto(nombre: string, v: boolean) {
  igual(nombre, v, true);
}

console.log("\n· 16k: sesiones de la consola");
{
  const ahora = Date.parse("2026-11-02T12:00:00Z");
  const iso = (ms: number) => new Date(ahora + ms).toISOString();
  const s = (x: Partial<SesionCuenta>): SesionCuenta => ({
    id: "a",
    actual: false,
    navegador: "Chrome · Android",
    ip: "192.0.2.10",
    creada: iso(-3 * 86_400_000),
    usada: iso(-2 * 3_600_000),
    caduca: iso(6 * 3_600_000),
    maximo: iso(20 * 3_600_000),
    recordar: false,
    nueva: false,
    ...x,
  });
  igual("móvil: Android", esMovil("Chrome · Android"), true);
  igual("móvil: iPhone", esMovil("Safari · iPhone"), true);
  igual("ordenador: Windows", esMovil("Edge · Windows"), false);

  const l = lineaSesion(s({}), ahora);
  cierto("línea: cuándo se usó", l.startsWith("Usada hace"));
  cierto("línea: desde dónde", l.includes("desde 192.0.2.10"));
  cierto("línea: sin recordar, se cierra si no se usa", l.includes("se cierra si no se usa"));
  cierto("línea: la actual se llama «Esta sesión»", lineaSesion(s({ actual: true }), ahora).startsWith("Esta sesión"));
  cierto("línea: recordada hasta…", lineaSesion(s({ recordar: true }), ahora).includes("recordada hasta el"));
  cierto("línea: sin IP conocida, no la pone", !lineaSesion(s({ ip: "?" }), ahora).includes("desde"));

  const orden = ordenarSesiones([s({ id: "vieja", usada: iso(-9 * 3_600_000) }), s({ id: "yo", actual: true, usada: iso(-1) }), s({ id: "reciente", usada: iso(-60_000) })]);
  igual("orden: la actual primero, luego la más usada", orden.map((x) => x.id), ["yo", "reciente", "vieja"]);

  igual("puede cerrar: sin motivo", motivoNoCerrar(true, null, ahora), null);
  const m = motivoNoCerrar(false, iso(5 * 3_600_000), ahora) ?? "";
  cierto("sesión nueva: dice que es nueva", m.startsWith("Esta sesión es nueva"));
  cierto("sesión nueva: dice cuánto falta", m.includes("Podrás dentro de 5"));
  cierto("sesión nueva: ofrece otro dispositivo", m.includes("dispositivo donde ya tenías la sesión abierta"));

  const nuevas = [{ id: "n1", navegador: "Firefox · Linux", creada: iso(-5 * 60_000) }, { id: "n2", navegador: "", creada: iso(-60_000) }];
  igual("aviso: sin ver", nuevasSinVer(nuevas, ["n1"]).map((n) => n.id), ["n2"]);
  igual("aviso: sin campo (servidor anterior)", nuevasSinVer(undefined, []), []);
  cierto("aviso: con el navegador", textoSesionNueva(nuevas[0], ahora).startsWith("Nueva sesión en Firefox · Linux, hace 5"));
  cierto("aviso: sin navegador conocido", textoSesionNueva(nuevas[1], ahora).startsWith("Nueva sesión en un navegador"));

  igual("topes: como el servidor", conTopes({ inactividad_horas: 0, maximo_horas: 9999, recordar_dias: 365 }), { inactividad_horas: 1, maximo_horas: 168, recordar_dias: 30 });
  igual("topes: el máximo nunca menor que la inactividad", conTopes({ inactividad_horas: 12, maximo_horas: 2, recordar_dias: 0 }), { inactividad_horas: 12, maximo_horas: 12, recordar_dias: 0 });
  igual("topes: lo que no es un número, al valor por defecto", conTopes({ inactividad_horas: Number.NaN, maximo_horas: Number.NaN, recordar_dias: Number.NaN }), { inactividad_horas: 8, maximo_horas: 24, recordar_dias: 14 });
  igual("plazos en una frase", frasePlazos({ inactividad_horas: 8, maximo_horas: 24, recordar_dias: 14 }), "Se cierra tras 8 h sin usarla y, aunque se use, a las 24 h; con «Recordar este dispositivo», hasta 14 días.");
  cierto("plazos sin recordar", frasePlazos({ inactividad_horas: 8, maximo_horas: 24, recordar_dias: 0 }).endsWith("sin «Recordar este dispositivo»."));
}

console.log("\n· 16a y 16f: marca de los discos, número de serie e inventario del servidor");
{
  igual("marca: Samsung por el modelo", marcaDeModelo("Samsung SSD 870 EVO 1TB"), "Samsung");
  igual("marca: WD por WDC", marcaDeModelo("WDC WD40EFRX-68N32N0"), "WD");
  igual("marca: Seagate por ST…", marcaDeModelo("ST4000VN008-2DR166"), "Seagate");
  igual("marca: Crucial por CT…", marcaDeModelo("CT1000MX500SSD1"), "Crucial");
  igual("marca: Toshiba", marcaDeModelo("TOSHIBA MG04ACA400E"), "Toshiba");
  igual("marca: HPE (no HP)", marcaDeModelo("HPE MB4000GVYZK"), "HPE");
  igual("marca: si no es evidente, nada", marcaDeModelo("Disco virtual 2"), null);
  igual("marca: sin modelo, nada", marcaDeModelo(null), null);
  igual("marca y modelo sin repetir", marcaYModelo("Samsung", "Samsung SSD 870 EVO"), "Samsung SSD 870 EVO");
  igual("marca y modelo juntos", marcaYModelo("WD", "Red Plus 4TB"), "WD Red Plus 4TB");
  igual("solo la marca", marcaYModelo("Dell", null), "Dell");
  igual("detalle del disco con su marca", detalleDisco({ tipo: "hdd", tamano: null, marca: "WD", modelo: "Red Plus", bahia: "2" }), "HDD · WD Red Plus · bahía 2");

  const valor = { nombre: "hv-01", serie: "ABC1234", inventario: "INV-0042", discos: [{ id: "b1", nombre: "Bahía 1", marca: "WD", serie: "SN-1" }] };
  igual("infra válida con serie, inventario y marca", infraValida("infra:servidor:hv-01", valor), { ok: true, limpio: valor });
  igual("serie de más de 64: no", infraValida("infra:servidor:hv-01", { nombre: "hv-01", serie: "x".repeat(65) }).ok, false);
  igual("marca de más de 80: no", infraValida("infra:servidor:hv-01", { nombre: "hv-01", discos: [{ id: "b1", nombre: "B", marca: "m".repeat(81) }] }).ok, false);

  const nuevo = { resumen: { admite: ["infra_comun", "infra_ficha"] } } as unknown as Equipo;
  const viejo = { resumen: { admite: ["infra_comun"] } } as unknown as Equipo;
  const x = { clave: "infra:servidor:hv-01", valor, cambiado: "2026-11-02T10:00:00Z" };
  igual("a un agente que lo admite: tal cual", adaptarPara(nuevo, x), x);
  igual("a uno anterior: sin serie, inventario ni marca", adaptarPara(viejo, x).valor, { nombre: "hv-01", discos: [{ id: "b1", nombre: "Bahía 1", serie: "SN-1" }] });
  igual("otras claves: tal cual", adaptarPara(viejo, { clave: "infra:equipo:e1", valor: { servidor: "hv-01" } }).valor, { servidor: "hv-01" });
  igual("quitado (null): tal cual", adaptarPara(viejo, { clave: "infra:servidor:hv-01", valor: null }).valor, null);
  const leido = leerInfra({ servidores: [{ id: "hv-01", nombre: "hv-01", serie: "ABC1234", inventario: "INV-0042", discos: [{ id: "b1", nombre: "Bahía 1", marca: "WD" }], conjuntos: [] }] });
  igual("leer: la serie y el inventario del servidor", [leido.servidores[0].serie, leido.servidores[0].inventario], ["ABC1234", "INV-0042"]);
  igual("leer: la marca del disco", leido.servidores[0].discos[0].marca, "WD");
}

console.log("\n· 16g: los equipos de usuario no salen «Sin anotar»");
{
  const eq = (id: string, resumen: Record<string, unknown>) => ({ id, nombre: id.toUpperCase(), confirmado: true, modo: "gestionado", resumen }) as unknown as Equipo;
  const vmw = eq("vm1", { entorno: { virtual: "vmware" } });
  const kvm = eq("vm2", { entorno: { virtual: "kvm" } });
  const ct = eq("ct1", { entorno: { contenedor: "lxc" } });
  const puesto = eq("pc1", { datos_basicos: {} });
  const srvFis = eq("hv1", { datos_basicos: {} });
  const viejo = eq("old", {});
  const infra = {
    servidores: [
      { id: "esx", nombre: "ESX-01", hipervisor: "vmware", discos: [], conjuntos: [] },
      { id: "pve", nombre: "PVE-01", hipervisor: "proxmox", discos: [], conjuntos: [] },
      { id: "nas", nombre: "NAS", hipervisor: null, discos: [], conjuntos: [] },
    ],
    equipos: { hv1: { servidor: "pve" }, vm2: { servidor: "pve" } },
    destinos: {},
  } as unknown as Infraestructura;
  igual("grupo: máquina virtual", grupoInfra(vmw, infra), "virtual");
  igual("grupo: contenedor", grupoInfra(ct, infra), "virtual");
  igual("grupo: físico de usuario", grupoInfra(puesto, infra), "usuario");
  igual("grupo: físico anotado en un servidor", grupoInfra(srvFis, infra), "servidor");
  igual("grupo: sin saber el tipo, como de usuario", grupoInfra(viejo, infra), "usuario");
  const g = equiposPorGrupo([puesto, kvm, vmw, ct, srvFis, viejo], infra);
  igual("por grupo: las virtuales sin anotar arriba", g.virtual.map((e) => e.id), ["vm1", "ct1", "vm2"]);
  igual("por grupo: faltan solo las virtuales sin servidor", g.faltan, 2);
  igual("por grupo: de usuario", g.usuario.map((e) => e.id), ["pc1", "old"]);
  igual("propuesto: el único VMware", servidorPropuesto(vmw, infra)?.id, "esx");
  igual("propuesto: LXC en el Proxmox", servidorPropuesto(ct, infra)?.id, "pve");
  igual("propuesto: a un físico, nada", servidorPropuesto(puesto, infra), null);
  const dos = { ...infra, servidores: [...infra.servidores, { id: "esx2", nombre: "ESX-02", hipervisor: "vmware", discos: [], conjuntos: [] }] } as unknown as Infraestructura;
  igual("propuesto: con dos que encajan, ninguno", servidorPropuesto(vmw, dos), null);
  igual("para elegir: primero los que encajan", servidoresParaElegir(ct, infra).map((s) => s.id)[0], "pve");
  igual("hipervisores de KVM", hipervisoresPara("kvm"), ["proxmox", "kvm", "nutanix"]);
  igual("faltan: nada", textoFaltan(0), null);
  cierto("faltan: una", (textoFaltan(1) ?? "").startsWith("Falta decir dónde está 1"));
  const sa = sinAnotar(infra, [puesto, vmw, ct, srvFis, viejo], []);
  igual("aviso de Estado: solo virtuales y contenedores sin servidor", sa.equipos.map((e) => e.id), ["vm1", "ct1"]);
}

console.log("\n· 16d: la página de cada disco y de cada conjunto");
{
  const eq = (id: string) => ({ id, nombre: id.toUpperCase(), confirmado: true, modo: "gestionado", resumen: {} }) as unknown as Equipo;
  const infra = {
    servidores: [
      {
        id: "srv",
        nombre: "SRV-01",
        hipervisor: "hyperv",
        discos: [
          { id: "b1", nombre: "Bahía 1", tipo: "hdd" },
          { id: "b2", nombre: "Bahía 2", tipo: "hdd" },
          { id: "s1", nombre: "SSD suelto", tipo: "ssd" },
        ],
        conjuntos: [
          { id: "dv1", nombre: "Disco virtual 1", tipo: "raid1", miembros: ["b1", "b2"] },
          { id: "r0", nombre: "Rápido", tipo: "raid0" },
        ],
      },
    ],
    equipos: { vm1: { servidor: "srv", conjunto: "dv1" }, vm2: { servidor: "srv", volumenes: [{ punto: "D:", disco: "s1" }] } },
    destinos: { "zona:alm:principal": { servidor: "srv", conjunto: "dv1" } },
  } as unknown as Infraestructura;
  const destinos = [{ clave: "zona:alm:principal", nombre: "Almacén · principal", equipo: "alm" }];
  const fc = fichaSoporte(infra, "srv", "conjunto", "dv1", [eq("vm1"), eq("vm2")], destinos);
  igual("conjunto: sus discos", fc?.discosDelConjunto.map((d) => d.id), ["b1", "b2"]);
  igual("conjunto: lo que vive en él", fc?.equipos.map((x) => [x.equipo.id, x.entero]), [["vm1", true]]);
  igual("conjunto: sus destinos", fc?.destinos.map((d) => d.clave), ["zona:alm:principal"]);
  cierto("conjunto redundante: aguanta un disco", (fc?.siFalla[0] ?? "").startsWith("Aguanta que fallen un disco"));
  const fd = fichaSoporte(infra, "srv", "disco", "b1", [eq("vm1"), eq("vm2")], destinos);
  igual("disco: de qué conjunto", fd?.conjuntosDelDisco.map((c) => c.id), ["dv1"]);
  igual("disco en un espejo: lo de dentro también se ve", fd?.equipos.map((x) => x.equipo.id), ["vm1"]);
  igual("disco en un espejo: una frase (el conjunto aguanta)", fd?.siFalla.length, 1);
  const fs = fichaSoporte(infra, "srv", "disco", "s1", [eq("vm1"), eq("vm2")], destinos);
  igual("disco suelto: el volumen D: de VM2", fs?.equipos.map((x) => [x.equipo.id, x.entero, x.volumenes]), [["vm2", false, ["D:"]]]);
  cierto("disco suelto: se pierde", (fs?.siFalla[0] ?? "").startsWith("Es un disco suelto"));
  cierto("disco suelto: los originales de VM2 (D:)", (fs?.siFalla[1] ?? "").includes("VM2 (D:)"));
  const f0 = fichaSoporte(infra, "srv", "conjunto", "r0", [], []);
  cierto("RAID 0: lo dice", (f0?.siFalla[0] ?? "").startsWith("Es un RAID 0"));
  cierto("RAID 0 vacío: nada anotado", (f0?.siFalla.at(-1) ?? "") === "No hay nada anotado en él.");
  igual("no existe: null", fichaSoporte(infra, "srv", "disco", "nada", [], []), null);
  igual("servidor que no existe: null", fichaSoporte(infra, "otro", "disco", "b1", [], []), null);
  igual("dirección de la página", hrefSoporte("c1", "srv", "disco", "b1"), "/c/c1/ajustes/infraestructura/srv/disco/b1");
}

console.log("\n· 16b: los archivos que no se pudieron leer");
{
  igual("motivo: en uso", claseError("open C:\\x.pst: The process cannot access the file because it is being used by another process."), "en_uso");
  igual("motivo: sin permiso", claseError("open C:\\x: Access is denied."), "permiso");
  igual("motivo: OneDrive", claseError("read: The cloud file provider is not running."), "nube");
  igual("motivo: ruta larga", claseError("The filename or extension is too long."), "larga");
  igual("motivo: ya no estaba", claseError("lstat C:\\x: The system cannot find the file specified."), "no_existe");
  igual("motivo: carpeta que el agente no ve", claseError("el agente no encuentra esta carpeta"), "no_existe");
  igual("motivo: otro", claseError("algo raro"), "otro");
  igual("carpeta (Windows)", carpetaDe("C:\\Users\\ana\\a.pst"), "C:\\Users\\ana");
  igual("carpeta (raíz de Windows)", carpetaDe("C:\\a.pst"), "C:\\");
  igual("carpeta (Linux)", carpetaDe("/srv/datos/a.db"), "/srv/datos");
  igual("nombre", nombreDe("C:\\Users\\ana\\a b.pst"), "a b.pst");
  const xs = [
    { ruta: "C:\\U\\ana\\a.pst", motivo: "being used by another process", veces: 5 },
    { ruta: "C:\\U\\ana\\b.pst", motivo: "being used by another process", veces: 5 },
    { ruta: "C:\\U\\OneDrive\\c.docx", motivo: "cloud file provider", veces: 5 },
  ];
  const g = agruparArchivos(xs);
  igual("agrupados: primero el motivo con más archivos", g.map((x) => [x.clase, x.total]), [["en_uso", 2], ["nube", 1]]);
  igual("agrupados: por carpeta", g[0].carpetas.map((c) => [c.carpeta, c.archivos.length]), [["C:\\U\\ana", 2]]);
  cierto("OneDrive: explica qué pasa", g[1].explica.includes("Mantener siempre en este dispositivo"));
  igual("siempre los mismos", resumenRepeticion(xs, 5), "Son siempre los mismos 3 archivos: fallaron en las últimas 5 copias.");
  igual("uno siempre", resumenRepeticion([xs[0]], 5), "Es siempre el mismo archivo: falló en las últimas 5 copias.");
  cierto("algunos siempre", (resumenRepeticion([xs[0], { ...xs[1], veces: 1 }], 5) ?? "").startsWith("1 de 2 fallaron"));
  igual("una sola copia: nada que comparar", resumenRepeticion(xs, 1), null);
  igual("leer: lo que no cuadra, fuera", leerArchivos({ archivos: [{ ruta: "C:\\a", motivo: "x", veces: 2 }, { motivo: "sin ruta" }, null, "x"], copias: 99 }), { archivos: [{ ruta: "C:\\a", motivo: "x", veces: 2 }], copias: 10 });
  igual("leer: nada", leerArchivos(null), { archivos: [], copias: 1 });
  igual("exclusiones sin repetir (Windows, sin mayúsculas)", conExclusiones(["C:\\U\\ana\\A.pst"], ["C:\\U\\ana\\a.pst", "C:\\U\\ana\\b.pst"], true), ["C:\\U\\ana\\A.pst", "C:\\U\\ana\\b.pst"]);
  igual("exclusiones en Linux distinguen mayúsculas", conExclusiones(["/srv/A"], ["/srv/a"], false), ["/srv/A", "/srv/a"]);
}

console.log("\n· 16h: «Este equipo no llega a esa consola»");
{
  const ahora = Date.parse("2026-11-02T12:00:00Z");
  const reciente = new Date(ahora - 3600_000).toISOString();
  const linea = { identidad: "ID-LINEA", nombre: "Consola en línea", url: "https://consola.ejemplo.com", esta: false, ultimo_contacto: reciente };
  const local = { identidad: "ID-LOCAL", nombre: "Oficina", url: "https://oficina.ejemplo.com", esta: true, ultimo_contacto: reciente };
  const eq = (id: string, consolas: unknown[]) => ({ id, nombre: id.toUpperCase(), confirmado: true, modo: "gestionado", resumen: { admite: ["consolas_multiples"], consolas } }) as unknown as Equipo;
  const a = eq("a", [local, linea]);
  const vm1 = eq("vm1", [local]);
  const vm2 = eq("vm2", [local]);
  const sin = otrasConsolas([a, vm1, vm2], ahora);
  igual("sin marcas: faltan las dos", sin[0].sin.map((e) => e.id), ["vm1", "vm2"]);
  const marcas = { vm1: { "ID-LINEA": { motivo: "Red aislada" } } };
  const con = otrasConsolas([a, vm1, vm2], ahora, [], marcas);
  igual("con la marca: solo falta la otra", con[0].sin.map((e) => e.id), ["vm2"]);
  igual("la marcada sale aparte, con su motivo", con[0].aproposito.map((x) => [x.equipo.id, x.motivo]), [["vm1", "Red aislada"]]);
  igual("el aviso cuenta solo las que de verdad faltan", equiposQueFaltan(con).map((e) => e.id), ["vm2"]);
  igual("en su ficha ya no falta", faltaEn(vm1, con).length, 0);
  igual("marca de otra consola: no cuenta", otrasConsolas([a, vm1], ahora, [], { vm1: { "OTRA": {} } })[0].sin.map((e) => e.id), ["vm1"]);
  igual("¿marcado?", [noLlega(marcas, "vm1", "ID-LINEA"), noLlega(marcas, "vm2", "ID-LINEA"), noLlega(null, "vm1", "ID-LINEA")], [true, false, false]);
}

console.log(`\n${total - fallos} de ${total} bien`);
if (fallos) process.exit(1);
