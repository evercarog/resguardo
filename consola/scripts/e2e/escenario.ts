// Escenario de extremo a extremo de Resguardo con los programas DE VERDAD:
// resguardo-server y dos resguardo-agente (compilación de desarrollo, en modo
// de pruebas), restic y rest-server, hablando entre ellos por la red local
// (127.0.0.1) como en una oficina. «La consola» es este script, que usa la
// criptografía y la lógica de la consola (src/lib) contra la API real.
//
// Y, con un tercer servidor, «varias consolas a la vez» y «Mover a otra consola»
// (docs/consolas-multiples.md).
//
// Busca los fallos que no ven las pruebas unitarias: lo que la consola enseña
// después de una copia («0 versiones»), lo que tarda en llegar un cambio, lo
// que pasa al restaurar la consola, etc.
//
//   cargo build -p resguardo-servidor -p resguardo-agente --bins
//   cd consola && npm run e2e
//
// Variables (opcionales):
//   RESGUARDO_E2E_TARGET     carpeta con resguardo-server y resguardo-agente (src-tauri/target/debug)
//   RESGUARDO_E2E_BINARIOS   carpeta con restic y rest-server (Windows: src-tauri/binaries, con sus
//                            nombres de Tauri; Linux: la de scripts/fetch-binarios-linux.sh)
//   RESGUARDO_E2E_TMP        dónde crear la carpeta temporal (la del sistema)
//   RESGUARDO_E2E_MB         tamaño de los datos de prueba del equipo B (64)
//   RESGUARDO_E2E_CONSERVAR  1: no borrar la carpeta temporal al terminar (para mirar los registros)
//   RESGUARDO_E2E_OBLIGATORIO 1: sin restic o rest-server, fallar en vez de saltarse
//
// Todo corre como procesos normales, con carpetas temporales en <tmp>/e2e-<azar>/ y puertos
// libres en 127.0.0.1. Nunca toca servicios instalados ni ProgramData.
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { createHash, randomBytes } from "node:crypto";
import { cuerpoAlmacen, errorTrabajos, FRENO_DEFECTO, trabajosDelAlmacen, type TrabajoEspejo } from "../../src/lib/espejoTrabajos";
import { spawnSync } from "node:child_process";
import { aB64, aleatorio } from "../../src/lib/cripto/bytes";
import { etiquetaValida, kCfg, materialCliente } from "../../src/lib/cripto/claves";
import { ClaveNueva } from "../../src/lib/cambioClave";
import { crearCodigo, cuerpoAnadir, leerCodigo } from "../../src/lib/conexion";
import { anclaDe, comprobarAncla, leerAncla, lineaAncla } from "../../src/lib/auditoria";
import { fraseEquipo, otrasConsolas } from "../../src/lib/consolasCliente";
import { publicaRespaldo, salRespaldo } from "../../src/lib/cripto/respaldo";
import { almacenDe, nuevaClave, reglaParaOrden, seQuedan } from "../../src/lib/retencion";
import { vueltasDelRepo, type EntradaRetencion } from "../../src/lib/retencionDetalle";
import { bytesRepo, destinoDe, informeDe, nVersiones, proteccion } from "../../src/lib/repo";
import { proximaDe } from "../../src/lib/copia";
import { claveZona, destinosDelCliente, errorRespuestaZona, idDestinoZona, zonaDeDestino, zonasDe } from "../../src/lib/destinos";
import { destinoNubeCuerpo, opcionesRepoNuevo } from "../../src/lib/repoNuevo";
import { nubesDelEquipo } from "../../src/lib/nubesEquipo";
import { unirBusqueda, type PaginaBusqueda } from "../../src/lib/buscarArchivos";
import { pasoAlDia, reglaDeCopia } from "../../src/lib/regla321";
import type { Cliente, DestinoCatalogo, EntradaAuditoria, Equipo, Regla } from "../../src/lib/tipos";
import { argon2, Agente, binario, Consola, SesionE2E, Servidor } from "./actores";
// @ts-expect-error: módulo de Node en JavaScript, sin tipos.
import { firmarPruebas, semillaPruebas } from "../../../scripts/lib/minisign.mjs";
import { OyenteVivo } from "./vivo";
import { borrarCarpeta, BuzonSmtp, comprobar, dormir, EXE, ejecutar, esperar, Fallo, igual, log, paso, pasoEnCurso, pararTodo, puertoLibre, WIN } from "./entorno";

const AQUI = path.dirname(fileURLToPath(import.meta.url));
const RAIZ = path.resolve(AQUI, "../../..");
const TARGET = process.env.RESGUARDO_E2E_TARGET ?? path.join(RAIZ, "src-tauri", "target", "debug");
const CORREO = "ana@ejemplo.com";
const CONTRASENA = "una contraseña bien larga para la consola";
// Con tildes: la consola y el agente la normalizan igual (NFC).
const CLAVE_ADMIN = "caballo batería grapa correcta";
// La de después de «Cambiar la clave de administración» (paso 8b).
const CLAVE_NUEVA = "otra clave de administración, bien larga";
const CLAVE_RESPALDO = "clave de respaldo de la consola, larga";

// ---------------------------------------------------------------------------
// Programas
// ---------------------------------------------------------------------------

function buscarBinarios(): { restic: string; restServer: string } | null {
  const dir = process.env.RESGUARDO_E2E_BINARIOS ?? path.join(RAIZ, "src-tauri", "binaries");
  const candidatos = (n: string) => [path.join(dir, `${n}${EXE}`), path.join(dir, `${n}-x86_64-pc-windows-msvc.exe`), path.join(dir, `${n}-x86_64-unknown-linux-gnu`)];
  const restic = candidatos("restic").find((p) => fs.existsSync(p)) ?? (WIN ? null : (spawnSync("sh", ["-c", "command -v restic"], { encoding: "utf8" }).stdout.trim() || null));
  const restServer = candidatos("rest-server").find((p) => fs.existsSync(p));
  return restic && restServer ? { restic, restServer } : null;
}

/** El archivo de código (`src/`, `Cargo.toml`) más reciente de esas carpetas de crates. */
function codigoMasNuevo(carpetas: string[]): { archivo: string; t: number } | null {
  let mejor: { archivo: string; t: number } | null = null;
  const ver = (p: string) => {
    const t = fs.statSync(p).mtimeMs;
    if (!mejor || t > mejor.t) mejor = { archivo: p, t };
  };
  const recorrer = (d: string) => {
    for (const e of fs.readdirSync(d, { withFileTypes: true })) {
      const p = path.join(d, e.name);
      if (e.isDirectory()) recorrer(p);
      else if (/\.(rs|toml|json|sql)$/.test(e.name)) ver(p);
    }
  };
  for (const c of carpetas) {
    if (fs.existsSync(path.join(c, "Cargo.toml"))) ver(path.join(c, "Cargo.toml"));
    if (fs.existsSync(path.join(c, "src"))) recorrer(path.join(c, "src"));
  }
  return mejor;
}

const resticVersion = (restic: string) => ejecutar(restic, ["version"]).salida.trim();

// ---------------------------------------------------------------------------
// Utilidades del escenario
// ---------------------------------------------------------------------------

/** La ruta de restic de un archivo del equipo («/F/datos/x.txt» en Windows). */
const rutaRestic = (p: string) => (WIN ? `/${p[0].toUpperCase()}${p.slice(2).replace(/\\/g, "/")}` : p);

/** Datos de prueba: unos pocos archivos de texto conocidos y relleno aleatorio (no se comprime). */
function crearDatos(dir: string, mb: number) {
  fs.mkdirSync(path.join(dir, "Facturas"), { recursive: true });
  fs.mkdirSync(path.join(dir, "cache"), { recursive: true });
  fs.writeFileSync(path.join(dir, "Facturas", "factura-001.txt"), "Factura 001 · Cliente Café del Sur · 1.234,56 €\n".repeat(50));
  fs.writeFileSync(path.join(dir, "notas.md"), "# Notas\nAlgo importante.\n");
  fs.writeFileSync(path.join(dir, "borrador.tmp"), "esto no se copia");
  fs.writeFileSync(path.join(dir, "cache", "basura.bin"), randomBytes(1024));
  const relleno = path.join(dir, "Relleno");
  fs.mkdirSync(relleno, { recursive: true });
  const porArchivo = 4 * 1024 * 1024;
  for (let i = 0; i * porArchivo < mb * 1024 * 1024; i++) fs.writeFileSync(path.join(relleno, `parte-${String(i).padStart(3, "0")}.bin`), randomBytes(porArchivo));
}

function horaLocal(d: Date) {
  const p = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())} ${p(d.getHours())}:${p(d.getMinutes())}:${p(d.getSeconds())}`;
}

const HH = (d: Date) => `${String(d.getHours()).padStart(2, "0")}:00`;

// ---------------------------------------------------------------------------
// El escenario
// ---------------------------------------------------------------------------

async function principal() {
  const bins = buscarBinarios();
  if (!bins) {
    const m = "No hay restic y rest-server (RESGUARDO_E2E_BINARIOS, src-tauri/binaries o restic en el PATH): escenario saltado.";
    if (process.env.RESGUARDO_E2E_OBLIGATORIO === "1") throw new Fallo(m);
    console.log(`SALTADO: ${m}`);
    return;
  }
  for (const b of ["resguardo-server", "resguardo-agente"]) {
    comprobar(fs.existsSync(binario(TARGET, b)), `Falta ${binario(TARGET, b)}: compila antes con «cargo build -p resguardo-servidor -p resguardo-agente --bins».`);
  }
  // Un binario anterior a su código (p. ej. solo se recompiló el servidor) prueba otra cosa
  // y falla lejos de la causa: mejor decirlo aquí (como cargo, por la fecha de los archivos).
  if (process.env.RESGUARDO_E2E_BINARIOS_VIEJOS !== "1") {
    const fuentes: Record<string, string[]> = { "resguardo-server": ["servidor", "motor", "protocolo"], "resguardo-agente": ["agente", "motor", "protocolo"] };
    for (const [b, crates] of Object.entries(fuentes)) {
      const nuevo = codigoMasNuevo(crates.map((k) => path.join(RAIZ, "crates", k)));
      const t = fs.statSync(binario(TARGET, b)).mtimeMs;
      comprobar(
        !nuevo || nuevo.t <= t,
        `${binario(TARGET, b)} es anterior a su código (${path.relative(RAIZ, nuevo?.archivo ?? "")}): compila antes con «cargo build -p resguardo-servidor -p resguardo-agente --bins» (o RESGUARDO_E2E_BINARIOS_VIEJOS=1 para probarlo igual).`,
      );
    }
  }
  const base = fs.mkdtempSync(path.join(process.env.RESGUARDO_E2E_TMP ?? os.tmpdir(), "e2e-"));
  const dir = (...p: string[]) => path.join(base, ...p);
  const registros = dir("registros");
  fs.mkdirSync(registros, { recursive: true });
  console.log(`Carpeta del escenario: ${base}`);
  console.log(`restic: ${resticVersion(bins.restic)}`);

  // El agente busca restic y rest-server junto a él: una carpeta con los tres.
  const binDir = dir("bin");
  fs.mkdirSync(binDir);
  fs.copyFileSync(binario(TARGET, "resguardo-agente"), binario(binDir, "resguardo-agente"));
  fs.copyFileSync(bins.restic, binario(binDir, "restic"));
  fs.copyFileSync(bins.restServer, binario(binDir, "rest-server"));
  if (!WIN) for (const b of ["resguardo-agente", "restic", "rest-server"]) fs.chmodSync(binario(binDir, b), 0o755);
  // Tarea 4a (paso 3c): el rclone que acompaña al agente, para una copia derivada por rclone.
  const rclone = [path.dirname(bins.restic), path.join(RAIZ, "src-tauri", "binaries")]
    .flatMap((d) => [path.join(d, `rclone${EXE}`), path.join(d, "rclone-x86_64-pc-windows-msvc.exe"), path.join(d, "rclone-x86_64-unknown-linux-gnu")])
    .find((p) => fs.existsSync(p));
  if (rclone) {
    fs.copyFileSync(rclone, binario(binDir, "rclone"));
    if (!WIN) fs.chmodSync(binario(binDir, "rclone"), 0o755);
  }
  const agenteBin = binario(binDir, "resguardo-agente");
  const resticBin = binario(binDir, "restic");
  const servidorBin = binario(TARGET, "resguardo-server");

  let ok = false;
  const buzon = new BuzonSmtp(registros);
  try {
    await buzon.abrir();
    const s1 = new Servidor("Servidor 1", servidorBin, dir("servidor-1"), await puertoLibre(), path.join(registros, "servidor-1.log"));
    const A = new Agente("ALMACEN-A", agenteBin, dir("agente-a"), path.join(registros, "agente-a.log"));
    const B = new Agente("EQUIPO-B", agenteBin, dir("agente-b"), path.join(registros, "agente-b.log"));
    const datosB = dir("datos-b");
    const almacen = dir("almacen-a");
    const mb = Number(process.env.RESGUARDO_E2E_MB ?? 64);
    crearDatos(datosB, mb);

    // -----------------------------------------------------------------------
    paso("1. Resguardo Server: primer arranque, cuenta de propietario con TOTP y un cliente");
    await s1.arrancar();
    const consola = new Consola(s1);
    await consola.primerArranque(CORREO, "Ana", CONTRASENA);
    const c: Cliente = await consola.ok("POST", "/api/clientes", { nombre: "Café del Sur", espera_min_horas: 1 });
    comprobar(c.sal_cliente, "El cliente nuevo trae su sal", c);
    Object.assign(c, await consola.ok("GET", `/api/clientes/${c.id}`));
    log(`Cliente «${c.nombre}» (${c.id})`);

    // -----------------------------------------------------------------------
    paso("2. Dos agentes: A guarda copias, B es un equipo normal (emparejados con SAS v3)");
    // A con el instalador listo armado en el navegador (v1.48); B con el código de la forma de antes
    // (lo genera el servidor: consolas anteriores). B se vuelve a vincular en el paso 7 con el código
    // de 15 min generado en el navegador.
    const eqA = await consola.emparejar(c, A, CLAVE_ADMIN, "instalador");
    const eqB = await consola.emparejar(c, B, CLAVE_ADMIN, "servidor");
    const puertoAlmacen = await puertoLibre();
    await consola.hecha(c, eqA.id, "guarda_copias", { activo: true, carpeta: almacen, puerto: puertoAlmacen, solo_red_local: true }, { claveAdmin: CLAVE_ADMIN });
    // La consola ofrece «Copiar en ALMACEN-A» cuando su resumen dice que guarda copias.
    const almacenListo = await esperar("que el resumen de A diga que guarda copias", async () => {
      const e = await consola.equipo(c, eqA.id);
      return e.resumen?.guarda_copias?.activo && e.rol === "almacenamiento" ? e : null;
    }, { plazo: 30_000 });
    igual(almacenListo.resumen!.guarda_copias!.puerto, puertoAlmacen, "Puerto del almacén en el resumen");
    log(`A guarda copias en ${almacen} (puerto ${puertoAlmacen})`);

    // -----------------------------------------------------------------------
    paso("3. B copia en el almacén de A: «Copiar en», copia con horario y exclusiones, «Copiar ahora»");
    interface Acceso {
      usuario: string;
      contrasena: string;
      destino: { tipo: "rest"; donde: string; usuario: string; secreto: string; ca_pem: string };
      huella_tls: string;
    }
    const acceso = await consola.hechaSellada<Acceso>(c, eqA.id, "guarda_copias", { anadir: eqB.id }, { claveAdmin: CLAVE_ADMIN });
    comprobar(acceso.destino?.ca_pem?.includes("BEGIN CERTIFICATE") && acceso.huella_tls, "El acceso sellado trae la autoridad del almacén", acceso);
    const repoId = `almacen-a-${randomBytes(2).toString("hex")}`;
    const contrasenaRepo = Buffer.from(aleatorio(32)).toString("base64url");
    await consola.hecha(
      c,
      eqB.id,
      "crear_repositorio",
      {
        id: repoId,
        nombre: "Copias en ALMACEN-A",
        contrasena: contrasenaRepo,
        // Como CopiarEnAlmacen.svelte (v1.30: con `equipo_almacen`).
        destino: {
          id: `almacen-${eqA.id.slice(0, 8)}`,
          nombre: eqA.nombre,
          tipo: "rest",
          donde: acceso.destino.donde,
          usuario: acceso.destino.usuario,
          secreto: acceso.destino.secreto,
          ca_pem: acceso.destino.ca_pem,
          equipo_almacen: eqA.id,
        },
      },
      { claveAdmin: CLAVE_ADMIN },
      {},
      120_000,
    );
    // Sin esperar al informe de 5 minutos: el repositorio nuevo sale ya en el resumen.
    const conRepo = await esperar("el repositorio nuevo en el resumen de B", async () => {
      const e = await consola.equipo(c, eqB.id);
      return e.resumen?.repositorios?.find((r) => r.id === repoId) ? e : null;
    }, { plazo: 20_000 });
    // v1.30: el destino dice de qué almacén es (la consola ya no tiene que adivinarlo por el nombre).
    const destinoB = destinoDe(conRepo.resumen?.destinos, conRepo.resumen!.repositorios!.find((r) => r.id === repoId));
    igual(destinoB?.equipo_almacen, eqA.id, "El resumen de B dice que el destino es el almacén de A (equipo_almacen)");

    const ahora = new Date();
    const horaCopia = HH(new Date(ahora.getTime() + 3 * 3600_000));
    const copia = {
      id: "documentos",
      nombre: "Documentos",
      repo: repoId,
      carpetas: [datosB],
      exclusiones: ["*.tmp", "cache"],
      horario: { dias: [1, 2, 3, 4, 5, 6, 7], horas: [horaCopia], reglas: [{ tipo: "horas", dias: [1, 2, 3, 4, 5, 6, 7], horas: [horaCopia] }, { tipo: "mensual", dia: -1, hora: "23:30" }] },
      activa: true,
      gancho: null,
      solo_si_cambios: true,
    };
    await consola.hecha(c, eqB.id, "config", { config: { v: 1, copias: [copia] } }, { claveAdmin: CLAVE_ADMIN });
    await esperar("la copia en el resumen de B, con su próxima vez", async () => {
      const k = (await consola.equipo(c, eqB.id)).resumen?.copias?.find((x) => x.id === "documentos");
      return k?.proxima ? k : null;
    }, { plazo: 20_000 });

    // «Modo discreto» en B (agent.json, lo que pone un administrador en el equipo): la subida a
    // destinos remotos, limitada. Así la copia dura unos segundos y se ve su progreso en vivo
    // (el agente lo manda cada 5 s), y de paso se prueba el límite.
    const p2 = (n: number) => String(n).padStart(2, "0");
    const hm = (d: Date) => `${p2(d.getHours())}:${p2(d.getMinutes())}`;
    const cfgB = JSON.parse(fs.readFileSync(path.join(B.dir, "agent.json"), "utf8"));
    cfgB.discreet = { days: [0, 1, 2, 3, 4, 5, 6], from: hm(new Date(Date.now() - 3600_000)), to: hm(new Date(Date.now() + 3 * 3600_000)), upload_kib: 4096 };
    fs.writeFileSync(path.join(B.dir, "agent.json"), JSON.stringify(cfgB, null, 2));

    // «Copiar ahora» y el progreso en vivo (lo que pinta la barra de la consola).
    const antes = Date.now();
    const vistas: any[] = [];
    let siguiendo = true;
    const seguir = (async () => {
      while (siguiendo) {
        const p = await consola.ok("GET", `/api/clientes/${c.id}/progreso`).catch(() => []);
        for (const x of p as { equipo: string; tareas: any[] }[]) if (x.equipo === eqB.id) vistas.push(...x.tareas);
        await dormir(300);
      }
    })();
    await consola.hecha(c, eqB.id, "copiar_ahora", { copia: "documentos", repo: repoId });
    const ultima = await esperar("que termine la copia de B", async () => {
      const k = (await consola.equipo(c, eqB.id)).resumen?.copias?.find((x) => x.id === "documentos");
      return k?.ultima && new Date(k.ultima.cuando).getTime() >= antes - 5_000 ? k.ultima : null;
    }, { plazo: 180_000, cada: 1000 });
    const finVisto = Date.now();
    siguiendo = false;
    await seguir;
    igual(ultima.estado, "ok", `La copia terminó bien (${ultima.mensaje ?? ""})`);
    const copias = vistas.filter((t) => t.tipo === "copia" && t.repo === repoId);
    log(`Progreso en vivo: ${copias.length} mensajes (${[...new Set(copias.map((t) => t.fase))].join(", ")})`);
    comprobar(copias.length > 0, "La consola vio el progreso en vivo de la copia");
    comprobar(copias.some((t) => ["escaneando", "subiendo", "terminando"].includes(t.fase) && t.bytes_total > 0), "Con cifras de restic", copias.slice(-3));

    // Lo que enseña la consola: versiones, tamaño, próxima vez, cifras de la vuelta y salud de la protección.
    const vista = await esperar("versiones > 0 en lo que ve la consola", async () => {
      const e = await consola.equipo(c, eqB.id);
      const r = e.resumen?.repositorios?.find((x) => x.id === repoId);
      const inf = informeDe(e.ultimo_informe as any, repoId);
      return r && nVersiones(r, inf) > 0 && bytesRepo(r, inf) && inf?.ejecuciones?.length ? { e, r, inf } : null;
    }, { plazo: 60_000, cada: 500 });
    // v1.30: el informe de cuando termina la copia ya lleva sus versiones (antes llegaban 15–35 s después).
    const retraso = Date.now() - finVisto;
    log(`Versiones en la consola ${(retraso / 1000).toFixed(1)} s después de ver terminar la copia`);
    comprobar(retraso < 5_000, `Las versiones llegan con el final de la copia, no en el informe siguiente (${retraso} ms)`);
    const k = vista.e.resumen!.copias!.find((x) => x.id === "documentos")!;
    const proxima = proximaDe(k, vista.e.ultimo_informe as any);
    comprobar(proxima && new Date(proxima).getTime() > Date.now(), "Próxima copia en el futuro", { proxima });
    const vuelta = vista.inf!.ejecuciones[0];
    comprobar(vuelta.resultado === "ok" && vuelta.duracion_s !== null && (vuelta.anadido ?? 0) > 0 && (vuelta.archivos_nuevos ?? 0) > 0, "Cifras de la vuelta (añadido, archivos, duración)", vuelta);
    const prot = proteccion(vista.inf);
    comprobar(prot && prot.total > 0 && prot.puntuacion > 0, "Salud de la protección", vista.inf?.proteccion);
    log(`Consola: ${nVersiones(vista.r, vista.inf)} versiones · ${bytesRepo(vista.r, vista.inf)} B · próxima ${proxima} · protección ${prot.puntuacion}/${prot.total}`);

    /** «Copiar ahora» en B y espera a que termine: devuelve `ultima` de la copia en el resumen. */
    const copiarAhora = async (srvConsola: Consola, cl: Cliente, plazo = 180_000) => {
      const desde = Date.now();
      // La vuelta anterior no cuenta (puede haber terminado justo antes de la orden).
      const anterior = (await srvConsola.equipo(cl, eqB.id)).resumen?.copias?.find((x) => x.id === "documentos")?.ultima?.cuando;
      await srvConsola.hecha(cl, eqB.id, "copiar_ahora", { copia: "documentos", repo: repoId });
      return esperar("que termine la copia de B", async () => {
        const k = (await srvConsola.equipo(cl, eqB.id)).resumen?.copias?.find((x) => x.id === "documentos");
        return k?.ultima && k.ultima.cuando !== anterior && new Date(k.ultima.cuando).getTime() >= desde - 1_000 ? k.ultima : null;
      }, { plazo, cada: 1000 });
    };
    const secretosRepo = { repo: { repo: repoId, contrasena: contrasenaRepo } };

    // -----------------------------------------------------------------------
    paso("3b. Otra zona en el almacén de A (tarea 7b): «Disco E» con su puerto; B copia también allí");
    {
      const resumenA = (await consola.equipo(c, eqA.id)).resumen;
      comprobar(resumenA?.admite?.includes("zonas_almacen"), "El almacén dice que admite zonas", resumenA?.admite);
      const carpetaE = dir("almacen-a-disco-e");
      const puertoE = await puertoLibre();
      // Dentro de la carpeta del almacén, no: el agente lo rechaza.
      const mal = await consola.resultado(c, eqA.id, await consola.mandar(c, eqA.id, "guarda_copias", { zona: { carpeta: path.join(almacen, "dentro"), puerto: puertoE } }, { claveAdmin: CLAVE_ADMIN }));
      igual(mal.estado, "fallida", `Una zona dentro de la carpeta del almacén se rechaza (${mal.mensaje ?? ""})`);
      const r = await consola.hecha(c, eqA.id, "guarda_copias", { zona: { nombre: "Disco E", carpeta: carpetaE, puerto: puertoE } }, { claveAdmin: CLAVE_ADMIN }, {}, 60_000);
      log(`Zona: ${r.mensaje}`);
      const zona = await esperar("la zona en el resumen de A, en marcha", async () => {
        const z = (await consola.equipo(c, eqA.id)).resumen?.guarda_copias?.zonas?.find((x) => x.puerto === puertoE);
        return z?.escucha ? z : null;
      }, { plazo: 30_000 });
      igual([zona.nombre, zona.carpeta, zona.usuarios], ["Disco E", carpetaE, 0], "La zona con su nombre, su carpeta y sin equipos todavía");
      // El acceso de B en esa zona: su propio usuario, por el puerto de la zona (lo comprueba la consola).
      const accesoE = await consola.hechaSellada<{ usuario: string; zona?: string; destino: { donde: string; usuario: string; secreto: string; ca_pem: string } }>(
        c,
        eqA.id,
        "guarda_copias",
        { anadir: eqB.id, zona: zona.id },
        { claveAdmin: CLAVE_ADMIN },
      );
      const vista = zonasDe(await consola.equipo(c, eqA.id)).find((z) => z.id === zona.id)!;
      igual(errorRespuestaZona(accesoE, vista), null, "La respuesta es de la zona pedida y por su puerto");
      comprobar(accesoE.usuario !== acceso.usuario, "En la zona, B tiene otro usuario (únicos en todo el almacén)", [accesoE.usuario, acceso.usuario]);
      const repoE = `disco-e-${randomBytes(2).toString("hex")}`;
      const contrasenaE = Buffer.from(aleatorio(32)).toString("base64url");
      await consola.hecha(
        c,
        eqB.id,
        "crear_repositorio",
        {
          id: repoE,
          nombre: "Copias en ALMACEN-A · Disco E",
          contrasena: contrasenaE,
          // Como CopiarEnAlmacen.svelte con una zona.
          destino: { id: idDestinoZona(eqA.id, zona.id), nombre: `${eqA.nombre} · Disco E`, tipo: "rest", donde: accesoE.destino.donde, usuario: accesoE.destino.usuario, secreto: accesoE.destino.secreto, ca_pem: accesoE.destino.ca_pem, equipo_almacen: eqA.id },
        },
        { claveAdmin: CLAVE_ADMIN },
        {},
        120_000,
      );
      const copiaE = { ...copia, id: "documentos-e", nombre: "Documentos a Disco E", repo: repoE };
      await consola.hecha(c, eqB.id, "config", { config: { v: 1, copias: [copia, copiaE] } }, { claveAdmin: CLAVE_ADMIN });
      const desde = Date.now();
      await consola.hecha(c, eqB.id, "copiar_ahora", { copia: "documentos-e", repo: repoE });
      const ultimaE = await esperar("que termine la copia de B en la zona", async () => {
        const k = (await consola.equipo(c, eqB.id)).resumen?.copias?.find((x) => x.id === "documentos-e");
        return k?.ultima && new Date(k.ultima.cuando).getTime() >= desde - 5_000 ? k.ultima : null;
      }, { plazo: 180_000, cada: 1000 });
      igual(ultimaE.estado, "ok", `La copia en la zona terminó bien (${ultimaE.mensaje ?? ""})`);
      comprobar(fs.existsSync(path.join(carpetaE, accesoE.usuario, repoE, "config")), "El repositorio está en la carpeta de la zona");
      comprobar(!fs.existsSync(path.join(almacen, accesoE.usuario)), "Y no en la carpeta principal del almacén");
      // El resumen de A dice que un equipo copia en la zona (sale tras la orden `anadir`; la lista de
      // repositorios de cada zona llega con su informe periódico, como la de la principal).
      await esperar("un equipo en la zona, en el resumen de A", async () => {
        const z = (await consola.equipo(c, eqA.id)).resumen?.guarda_copias?.zonas?.find((x) => x.id === zona.id);
        return z?.usuarios === 1 ? z : null;
      }, { plazo: 30_000 });
      // «Se guarda en» y el destino de la consola: dentro de esa zona.
      const eqBAhora = await consola.equipo(c, eqB.id);
      const eqAAhora = await consola.equipo(c, eqA.id);
      const dE = destinoDe(eqBAhora.resumen?.destinos, eqBAhora.resumen?.repositorios?.find((x) => x.id === repoE));
      igual(zonaDeDestino(dE, [eqAAhora, eqBAhora])?.id, zona.id, "La consola reconoce el destino de B como la zona E de A");
      // El catálogo de destinos (7a): un nombre para la zona, en claro y sin secretos.
      await consola.ok("PUT", `/api/clientes/${c.id}/destinos/${encodeURIComponent(claveZona(eqA.id, zona.id))}`, { nombre: "Almacén · Disco E", tipo: "zona" });
      const catalogo = await consola.ok<DestinoCatalogo[]>("GET", `/api/clientes/${c.id}/destinos`);
      igual(destinosDelCliente([eqAAhora, eqBAhora], catalogo).find((v) => v.clave === claveZona(eqA.id, zona.id))?.nombre, "Almacén · Disco E", "El nombre del catálogo se ve en la zona");
      // Tarea 8: el sistema de archivos de la zona (solo un dato) y la regla 3-2-1-1-0 de la copia de B en ella.
      const zE = eqAAhora.resumen?.guarda_copias?.zonas?.find((x) => x.id === zona.id);
      comprobar(typeof zE?.sistema_archivos === "string" && zE.sistema_archivos.length > 0, "El almacén dice el sistema de archivos de la zona", zE);
      const urlZona = `/api/clientes/${c.id}/destinos/${encodeURIComponent(claveZona(eqA.id, zona.id))}`;
      await consola.ok("PUT", urlZona, { nombre: "Almacén · Disco E", tipo: "zona", atributos: { inmutable: "instantaneas" } });
      // Renombrar sin `atributos` (como una consola anterior) no los borra.
      await consola.ok("PUT", urlZona, { nombre: "Almacén · Disco E (zona)", tipo: "zona" });
      const catalogo8 = await consola.ok<DestinoCatalogo[]>("GET", `/api/clientes/${c.id}/destinos`);
      igual(catalogo8.find((x) => x.id === claveZona(eqA.id, zona.id))?.atributos, { inmutable: "instantaneas" }, "Lo marcado para la regla 3-2-1 se queda al renombrar");
      const kE = eqBAhora.resumen?.copias?.find((x) => x.id === "documentos-e");
      const rcE = kE ? reglaDeCopia(eqBAhora, kE, [eqAAhora, eqBAhora], null, catalogo8, Date.now()) : null;
      igual(
        rcE?.pasos.map((p) => [p.lugar, p.inmutable, p.porDefecto.inmutable, pasoAlDia(p, Date.now())]),
        [["oficina", "instantaneas", "solo_anadir", true]],
        "La regla ve la copia de B en la zona E: otro equipo de la oficina, al día y con lo marcado",
      );
      igual(rcE?.regla.avisos, ["inmutable_local"], "…y avisa de que lo inmutable es local");
      log(`B copia también en «Disco E» (puerto ${puertoE}, usuario ${accesoE.usuario})`);

      // ---------------------------------------------------------------------
      paso("3c. Copias en cadena (tarea 7, parte B): Documentos → zona D; después → zona E; espejo E → D; copia derivada por rclone con otra contraseña");
      const eqBVer = await consola.equipo(c, eqB.id);
      comprobar(["cadenas", "derivadas", "filtros", "nube_equipo"].every((x) => eqBVer.resumen?.admite?.includes(x)), "B admite cadenas, derivadas, filtros y nubes en el equipo", eqBVer.resumen?.admite);
      comprobar(eqAAhora.resumen?.admite?.includes("espejo_zonas"), "A admite el espejo por zonas", eqAAhora.resumen?.admite);
      // 1. «Después de la anterior»: la copia a la zona E empieza cuando termina bien la de la zona D.
      // (Una copia aparte al mismo repositorio: «Documentos» sigue con su última vez, que el paso 5b cuenta.)
      const copiaD = { ...copia, id: "cadena-d", nombre: "Cadena: Documentos a Disco D" };
      const copiaTras = { ...copiaE, horario: { dias: [], horas: [] }, tras: "cadena-d" };
      await consola.hecha(c, eqB.id, "config", { config: { v: 1, copias: [copia, copiaD, copiaTras] } }, { claveAdmin: CLAVE_ADMIN });
      const antesE = (await consola.equipo(c, eqB.id)).resumen?.copias?.find((x) => x.id === "documentos-e")?.ultima?.cuando;
      const desdeD = Date.now();
      await consola.hecha(c, eqB.id, "copiar_ahora", { copia: "cadena-d", repo: repoId });
      const ultimaD = await esperar("que termine la primera de la cadena", async () => {
        const k = (await consola.equipo(c, eqB.id)).resumen?.copias?.find((x) => x.id === "cadena-d");
        return k?.ultima && new Date(k.ultima.cuando).getTime() >= desdeD - 1_000 ? k.ultima : null;
      }, { plazo: 180_000, cada: 1000 });
      igual(ultimaD.estado, "ok", "La primera de la cadena terminó bien");
      const ultimaTras = await esperar("que la cadena lance la copia a la zona E", async () => {
        const k = (await consola.equipo(c, eqB.id)).resumen?.copias?.find((x) => x.id === "documentos-e");
        return k?.ultima && k.ultima.cuando !== antesE ? k : null;
      }, { plazo: 180_000, cada: 1000 });
      igual([ultimaTras.ultima!.estado, ultimaTras.tras], ["ok", "cadena-d"], "La segunda se hizo sola, después de la primera");
      // 2. Paso «espejo»: el almacén copia ese repositorio de la zona E a la principal, en local (sin contraseñas).
      const nombreEnA = `${accesoE.usuario}/${repoE}`;
      // El espejo deja para la vuelta siguiente lo escrito en los últimos 10 minutos (una subida a medias):
      // aquí, como si la copia hubiera sido hace una hora.
      const haceUnaHora = new Date(Date.now() - 3600_000);
      const envejecer = (d: string) => {
        for (const e of fs.readdirSync(d, { withFileTypes: true })) {
          const p = path.join(d, e.name);
          if (e.isDirectory()) envejecer(p);
          else fs.utimesSync(p, haceUnaHora, haceUnaHora);
        }
      };
      envejecer(path.join(carpetaE, accesoE.usuario, repoE));
      await consola.hecha(
        c,
        eqA.id,
        "guarda_copias",
        { espejo: { hora: "02:00", destinos: [{ tipo: "zona", carpeta: "principal", zona: zona.id, repos: [nombreEnA], vistos: [nombreEnA], tras_copia: true }] } },
        { claveAdmin: CLAVE_ADMIN },
      );
      await esperar("el espejo de la zona E en la principal", async () => (fs.existsSync(path.join(almacen, accesoE.usuario, repoE, "config")) ? true : null), { plazo: 120_000, cada: 1000 });
      const dEsp = (await consola.equipo(c, eqA.id)).resumen?.guarda_copias?.espejo?.destinos?.[0];
      igual([dEsp?.tipo, dEsp?.zona], ["zona", zona.id], "El resumen de A dice el destino «zona» y su zona de origen");
      comprobar(!fs.existsSync(path.join(almacen, accesoE.usuario, "documentos")), "Solo ese repositorio");
      if (rclone) {
        // 3. Una nube en B (por rclone; en las pruebas, una carpeta) y una copia derivada a ella: otra contraseña y un filtro de fechas.
        const carpetaNube = dir("nube-b");
        fs.mkdirSync(carpetaNube, { recursive: true });
        await consola.hecha(c, eqB.id, "conectar_nube", { tipo: "alias", nombre: "Nube de pruebas", parametros: { carpeta: carpetaNube } }, { claveAdmin: CLAVE_ADMIN }, {}, 60_000);
        const otraClave = Buffer.from(aleatorio(24)).toString("base64url");
        const secretosE = { repo: { repo: repoE, contrasena: contrasenaE } };
        await consola.hecha(
          c,
          eqB.id,
          "cambiar_derivada",
          {
            repo: repoE,
            id: "d1",
            destino: { id: "nube-pruebas", nombre: "Nube de pruebas", tipo: "nube", nube: "Nube de pruebas", donde: "Resguardo" },
            tras_copia: true,
            contrasena_destino: otraClave,
            filtro: { ultimos_dias: 30 },
            verificacion: { cada_dias: 7, porcentaje: 5 },
          },
          secretosE,
          {},
          120_000,
        );
        await consola.hecha(c, eqB.id, "subir_ahora", { repo: repoE, derivada: "d1" });
        const dv = await esperar("la copia derivada hecha (informe de B)", async () => {
          const inf = informeDe((await consola.equipo(c, eqB.id)).ultimo_informe as any, repoE) as any;
          const x = inf?.derivadas?.find((d: { id: string }) => d.id === "d1");
          return x?.resultado === "ok" ? x : x?.resultado === "fallo" ? Promise.reject(new Error(x.mensaje_corto)) : null;
        }, { plazo: 180_000, cada: 1500 });
        log(`Derivada: ${dv.mensaje_corto}`);
        const derivada = { ...{ RESTIC_REPOSITORY: path.join(carpetaNube, "Resguardo", `${repoE}-d1`), RESTIC_PASSWORD: otraClave } };
        const vistas = JSON.parse(ejecutar(resticBin, ["snapshots", "--json", "--no-lock"], { env: derivada }).salida || "[]") as unknown[];
        const enE = JSON.parse(ejecutar(resticBin, ["snapshots", "--json", "--no-lock"], { env: { RESTIC_REPOSITORY: path.join(carpetaE, accesoE.usuario, repoE), RESTIC_PASSWORD: contrasenaE } }).salida || "[]") as unknown[];
        comprobar(vistas.length > 0 && vistas.length === enE.length, "La derivada tiene las versiones de los últimos 30 días (todas) y se abre con SU contraseña", [vistas.length, enE.length]);
        const conLaDelOrigen = ejecutar(resticBin, ["snapshots", "--json", "--no-lock"], { env: { ...derivada, RESTIC_PASSWORD: contrasenaE } });
        comprobar(conLaDelOrigen.codigo !== 0, "La contraseña del origen no la abre");
        const rB = (await consola.equipo(c, eqB.id)).resumen?.repositorios?.find((x) => x.id === repoE);
        igual([rB?.derivadas?.[0]?.id, rB?.derivadas?.[0]?.cuando?.tras_copia, rB?.derivadas?.[0]?.filtro?.ultimos_dias], ["d1", true, 30], "El resumen de B lleva la derivada (sin rutas ni secretos)");
        comprobar(!JSON.stringify(rB).includes(otraClave) && !JSON.stringify(rB).includes(carpetaNube), "Ni la contraseña ni la carpeta en el resumen");

        // 4. Tarea 4a completa: un repositorio DIRECTAMENTE en la nube, como lo crea «+ Repositorio nuevo…»
        //    del editor de copias (lib/repoNuevo.ts), una copia de las carpetas a él y una restauración.
        const eqBNube = await consola.equipo(c, eqB.id);
        comprobar(eqBNube.resumen?.admite?.includes("repo_en_nube"), "B admite repositorios directamente en una nube", eqBNube.resumen?.admite);
        const opcion = opcionesRepoNuevo(eqBNube, [eqAAhora, eqBNube]).find((o) => o.nombre === "Nube de pruebas" && o.uso.ok);
        comprobar(opcion?.que.tipo === "nube" || opcion?.que.tipo === "propio", "«Nuevo repositorio» ofrece la nube conectada en B", opcion);
        const repoNube = `directo-nube-${randomBytes(2).toString("hex")}`;
        const claveNube = Buffer.from(aleatorio(32)).toString("base64url");
        const destNube = destinoNubeCuerpo(eqBNube, "Nube de pruebas", "Directo");
        await consola.hecha(c, eqB.id, "crear_repositorio", { id: repoNube, nombre: "Documentos en la nube", contrasena: claveNube, destino: destNube }, { claveAdmin: CLAVE_ADMIN }, {}, 120_000);
        comprobar(fs.existsSync(path.join(carpetaNube, "Directo", repoNube, "config")), "El repositorio está en la nube (la carpeta de la nube de pruebas)");
        const copiaNube = { ...copia, id: "documentos-nube", nombre: "Documentos a la nube", repo: repoNube };
        await consola.hecha(c, eqB.id, "config", { config: { v: 1, copias: [copia, copiaD, copiaTras, copiaNube] } }, { claveAdmin: CLAVE_ADMIN });
        const desdeNube = Date.now();
        await consola.hecha(c, eqB.id, "copiar_ahora", { copia: "documentos-nube", repo: repoNube });
        const ultimaNube = await esperar("que termine la copia directa a la nube", async () => {
          const k = (await consola.equipo(c, eqB.id)).resumen?.copias?.find((x) => x.id === "documentos-nube");
          return k?.ultima && new Date(k.ultima.cuando).getTime() >= desdeNube - 5_000 ? k.ultima : null;
        }, { plazo: 180_000, cada: 1000 });
        igual(ultimaNube.estado, "ok", `La copia directa a la nube terminó bien (${ultimaNube.mensaje ?? ""})`);
        const enNube = JSON.parse(ejecutar(resticBin, ["snapshots", "--json", "--no-lock"], { env: { RESTIC_REPOSITORY: path.join(carpetaNube, "Directo", repoNube), RESTIC_PASSWORD: claveNube } }).salida || "[]") as unknown[];
        comprobar(enNube.length === 1, "En la nube hay una versión y se abre con su contraseña", enNube.length);
        const secretosNube = { repo: { repo: repoNube, contrasena: claveNube } };
        const sesNube = new SesionE2E(consola, c);
        await sesNube.abrir(eqB.id, "explorar", { repo: repoNube }, secretosNube);
        const vNube = ((await sesNube.pedir("versiones")).versiones as { id: string }[])[0]?.id;
        await sesNube.cerrar();
        comprobar(!!vNube, "Explorar el repositorio de la nube enseña su versión");
        const facturaNube = path.join(datosB, "Facturas", "factura-001.txt");
        const antesNube = new Set(fs.readdirSync(path.join(datosB, "Facturas")));
        const restN = await consola.mandar(c, eqB.id, "restaurar", { repo: repoNube, version: vNube, rutas: [rutaRestic(facturaNube)], destino: "junto", reemplazar: false }, secretosNube);
        const rN = await consola.resultado(c, eqB.id, restN, { plazo: 120_000 });
        igual(rN.estado, "hecha", `Restaurar desde la nube: ${rN.mensaje}`);
        const nuevaCarpeta = fs.readdirSync(path.join(datosB, "Facturas")).find((n) => n.startsWith("Restaurado ") && !antesNube.has(n));
        comprobar(nuevaCarpeta && fs.readFileSync(path.join(datosB, "Facturas", nuevaCarpeta, "factura-001.txt"), "utf8") === fs.readFileSync(facturaNube, "utf8"), "El archivo restaurado desde la nube es el de la versión");
        if (nuevaCarpeta) fs.rmSync(path.join(datosB, "Facturas", nuevaCarpeta), { recursive: true, force: true });
        const rNube = (await consola.equipo(c, eqB.id)).resumen;
        comprobar(!JSON.stringify(rNube).includes(claveNube) && !JSON.stringify(rNube).includes(carpetaNube), "Ni la contraseña ni la carpeta de la nube en el resumen de B");
        // Se quita (los pasos siguientes cuentan los repositorios de B); lo de la nube se queda.
        await consola.hecha(c, eqB.id, "config", { config: { v: 1, copias: [copia, copiaD, copiaTras] } }, { claveAdmin: CLAVE_ADMIN });
        await consola.hecha(c, eqB.id, "quitar_repositorio", { repo: repoNube, quitar_destino: true }, { claveAdmin: CLAVE_ADMIN, ...secretosNube }, {}, 120_000);
        log(`B copió directamente a la nube («${repoNube}») y restauró desde ella`);
        // Quitar la derivada (espera; aquí de 0 s) y la nube; vuelve la configuración de antes (los pasos siguientes cuentan con una sola copia).
        // Plan 0.7.26 (1.1): desconectar la nube. Mientras la usa la derivada, el equipo no deja (y dice cuál).
        comprobar(nubesDelEquipo(await consola.equipo(c, eqB.id)).find((n) => n.nombre === "Nube de pruebas")?.usos.length, "La consola sabe qué usa la nube de B");
        const usada = await consola.resultado(c, eqB.id, await consola.mandar(c, eqB.id, "quitar_nube", { nombre: "Nube de pruebas" }, { claveAdmin: CLAVE_ADMIN }), { plazo: 60_000 });
        comprobar(usada.estado === "fallida" && /la usa una copia derivada/.test(usada.mensaje ?? ""), "Con la derivada puesta, B no deja desconectar la nube", usada);
        await consola.hecha(c, eqB.id, "quitar_derivada", { repo: repoE, id: "d1" }, secretosE);
        const fuera = await consola.resultado(c, eqB.id, await consola.mandar(c, eqB.id, "quitar_nube", { nombre: "Nube de pruebas" }, { claveAdmin: CLAVE_ADMIN }), { plazo: 60_000 });
        comprobar(fuera.estado === "hecha" && /credenciales borradas/.test(fuera.mensaje ?? ""), "Sin uso, la nube se desconecta y el equipo borra sus credenciales", fuera);
        comprobar(!(await consola.equipo(c, eqB.id)).resumen?.nubes?.some((n) => n.nombre === "Nube de pruebas"), "La nube ya no sale en el resumen de B");
        log(`Nube desconectada: ${fuera.mensaje}`);
      } else log("Sin rclone junto a restic: se salta la copia derivada por rclone.");
      await consola.hecha(c, eqA.id, "guarda_copias", { espejo: null }, { claveAdmin: CLAVE_ADMIN });
      await consola.hecha(c, eqB.id, "config", { config: { v: 1, copias: [copia] } }, { claveAdmin: CLAVE_ADMIN });

      // ---------------------------------------------------------------------
      paso("3d. Espejos como trabajos (plan 0.7.26, bloque 4): zona E → principal «igual que el origen», a una nube simulada «nunca borra», freno y su confirmación");
      const eqA4 = await consola.equipo(c, eqA.id);
      comprobar(["espejo_trabajos", "espejo_equipo"].every((x) => eqA4.resumen?.admite?.includes(x)), "A admite los trabajos de espejo y los del propio equipo", eqA4.resumen?.admite);
      // Un repositorio de mentira en la zona E: el espejo no abre nada, solo copia archivos con nombre de su huella.
      const usuarioE = accesoE.usuario;
      const repoFalso = `${usuarioE}/espejo-e2e`;
      const raizFalsa = path.join(carpetaE, usuarioE, "espejo-e2e");
      const antiguo = (p: string) => fs.utimesSync(p, new Date(Date.now() - 3600_000), new Date(Date.now() - 3600_000));
      const paquete = (i: number) => {
        const contenido = `paquete ${i} ${randomBytes(8).toString("hex")}`;
        const h = createHash("sha256").update(contenido).digest("hex");
        const rel = path.join("data", h.slice(0, 2), h);
        fs.mkdirSync(path.dirname(path.join(raizFalsa, rel)), { recursive: true });
        fs.writeFileSync(path.join(raizFalsa, rel), contenido);
        antiguo(path.join(raizFalsa, rel));
        return rel;
      };
      fs.mkdirSync(path.join(raizFalsa, "snapshots"), { recursive: true });
      fs.writeFileSync(path.join(raizFalsa, "config"), "config de prueba");
      antiguo(path.join(raizFalsa, "config"));
      const paquetes = Array.from({ length: 6 }, (_, i) => paquete(i));
      const carpetaNubeA = dir("nube-a");
      if (rclone) {
        fs.mkdirSync(carpetaNubeA, { recursive: true });
        await consola.hecha(c, eqA.id, "conectar_nube", { tipo: "alias", nombre: "Nube simulada", parametros: { carpeta: carpetaNubeA } }, { claveAdmin: CLAVE_ADMIN }, {}, 60_000);
      }
      // «Igual que el origen» con un freno estricto (para que salte con pocos archivos) y «nunca borra» después de él.
      const igualT: TrabajoEspejo = {
        id: "e2e-igual", nombre: "Zona E igual", activo: true, quien: "almacen", que: { tipo: "repos", repos: [repoFalso] }, zona: zona.id,
        adonde: { tipo: "zona", carpeta: "principal" }, cuando: { horario: { dias: [1, 2, 3, 4, 5, 6, 7], horas: ["00:01"] } },
        retencion: { modo: "igual" }, freno: { pct: 10, min_archivos: 0, min_faltan: 2, accion: "confirmar" }, orden: 0,
      };
      const nubeT: TrabajoEspejo = {
        id: "e2e-nube", nombre: "Nube nunca borra", activo: true, quien: "almacen", que: { tipo: "repos", repos: [repoFalso] }, zona: zona.id,
        adonde: { tipo: "nube", nube: "Nube simulada", carpeta: "Espejo" }, cuando: { despues: "e2e-igual" }, retencion: { modo: "nunca" }, freno: { ...FRENO_DEFECTO }, orden: 1,
      };
      const trabajos = rclone ? [igualT, nubeT] : [igualT];
      igual(errorTrabajos(trabajos, "almacen"), null, "La consola da por buenos los dos espejos");
      await consola.hecha(c, eqA.id, "guarda_copias", cuerpoAlmacen(trabajos), { claveAdmin: CLAVE_ADMIN });
      const enPrincipal = (rel: string) => fs.existsSync(path.join(almacen, usuarioE, "espejo-e2e", rel));
      const enNube = (rel: string) => fs.existsSync(path.join(carpetaNubeA, "Espejo", usuarioE, "espejo-e2e", rel));
      const trabajoDeA = async (id: string) => trabajosDelAlmacen(await consola.equipo(c, eqA.id)).find((t) => t.id === id);
      await esperar("el espejo «igual que el origen» en la zona principal", async () => (paquetes.every(enPrincipal) && enPrincipal("config") ? true : null), { plazo: 120_000, cada: 1000 });
      if (rclone) await esperar("el espejo a la nube, después del otro", async () => (paquetes.every(enNube) ? true : null), { plazo: 120_000, cada: 1000 });
      const visto = await esperar("los trabajos en el resumen de A, con su resultado", async () => {
        const t = await trabajoDeA("e2e-igual");
        return t?.resultado && !t.resultado.startsWith("ERROR") ? t : null;
      }, { plazo: 90_000, cada: 1500 });
      igual([visto.retencion.modo, visto.zona, visto.adonde.tipo], ["igual", zona.id, "zona"], "El resumen dice la retención, de qué zona copia y adónde");
      comprobar((await consola.equipo(c, eqA.id)).resumen?.guarda_copias?.espejo?.destinos?.some((d) => d.trabajo === "e2e-igual"), "…y la vista para una consola anterior");
      // Una orden de antes (por destinos) ya no puede cambiar estos espejos: el equipo la rechaza.
      const vieja = await consola.resultado(c, eqA.id, await consola.mandar(c, eqA.id, "guarda_copias", { espejo: { hora: "02:00", destinos: [{ tipo: "zona", carpeta: "principal", zona: zona.id }] } }, { claveAdmin: CLAVE_ADMIN }), { plazo: 60_000 });
      comprobar(vieja.estado === "fallida" && /consola actualizada/.test(vieja.mensaje ?? ""), "Una consola anterior no deshace los espejos nuevos", vieja);
      /** Una vuelta más de «igual que el origen» («Hacer ahora») y su resultado. */
      const pasada = async (que: string) => {
        const antes = (await trabajoDeA("e2e-igual"))?.ultima ?? null;
        await consola.hecha(c, eqA.id, "guarda_copias", { espejo_ahora: { trabajo: "e2e-igual" } }, { claveAdmin: CLAVE_ADMIN });
        return esperar(que, async () => {
          const t = await trabajoDeA("e2e-igual");
          return t?.ultima && t.ultima !== antes ? t : null;
        }, { plazo: 120_000, cada: 1000 });
      };
      // La poda del original quita un paquete: se anota en una vuelta y se borra en la siguiente.
      fs.rmSync(path.join(raizFalsa, paquetes[0]));
      const p1 = await pasada("la vuelta que ve faltar un paquete");
      comprobar(enPrincipal(paquetes[0]) && p1.por_borrar?.archivos === 1, "«Igual que el origen»: primero se anota", p1.por_borrar);
      const p2 = await pasada("la vuelta que lo borra");
      comprobar(!enPrincipal(paquetes[0]) && !p2.por_borrar?.archivos, "…y en la vuelta siguiente se borra del espejo", p2);
      if (rclone) {
        await dormir(12_000);
        comprobar(enNube(paquetes[0]), "La nube «nunca borra» lo sigue teniendo");
      }
      // Falta de golpe casi todo: salta el freno, no se borra nada y avisa.
      for (const rel of paquetes.slice(1)) fs.rmSync(path.join(raizFalsa, rel));
      const f1 = await pasada("la vuelta en la que salta el freno");
      comprobar(f1.resultado?.startsWith("ERROR") && /falta de golpe/.test(f1.freno_aviso ?? ""), "El freno salta y lo dice", f1);
      comprobar(paquetes.slice(1).every(enPrincipal), "Con el freno, no se borra nada");
      // Confirmado con la clave (espera; aquí de 0 s): se anota y se borra en la vuelta siguiente.
      await consola.hecha(c, eqA.id, "guarda_copias", { espejo_freno: { trabajo: "e2e-igual" } }, { claveAdmin: CLAVE_ADMIN });
      const f2 = await pasada("la vuelta tras confirmar el freno");
      comprobar(!f2.freno_aviso && f2.por_borrar?.archivos === 5, "Confirmado: se anota lo que falta", f2);
      await pasada("la vuelta que borra lo confirmado");
      comprobar(!paquetes.slice(1).some(enPrincipal) && enPrincipal("config"), "…y se borra (el repositorio sigue)");
      if (rclone) comprobar(paquetes.every(enNube), "La nube sigue con todo: nunca borra");
      log(`Espejos como trabajos: «igual que el origen» a la zona principal${rclone ? " y «nunca borra» a la nube simulada" : ""}, con freno confirmado`);
      await consola.hecha(c, eqA.id, "guarda_copias", { espejo: null }, { claveAdmin: CLAVE_ADMIN });
      if (rclone) await consola.hecha(c, eqA.id, "quitar_nube", { nombre: "Nube simulada" }, { claveAdmin: CLAVE_ADMIN }, {}, 60_000);
      for (const d of [raizFalsa, path.join(almacen, usuarioE, "espejo-e2e")]) fs.rmSync(d, { recursive: true, force: true });
    }

    // -----------------------------------------------------------------------
    paso("4. Restaurar un archivo desde la consola, «junto al original»");
    const sesion = new SesionE2E(consola, c);
    await sesion.abrir(eqB.id, "explorar", { repo: repoId }, secretosRepo);
    const versiones = (await sesion.pedir("versiones")).versiones as { id: string; cuando: string }[];
    comprobar(versiones.length >= 1, "La sesión enseña las versiones", versiones);
    const version = versiones[0].id;
    const raiz = (await sesion.pedir("listar", { version, ruta: rutaRestic(datosB) })).entradas as { nombre: string; tipo: string }[];
    const nombres = raiz.map((x) => x.nombre).sort();
    comprobar(nombres.includes("Facturas") && nombres.includes("notas.md"), "La versión tiene las carpetas y archivos copiados", nombres);
    comprobar(!nombres.includes("borrador.tmp") && !nombres.includes("cache"), "Las exclusiones (*.tmp, cache) no entraron en la copia", nombres);
    const facturas = (await sesion.pedir("listar", { version, ruta: rutaRestic(path.join(datosB, "Facturas")) })).entradas as { nombre: string }[];
    comprobar(facturas.some((x) => x.nombre === "factura-001.txt"), "La factura está en la versión", facturas);
    // v1.33: «Qué cambió», «Lo que más ocupa» y las versiones de un archivo, por la misma sesión cifrada.
    const dif = await sesion.pedir("diferencias", { hasta: version });
    if (dif.primera) igual(dif.desde, null, "La primera versión de su copia no tiene con qué compararse");
    else comprobar(Array.isArray(dif.cambios) && typeof (dif.resumen as { nuevos?: number })?.nuevos === "number", "«Qué cambió» trae los cambios y sus recuentos", dif);
    const ocupa = await sesion.pedir("ocupa", { version });
    comprobar((ocupa.total_archivos as number) > 0 && (ocupa.archivos as unknown[]).length > 0, "«Lo que más ocupa» cuenta los archivos de la versión", ocupa);
    const rutaFactura = `${rutaRestic(path.join(datosB, "Facturas"))}/factura-001.txt`;
    const hist = (await sesion.pedir("historial_archivo", { ruta: rutaFactura })).versiones as { version: string }[];
    comprobar(hist.some((x) => version.startsWith(x.version) || x.version.startsWith(version)), "Las versiones de la factura incluyen la última", hist);
    await sesion.cerrar();
    const original = path.join(datosB, "Facturas", "factura-001.txt");
    const contenido = fs.readFileSync(original, "utf8");
    fs.writeFileSync(original, "cambiada después de la copia\n");
    const rest = await consola.mandar(c, eqB.id, "restaurar", { repo: repoId, version, rutas: [rutaRestic(original)], destino: "junto", reemplazar: false }, secretosRepo);
    const rr = await consola.resultado(c, eqB.id, rest, { plazo: 120_000 });
    igual(rr.estado, "hecha", `Restaurar: ${rr.mensaje}`);
    comprobar(!(rr.mensaje ?? "").includes(datosB), "El mensaje del resultado va sin rutas del equipo", rr.mensaje);
    const restaurado = fs.readdirSync(path.join(datosB, "Facturas")).find((n) => n.startsWith("Restaurado "));
    comprobar(restaurado, "Hay una carpeta «Restaurado …» junto al original", fs.readdirSync(path.join(datosB, "Facturas")));
    igual(fs.readFileSync(path.join(datosB, "Facturas", restaurado, "factura-001.txt"), "utf8"), contenido, "El archivo restaurado es el de la versión");
    igual(fs.readFileSync(original, "utf8"), "cambiada después de la copia\n", "El original no se tocó");
    log(`Restaurado en «Facturas/${restaurado}»`);

    // -----------------------------------------------------------------------
    paso("4a. Actualización automática: la consola sirve una versión firmada (llave de PRUEBAS) y B «se actualiza»");
    {
      // docs/actualizaciones.md: la sustitución real del programa se prueba en máquinas virtuales
      // (§11); aquí el agente de desarrollo la simula (simular-actualizacion.txt) y todo lo demás
      // es de verdad: firma, servidor espejo, política, anillo, descarga con su SHA-256 e informe.
      const cargo = fs.readFileSync(path.join(RAIZ, "crates", "agente", "Cargo.toml"), "utf8");
      const actualV = /^version\s*=\s*"([^"]+)"/m.exec(cargo)![1].split("-")[0];
      const [ma, mi, pa] = actualV.split(".").map(Number);
      const nueva = `${ma}.${mi}.${pa + 1}`;
      const plataforma = WIN ? "windows-x86_64" : process.arch === "arm64" ? "linux-aarch64" : "linux-x86_64";
      const nombre = WIN ? `Resguardo-Agente_${nueva}_x64-setup.exe` : `resguardo-agente-${process.arch === "arm64" ? "aarch64" : "x86_64"}-linux-musl.tar.gz`;
      const paquete = randomBytes(200_000);
      const { createHash } = await import("node:crypto");
      const manifiesto = {
        formato: 1,
        producto: "resguardo-agente",
        version: nueva,
        fecha: new Date().toISOString().replace(/\.\d{3}Z$/, "Z"),
        canal: "estable",
        archivos: [{ plataforma, tipo: WIN ? "instalador-nsis" : "tar.gz", nombre, sha256: createHash("sha256").update(paquete).digest("hex"), tamano: paquete.length }],
      };
      const texto = JSON.stringify(manifiesto, null, 2) + "\n";
      const fixtures = fs.readFileSync(path.join(RAIZ, "crates", "protocolo", "tests", "fixtures", "LLAVES-DE-PRUEBAS-LEEME.txt"), "utf8");
      const firmaA = firmarPruebas(Buffer.from(texto), semillaPruebas(fixtures, "C57E2BA1129956D5"), "C57E2BA1129956D5");
      const firmaB = firmarPruebas(Buffer.from(texto), semillaPruebas(fixtures, "851E4472FB75946F"), "851E4472FB75946F");
      // El servidor no acepta nada que no firme la llave fijada (la B no lo está), ni un manifiesto cambiado.
      igual((await consola.pedir("PUT", "/api/servidor/publicacion", { manifiesto: texto, firma: firmaB })).estado, 422, "Firmada con otra llave: no se acepta");
      igual((await consola.pedir("PUT", "/api/servidor/publicacion", { manifiesto: texto.replace(nueva, `${nueva}9`), firma: firmaA })).estado, 422, "Manifiesto cambiado: no se acepta");
      await consola.ok("PUT", "/api/servidor/publicacion", { manifiesto: texto, firma: firmaA });
      const cambiado = Buffer.from(paquete);
      cambiado[0] ^= 1;
      igual((await consola.pedir("PUT", `/api/servidor/publicacion/${nueva}/${nombre}`, cambiado)).estado, 422, "Un archivo que no es el firmado: no se acepta");
      const pub = await consola.ok<{ vigente: { version: string } | null }>("PUT", `/api/servidor/publicacion/${nueva}/${nombre}`, paquete);
      igual(pub.vigente?.version, nueva, "El servidor da la versión firmada");
      const disponible = await consola.ok<{ disponible: { version: string } | null; equipos: Record<string, { anillo: string }> }>("GET", `/api/clientes/${c.id}/actualizaciones`);
      igual([disponible.disponible?.version, disponible.equipos[eqB.id]?.anillo], [nueva, "general"], "La consola del cliente la ve; B está en el anillo general");
      // B, en el anillo de prueba (le llega un toque por el canal): la busca, la comprueba, la baja y se «instala».
      fs.writeFileSync(path.join(B.dir, "simular-actualizacion.txt"), "ok");
      await consola.ok("PUT", `/api/clientes/${c.id}/equipos/${eqB.id}/anillo`, { anillo: "prueba" });
      const informeB = async () =>
        (await consola.ok<{ equipo: string; datos: any }[]>("GET", `/api/clientes/${c.id}/informes`)).find((x) => x.equipo === eqB.id)?.datos?.actualizacion;
      let visto: any = null;
      await esperar(
        "que B diga que se actualizó",
        async () => {
          visto = await informeB();
          return visto?.estado === "actualizada" && visto?.version_objetivo === nueva;
        },
        { plazo: 240_000, cada: 2_000 },
      ).catch((e) => {
        console.log(`Lo último de B: ${JSON.stringify(visto)}\n${B.ultimasLineas(30)}`);
        throw e;
      });
      const simulada = JSON.parse(fs.readFileSync(path.join(B.dir, "privado", "actualizacion", "simulada.json"), "utf8"));
      igual(simulada.sha256, manifiesto.archivos[0].sha256, "B bajó de la consola justo el archivo firmado");
      igual(visto.anillo, "prueba", "B dice su anillo");
      fs.rmSync(path.join(B.dir, "simular-actualizacion.txt"), { force: true });
      log(`B «se actualizó» a la ${nueva} (simulada) desde ${visto.origen}`);
    }

    // -----------------------------------------------------------------------
    paso("5. Retención en el almacén con plazos, sin fiarse de las horas que pone el equipo");
    // Versiones con otras horas, como las haría B (su usuario y su contraseña: solo añadir).
    const caAlmacen = dir("ca-almacen.pem");
    fs.writeFileSync(caAlmacen, acceso.destino.ca_pem);
    const repoUrl = `rest:${acceso.destino.donde.replace(/\/?$/, "/")}${repoId}`;
    const envRestic = { RESTIC_REPOSITORY: repoUrl, RESTIC_PASSWORD: contrasenaRepo, RESTIC_REST_USERNAME: acceso.destino.usuario, RESTIC_REST_PASSWORD: acceso.destino.secreto, RESTIC_CACERT: caAlmacen, RESTIC_CACHE_DIR: dir("cache-restic") };
    const comoB = (args: string[]) => {
      const r = ejecutar(resticBin, args, { env: envRestic, plazo: 300_000 });
      comprobar(r.codigo === 0, `restic ${args[0]} (como B) falló`, r.salida);
      return r.salida;
    };
    type Snap = { id: string; short_id: string; time: string; hostname: string; paths: string[] };
    const snapshots = () => JSON.parse(comoB(["snapshots", "--json", "--no-lock"])) as Snap[];
    const delAgente = snapshots();
    comprobar(delAgente.length >= 1, "La copia del agente está en el almacén", delAgente);
    const { hostname } = delAgente[0];
    const H = 3600_000;
    const confiables = [-1 * H, -3 * H, -4 * H, -5 * H];
    const falsas = [3 * 24 * H, -10 * 24 * H]; // hora futura, y una de hace 10 días subida ahora
    for (const d of [...confiables, ...falsas]) {
      fs.writeFileSync(path.join(datosB, "notas.md"), `# Notas\n${d} ${randomBytes(8).toString("hex")}\n`);
      comoB(["backup", datosB, "--host", hostname, "--time", horaLocal(new Date(Date.now() + d)), "--exclude", "*.tmp", "--exclude", "cache"]);
    }
    const todas = snapshots();
    comprobar(todas.every((s) => JSON.stringify(s.paths) === JSON.stringify(delAgente[0].paths)), "Todas en el mismo grupo (equipo y carpetas)", todas);
    // Lo que espera la consola (seQuedan, como el editor de retención) con las de confianza; las sospechosas no se tocan.
    const regla: Regla = { diarias: 0, semanales: 0, mensuales: 0, anuales: 0, plazos: { horarias: "2h" } };
    const ahoraMs = Date.now();
    const sospechosa = (s: Snap) => new Date(s.time).getTime() > ahoraMs + 24 * H || new Date(s.time).getTime() < ahoraMs - 48 * H;
    const buenas = todas.filter((s) => !sospechosa(s)).sort((a, b) => new Date(b.time).getTime() - new Date(a.time).getTime() || b.id.localeCompare(a.id));
    const quedan = seQuedan(buenas.map((s) => new Date(s.time)), regla);
    const esperadas = new Set([...buenas.filter((_, i) => quedan[i]).map((s) => s.id), ...todas.filter(sospechosa).map((s) => s.id)]);
    log(`${todas.length} versiones (${todas.filter(sospechosa).length} con hora falsa); según la consola quedan ${esperadas.size}`);
    comprobar(esperadas.size < todas.length && todas.filter(sospechosa).length === 2, "El caso tiene algo que quitar y dos sospechosas");

    // Como «Retención en el almacén» (RetencionAlmacen.svelte): la regla en el equipo, su clave para el almacén y la regla en el almacén.
    const eqBAhora = await consola.equipo(c, eqB.id);
    const eqAAhora = await consola.equipo(c, eqA.id);
    const repoResumen = eqBAhora.resumen!.repositorios!.find((r) => r.id === repoId)!;
    const en = almacenDe(repoResumen, destinoDe(eqBAhora.resumen?.destinos, repoResumen), [eqAAhora, eqBAhora]);
    comprobar(en && en.admite, "La consola reconoce el almacén del repositorio (y que sabe aplicar la retención)", { repoResumen, destinos: eqBAhora.resumen?.destinos });
    comprobar(eqBAhora.resumen?.admite?.includes("retencion_plazos") && eqAAhora.resumen?.admite?.includes("retencion_plazos"), "Los dos agentes admiten plazos");
    await consola.hecha(c, eqB.id, "cambiar_retencion", { repo: repoId, ...reglaParaOrden(regla) }, { ...secretosRepo, claveAdmin: CLAVE_ADMIN });
    const claveAlmacen = nuevaClave();
    await consola.hecha(c, eqB.id, "clave_almacen", { repo: repoId, clave: claveAlmacen }, { ...secretosRepo, claveAdmin: CLAVE_ADMIN });
    const horaRetencion = HH(new Date(Date.now() + 6 * H));
    await consola.hecha(c, eqA.id, "retencion_almacen", { usuario: en.usuario, repo: en.carpeta, clave: claveAlmacen, retencion: reglaParaOrden(regla), horario: { dias: [1, 2, 3, 4, 5, 6, 7], hora: horaRetencion }, verificar: true }, { claveAdmin: CLAVE_ADMIN });
    const enAlmacen = await esperar("la regla en el resumen del almacén, con la clave comprobada", async () => {
      const r = (await consola.equipo(c, eqA.id)).resumen?.guarda_copias?.retenciones?.find((x) => x.usuario === en.usuario && x.repo === en.carpeta);
      return r?.clave === "ok" ? r : null;
    }, { plazo: 30_000 });
    comprobar(!JSON.stringify(enAlmacen).includes(claveAlmacen), "El resumen no lleva la clave del almacén");
    const ap = await consola.mandar(c, eqA.id, "aplicar_retencion_almacen", { usuario: en.usuario, repo: en.carpeta }, { claveAdmin: CLAVE_ADMIN });
    const apr = await consola.resultado(c, eqA.id, ap, { plazo: 180_000 });
    igual(apr.estado, "hecha", `Aplicar la retención en el almacén: ${apr.mensaje}`);
    log(`Almacén: ${apr.mensaje}`);
    const despues = snapshots();
    igual(despues.map((s) => s.id).sort(), [...esperadas].sort(), "Las versiones que quedan son las que dice la consola (y las de hora falsa siguen ahí)");
    comprobar(/2 versiones con una fecha que no cuadra/.test(apr.mensaje ?? ""), "El mensaje cuenta las sospechosas", apr.mensaje);
    // Desde B (solo añadir) sigue sin poderse borrar nada.
    comprobar(ejecutar(resticBin, ["forget", despues[0].id], { env: envRestic }).codigo !== 0, "B no puede borrar versiones en el almacén");
    await esperar("el resultado en el resumen del almacén", async () => {
      const r = (await consola.equipo(c, eqA.id)).resumen?.guarda_copias?.retenciones?.find((x) => x.usuario === en.usuario && x.repo === en.carpeta);
      return r?.resultado === "ok" && r.versiones === despues.length ? r : null;
    }, { plazo: 60_000 });
    // v1.30: y el dueño (B) lo ve enseguida (pista `refrescar` del servidor), sin esperar a su próxima copia ni 6 h.
    const cortas = despues.map((s) => s.id.slice(0, 8)).sort();
    const trasAlmacen = Date.now();
    await esperar("las versiones que quedan en lo que ve la consola de B", async () => {
      const e = await consola.equipo(c, eqB.id);
      const r = e.resumen?.repositorios?.find((x) => x.id === repoId);
      const inf = informeDe(e.ultimo_informe as any, repoId);
      const ids = (inf?.versiones ?? []).map((v) => v.id).sort();
      return r && nVersiones(r, inf) === despues.length && JSON.stringify(ids) === JSON.stringify(cortas) ? r : null;
    }, { plazo: 45_000, cada: 1000 });
    log(`B cuenta ${despues.length} versiones ${((Date.now() - trasAlmacen) / 1000).toFixed(1)} s después del resultado del almacén`);

    // v1.45: «Retención en detalle»: lo que anotó el almacén en su historial es lo que quitó de verdad,
    // y la consola (retencionDetalle.ts, como la página) lo enseña con el repositorio de B.
    const quitadasDeVerdad = todas.filter((s) => !despues.some((d) => d.id === s.id)).map((s) => s.id.slice(0, 8)).sort();
    const infB = informeDe((await consola.equipo(c, eqB.id)).ultimo_informe as any, repoId);
    const deA = await esperar("lo que quitó la retención, en el historial del almacén", async () => {
      const l = (await consola.ok("GET", `/api/clientes/${c.id}/equipos/${eqA.id}/historial?tipo=retencion`)) as EntradaRetencion[];
      return l.some((x) => x.usuario === en.usuario && x.repo === en.carpeta) ? l : null;
    }, { plazo: 90_000, cada: 1000 });
    const vueltas = vueltasDelRepo({ propias: [], delAlmacen: deA, repo: repoId, enAlmacen: { usuario: en.usuario, carpeta: en.carpeta }, copiaDe: (id) => infB?.versiones.find((v) => v.id === id)?.copia ?? null });
    comprobar(vueltas.length === 1, "Una vez aplicada en el almacén (las de antes del paso 5 no hay)", vueltas);
    const vr = vueltas[0];
    igual(vr.versiones.map((x) => x.id).sort(), quitadasDeVerdad, "La retención en detalle enseña las versiones que quitó el almacén");
    igual([vr.origen, vr.por, vr.ok, vr.quitadas, vr.quedan, vr.antes, vr.sospechosas], ["almacen", "orden", true, quitadasDeVerdad.length, despues.length, todas.length, 2], "Quién, cuántas y las sospechosas");
    comprobar(vr.versiones.every((x) => x.motivo && x.motivo.tipo !== "restic" && x.hora), "Cada una con su hora y por qué no la guardó la regla (lo mismo que decidió el almacén)", vr.versiones);
    comprobar(vr.liberado !== null && vr.liberado > 0, "Con lo que liberó prune", vr);
    comprobar([datosB, almacen].every((p) => !JSON.stringify(deA).includes(JSON.stringify(p).slice(1, -1))), "Sin rutas", deA);
    // Sin `tipo`, el historial de siempre: una consola anterior no ve las de la retención.
    const sinTipo = (await consola.ok("GET", `/api/clientes/${c.id}/equipos/${eqA.id}/historial`)) as { tipo: string }[];
    comprobar(!sinTipo.some((x) => x.tipo === "retencion"), "Sin `tipo`, el historial no trae las de la retención", sinTipo.map((x) => x.tipo));
    log(`Retención en detalle: ${vr.versiones.length} versiones quitadas (${vr.versiones.map((x) => `${x.id} ${x.motivo?.tipo}:${x.motivo?.periodo} ${x.copia ?? "?"}`).join(", ")}), ${vr.liberado} bytes liberados`);

    // -----------------------------------------------------------------------
    paso("5a. «Buscar archivos» en todas las versiones (buscar_todas), por la sesión cifrada");
    {
      const sb = new SesionE2E(consola, c);
      await sb.abrir(eqB.id, "explorar", { repo: repoId }, secretosRepo);
      comprobar(sb.ops?.includes("buscar_todas"), "El agente anuncia «buscar_todas» en lista.ops", sb.ops);
      // Todas las páginas de una búsqueda, unidas como en la consola.
      const buscarTodo = async (args: Record<string, unknown>) => {
        const paginas: PaginaBusqueda[] = [];
        let indice: number | null = 0;
        while (indice != null) {
          const p = (await sb.pedir("buscar_todas", { ...args, indice })) as PaginaBusqueda;
          paginas.push(p);
          indice = typeof p.siguiente === "number" && p.siguiente > indice ? p.siguiente : null;
        }
        return unirBusqueda(repoId, paginas);
      };
      // «notas.md» cambió en cada versión del paso 5: está en todas las que quedan, con tamaños distintos.
      const r = await buscarTodo({ texto: "NOTAS" });
      const notas = r.archivos.find((a) => a.ruta === `${rutaRestic(datosB)}/notas.md`);
      comprobar(notas, "Encuentra notas.md (sin distinguir mayúsculas)", r.archivos.map((a) => a.ruta));
      igual(r.versionesBuscadas, despues.length, "Busca en todas las versiones del repositorio");
      igual(notas!.versiones.map((v) => v.version).sort(), cortas, "notas.md está en todas las versiones que quedan");
      comprobar(notas!.versiones.every((v, i, xs) => i === 0 || Date.parse(xs[i - 1].cuando) >= Date.parse(v.cuando)), "Sus versiones, la más reciente primero", notas!.versiones);
      comprobar(notas!.cambios >= 1 && notas!.ultimoCambio, "Sabe cuándo cambió por última vez", notas);
      comprobar(!r.recortado && r.motivo === null, "Sin recortes", r);
      const hist = (await sb.pedir("historial_archivo", { ruta: notas!.ruta })).versiones as { version: string; bytes: number }[];
      igual(notas!.versiones.map((v) => [v.version, v.bytes]), hist.map((v) => [v.version, v.bytes]), "Cuadra con «historial_archivo»");
      comprobar(!r.archivos.some((a) => a.nombre === "Facturas" || a.ruta.includes("/cache/")), "Solo archivos (ni carpetas ni lo excluido)", r.archivos.map((a) => a.ruta));
      // La factura, dentro de una carpeta: por una parte del nombre y con un rango de fechas.
      const ordenadas = [...despues].sort((a, b) => Date.parse(a.time) - Date.parse(b.time));
      const desde = ordenadas[1].time;
      const hasta = ordenadas[ordenadas.length - 2].time;
      const enRango = ordenadas.filter((s) => Date.parse(s.time) >= Date.parse(desde) && Date.parse(s.time) <= Date.parse(hasta));
      comprobar(ordenadas.length >= 3, "Hay versiones para acotar por fechas", ordenadas.length);
      const rf = await buscarTodo({ texto: "factura-0", desde, hasta });
      // (También la restaurada en el paso 4, en «Facturas/Restaurado …», que entró en las copias del paso 5.)
      const factura = rf.archivos.find((a) => a.ruta === `${rutaRestic(path.join(datosB, "Facturas"))}/factura-001.txt`);
      comprobar(factura && rf.archivos.every((a) => a.nombre === "factura-001.txt"), "Encuentra la factura por una parte del nombre", rf.archivos.map((a) => a.ruta));
      igual(rf.versionesBuscadas, enRango.length, "Con fechas, solo las versiones de esas fechas");
      igual(factura!.versiones.length, enRango.length, "La factura, en todas las de esas fechas");
      // Caracteres de los patrones: ni rompen restic ni encuentran de más.
      const rc = await buscarTodo({ texto: "otas[1]*" });
      igual(rc.archivos.length, 0, "«[», «]» y «*» se buscan como texto (nada se llama así)");
      const rmax = await buscarTodo({ texto: "notas", max: 1 });
      igual([rmax.coincidencias, rmax.recortado, rmax.motivo], [1, true, "limite"], "Con «max», recorta y dice por qué");
      const mal = await sb.pedir("buscar_todas", { texto: "x" }, true);
      comprobar(/de 2 a 100/.test(mal.error ?? ""), "Valida el texto", mal);
      const malFecha = await sb.pedir("buscar_todas", { texto: "notas", desde: "ayer" }, true);
      comprobar(/Fecha no válida/.test(malFecha.error ?? ""), "Valida las fechas", malFecha);
      await sb.cerrar();
      log(`Buscar archivos: notas.md en ${notas!.versiones.length} versiones (${notas!.cambios} cambios); la factura en ${factura!.versiones.length} de ${enRango.length} en el rango`);
    }

    // -----------------------------------------------------------------------
    paso("5b. Consola en vivo: la copia programada se ve empezar y sus cifras llegan sin recargar (canal en vivo)");
    {
      // El canal como lo abre el navegador: cookie de sesión y Origin de este servidor (desde otra web, no).
      const ajeno = await OyenteVivo.abrir(s1.url, c.id, consola.cookieSesion, s1.ca, "https://otra.example").then(
        () => null,
        (e) => e as { estado?: number },
      );
      igual(ajeno?.estado, 403, "El canal en vivo no se abre desde otra web");
      const oyente = await OyenteVivo.abrir(s1.url, c.id, consola.cookieSesion, s1.ca);
      try {
        await oyente.esperar("el saludo", (m) => m.t === "hola", { plazo: 10_000 });
        // Una hora de copia dentro de un minuto o dos (y al menos 5 min después de la anterior: el agente no copia antes).
        const minimo = Math.max(Date.now() + 75_000, antes + 5 * 60_000 + 20_000);
        const slot = new Date(Math.ceil(minimo / 60_000) * 60_000);
        const horas = [horaCopia, hm(slot)];
        const conVivo = { ...copia, horario: { ...copia.horario, horas, reglas: [{ tipo: "horas", dias: [1, 2, 3, 4, 5, 6, 7], horas }, copia.horario.reglas[1]] } };
        await consola.hecha(c, eqB.id, "config", { config: { v: 1, copias: [conVivo] } }, { claveAdmin: CLAVE_ADMIN });
        await oyente.esperar("que B subió su configuración", (m) => m.t === "config" && m.equipo === eqB.id, { plazo: 20_000 });
        // Algo nuevo que copiar (la copia solo guarda versión si hay cambios), y que dure unos
        // segundos con la subida limitada del modo discreto (el agente cuenta el progreso cada 5 s).
        fs.writeFileSync(path.join(datosB, "notas.md"), `# Notas\nantes de la copia programada ${randomBytes(4).toString("hex")}\n`);
        fs.mkdirSync(path.join(datosB, "Relleno-programada"), { recursive: true });
        for (let n = 0; n < 12; n++) fs.writeFileSync(path.join(datosB, "Relleno-programada", `parte-${n}.bin`), randomBytes(4 * 1024 * 1024));
        // Lo que ve la consola antes (como «Estado»: el resumen y los últimos informes).
        type ResumenCliente = { equipos: { id: string; resumen?: { copias?: { id: string; ultima?: { cuando: string } }[]; repositorios?: { id: string }[] } }[] };
        const cifras = async () => {
          const [res, infs] = await Promise.all([consola.ok<ResumenCliente>("GET", `/api/clientes/${c.id}/resumen`), consola.ok<{ equipo: string; recibido: string; datos: any }[]>("GET", `/api/clientes/${c.id}/informes`)]);
          const e = res.equipos.find((x) => x.id === eqB.id);
          const r = e?.resumen?.repositorios?.find((x) => x.id === repoId);
          const inf = informeDe(infs.find((x) => x.equipo === eqB.id) ?? null, repoId);
          return { ultima: e?.resumen?.copias?.find((x) => x.id === "documentos")?.ultima?.cuando ?? null, versiones: r ? nVersiones(r as any, inf) : 0 };
        };
        const previas = await cifras();
        log(`Copia programada a las ${hm(slot)} (en ${Math.round((slot.getTime() - Date.now()) / 1000)} s); la consola ve ${previas.versiones} versiones`);
        const desde = oyente.mensajes.length;
        const empieza = await oyente.esperar("que empieza la copia programada de B", (m) => m.t === "progreso" && m.equipo === eqB.id && m.estado === "empieza", {
          plazo: slot.getTime() - Date.now() + 60_000,
          desde,
        });
        const tarde = (empieza.llegada - slot.getTime()) / 1000;
        log(`La consola en vivo vio empezar la copia ${tarde.toFixed(1)} s después de su hora`);
        comprobar(tarde < 30, `La consola se entera de que la copia programada empezó sin recargar (${tarde.toFixed(1)} s)`);
        // Lo que pide la consola al oírlo: el progreso, con la copia en marcha.
        const enMarcha = await consola.ok<{ equipo: string; tareas: any[] }[]>("GET", `/api/clientes/${c.id}/progreso`);
        comprobar(
          enMarcha.some((x) => x.equipo === eqB.id && x.tareas.some((t) => t.tipo === "copia" && t.copia === "documentos")),
          "Al oír «empieza», el progreso trae la copia en marcha",
          enMarcha,
        );
        const termina = await oyente.esperar("que termina la copia programada de B", (m) => m.t === "progreso" && m.equipo === eqB.id && m.estado === "termina", {
          plazo: 180_000,
          desde: oyente.mensajes.indexOf(empieza) + 1,
        });
        // Como la consola: con cada aviso de B desde el final, vuelve a pedir las cifras, hasta que traen la copia nueva.
        let i = oyente.mensajes.indexOf(termina);
        let fresco: { aviso: (typeof oyente.mensajes)[number]; listo: number } | null = null;
        const limite = Date.now() + 30_000;
        while (!fresco) {
          const aviso = oyente.mensajes[i];
          const ahora = await cifras();
          if (ahora.ultima && ahora.ultima !== previas.ultima && ahora.versiones > previas.versiones) fresco = { aviso, listo: Date.now() };
          else {
            const sig = await oyente.esperar("las cifras nuevas de B", (m) => m.equipo === eqB.id && ["informe", "config", "progreso"].includes(m.t), { plazo: Math.max(1_000, limite - Date.now()), desde: i + 1 });
            i = oyente.mensajes.indexOf(sig);
          }
        }
        const tras = (fresco.listo - termina.llegada) / 1000;
        log(`Cifras nuevas en la consola ${tras.toFixed(1)} s después del final (al oír «${fresco.aviso.m.t}»)`);
        comprobar(tras < 10, `Las cifras de la copia programada llegan en segundos, sin recargar (${tras.toFixed(1)} s)`);
        comprobar(fresco.listo - fresco.aviso.llegada < 3_000, "Las cifras ya estaban al llegar el aviso (sin esperar a ningún sondeo)");
      } finally {
        oyente.cerrar();
      }
    }

    // -----------------------------------------------------------------------
    paso("6a. Notificaciones por correo: aviso de una copia fallida y «Volvió a funcionar»");
    const canal = await consola.ok("POST", "/api/servidor/notificaciones/canales", {
      tipo: "correo",
      nombre: "Correo de prueba",
      config: { host: "127.0.0.1", puerto: buzon.puerto, seguridad: "ninguna", remitente: "Resguardo <avisos@prueba.example>" },
      codigo: await consola.totp!.codigo(),
    });
    comprobar(canal.completo === true, "El canal de correo queda completo", canal);
    const prueba = await consola.ok("POST", `/api/servidor/notificaciones/canales/${canal.id}/prueba`);
    comprobar(prueba.ok, `«Enviar prueba»: ${prueba.mensaje}`, prueba);
    await esperar("el correo de prueba en el buzón", () => buzon.correos.find((m) => m.para.some((p) => p.includes(CORREO))), { plazo: 30_000 });
    // La copia de B falla: su carpeta ya no está.
    const fuera = `${datosB}-fuera`;
    fs.renameSync(datosB, fuera);
    const fallo = await copiarAhora(consola, c);
    igual(fallo.estado, "fallo", `La copia sin su carpeta falla (${fallo.mensaje})`);
    const aviso = await esperar("el correo del aviso (copia fallida)", () => buzon.correos.find((m) => /Falló la copia «Documentos» en «EQUIPO-B»/.test(m.asunto)), { plazo: 90_000 });
    log(`Correo: «${aviso.asunto}»`);
    comprobar(!aviso.datos.includes(datosB) && !aviso.datos.includes(acceso.destino.secreto), "El correo no lleva rutas ni secretos");
    const avisos = await consola.ok("GET", `/api/clientes/${c.id}/avisos?abiertos=1`);
    comprobar(avisos.some((a: any) => a.tipo === "copia_fallida" && a.equipo === eqB.id), "El aviso está en la lista de la consola", avisos);
    fs.renameSync(fuera, datosB);
    const otraVez = await copiarAhora(consola, c);
    igual(otraVez.estado, "ok", `La copia vuelve a funcionar (${otraVez.mensaje})`);
    const vuelve = await esperar("el correo «Volvió a funcionar»", () => buzon.correos.find((m) => /Volvió a funcionar la copia «Documentos» en «EQUIPO-B»/.test(m.asunto)), { plazo: 90_000 });
    log(`Correo: «${vuelve.asunto}»`);
    const registro = await consola.ok("GET", "/api/servidor/notificaciones/registro");
    comprobar(registro.some((e: any) => e.tipo === "recuperacion" && e.estado === "enviado"), "El registro de envíos tiene la recuperación enviada", registro);

    // -----------------------------------------------------------------------
    paso("6a2. «Orden destructiva pendiente»: se cierra al aplicarse o cancelarse, y la siguiente vuelve a avisar");
    // La de la retención del paso 5 ya se aplicó: su aviso está cerrado, así que esta avisa otra vez.
    const destructivas = () => buzon.correos.filter((m) => /Orden destructiva pendiente en «ALMACEN-A»/.test(m.asunto));
    const cancelar = async (o: { id: string }) => {
      await consola.ok("POST", `/api/clientes/${c.id}/ordenes/${o.id}/cancelar`);
      await consola.resultado(c, eqA.id, o as any, { estados: ["cancelada"], plazo: 10_000 });
    };
    const d1 = await consola.mandar(c, eqA.id, "aplicar_retencion_almacen", { usuario: en.usuario, repo: en.carpeta }, { claveAdmin: CLAVE_ADMIN }, { esperar: true });
    await esperar("el correo de la orden destructiva pendiente", () => destructivas().length === 1 || null, { plazo: 60_000 });
    log(`Correo: «${destructivas()[0].asunto}»`);
    await cancelar(d1);
    // El servidor la cierra en cuanto se cancela (sin «Volvió a funcionar»).
    await dormir(5_000);
    const d2 = await consola.mandar(c, eqA.id, "aplicar_retencion_almacen", { usuario: en.usuario, repo: en.carpeta }, { claveAdmin: CLAVE_ADMIN }, { esperar: true });
    await esperar("otro correo para la orden destructiva siguiente", () => destructivas().length === 2 || null, { plazo: 60_000 });
    await cancelar(d2);
    await dormir(5_000);
    comprobar(!buzon.correos.some((m) => /Orden destructiva/.test(m.asunto) && !/pendiente/.test(m.asunto)), "Ningún «Resuelto» ni «Volvió a funcionar» de una orden destructiva", buzon.correos.map((m) => m.asunto));
    const reg2 = await consola.ok("GET", "/api/servidor/notificaciones/registro");
    comprobar(!reg2.some((e: any) => e.tipo === "recuperacion" && /Orden destructiva/.test(e.titulo)), "El registro no tiene recuperación de una orden destructiva", reg2);

    // -----------------------------------------------------------------------
    paso("6b. Verificación automática y «Verificar ahora»");
    comprobar(eqBAhora.resumen?.admite?.includes("verificacion_auto"), "B admite la verificación automática");
    await consola.hecha(c, eqB.id, "config", { config: { v: 1, copias: [copia], verificaciones: { [repoId]: { cada_dias: 7, porcentaje: 25 } } } }, { claveAdmin: CLAVE_ADMIN });
    const va = await esperar("la verificación automática en el resumen", async () => (await consola.equipo(c, eqB.id)).resumen?.repositorios?.find((r) => r.id === repoId)?.verificacion_auto, { plazo: 30_000 });
    comprobar(va.cada_dias === 7 && va.porcentaje === 25 && va.proxima && new Date(va.proxima).getTime() > Date.now(), "Cada 7 días, 25 %, con su próxima vez", va);
    const antesVerificar = Date.now();
    await consola.hecha(c, eqB.id, "verificar_ahora", { repo: repoId });
    const ver = await esperar("el resultado de la verificación en el informe", async () => {
      const inf = informeDe((await consola.equipo(c, eqB.id)).ultimo_informe as any, repoId);
      return inf?.verificacion?.ultima && new Date(inf.verificacion.ultima).getTime() >= antesVerificar - 2_000 ? inf.verificacion : null;
    }, { plazo: 180_000, cada: 1000 });
    igual(ver.resultado, "ok", `Verificación (${ver.mensaje_corto ?? ""})`);
    // v1.40: con un horario de reglas (como el de las copias); `cada_dias` sigue para un agente anterior.
    comprobar(eqBAhora.resumen?.admite?.includes("verificacion_horario"), "B admite la verificación con horario");
    const horarioVerif = { dias: [], horas: [], reglas: [{ tipo: "mensual", dia: 1, hora: "04:00" }, { tipo: "horas", dias: [6, 7], horas: ["02:30"] }] };
    await consola.hecha(c, eqB.id, "config", { config: { v: 1, copias: [copia], verificaciones: { [repoId]: { cada_dias: 7, porcentaje: 25, horario: horarioVerif } } } }, { claveAdmin: CLAVE_ADMIN });
    const vh = await esperar("la verificación con horario en el resumen", async () => {
      const v = (await consola.equipo(c, eqB.id)).resumen?.repositorios?.find((r) => r.id === repoId)?.verificacion_auto as any;
      return v?.horario?.reglas?.length === 2 ? v : null;
    }, { plazo: 30_000 });
    comprobar(vh.porcentaje === 25 && vh.cada_dias >= 1 && vh.cada_dias <= 31 && vh.proxima && new Date(vh.proxima).getTime() > Date.now(), "Con horario: 25 %, un cada_dias para consolas anteriores y la próxima vez", vh);
    const enProx = new Date(vh.proxima);
    comprobar((enProx.getDate() === 1 && enProx.getHours() === 4) || ([0, 6].includes(enProx.getDay()) && enProx.getHours() === 2 && enProx.getMinutes() === 30), "La próxima, en una de sus reglas", vh.proxima);

    // v1.40: observaciones y comentarios (en el servidor, sin la clave); se comprueban tras restaurar la copia de la consola.
    await consola.ok("PUT", `/api/clientes/${c.id}/notas/observacion`, { tipo: "equipo", objeto: eqB.id, texto: "**Caja**: llamar a Luis si falla" });
    await consola.ok("POST", `/api/clientes/${c.id}/notas/comentarios`, { tipo: "repositorio", objeto: `${eqB.id}/${repoId}`, texto: "Verificación con horario puesta." });

    // -----------------------------------------------------------------------
    paso("6b2. Copia externa a un repositorio que ya existe (como la subida a la nube de la app de escritorio), con bloqueo");
    {
      comprobar(eqBAhora.resumen?.admite?.includes("externa_existente"), "B admite la copia externa a uno que ya existe");
      // «La nube»: un repositorio con el troceado del de B y sus versiones de hasta ahora (lo que subió la app antigua), con otra contraseña.
      const nube = dir("nube-copias");
      const claveNube = Buffer.from(aleatorio(24)).toString("base64url");
      const envNube = { ...envRestic, RESTIC_REPOSITORY: path.join(nube, "siigo"), RESTIC_PASSWORD: claveNube, RESTIC_FROM_REPOSITORY: repoUrl, RESTIC_FROM_PASSWORD: contrasenaRepo };
      for (const args of [["init", "--copy-chunker-params"], ["copy"]]) {
        const r = ejecutar(resticBin, args, { env: envNube, plazo: 300_000 });
        comprobar(r.codigo === 0, `restic ${args[0]} de «la nube» falló`, r.salida);
      }
      const enNube = () => (JSON.parse(ejecutar(resticBin, ["snapshots", "--json", "--no-lock"], { env: envNube }).salida) as Snap[]).length;
      const yaSubidas = enNube();
      // Algo nuevo en B (lo que falta allí).
      fs.writeFileSync(path.join(datosB, "notas.md"), `# Notas\nantes de la copia externa ${randomBytes(4).toString("hex")}\n`);
      const nueva = await copiarAhora(consola, c);
      igual(nueva.estado, "ok", `Una copia más en B (${nueva.mensaje ?? ""})`);
      const externa = {
        repo: repoId,
        destino: { id: `externa-${randomBytes(3).toString("hex")}`, tipo: "local", donde: nube },
        ruta: "siigo",
        existente: true,
        contrasena_destino: claveNube,
        hora: "03:00",
        retencion: reglaParaOrden({ diarias: 7, semanales: 4, mensuales: 12, anuales: 2 }),
        bloqueo_dias: 30,
      };
      // «Probar»: dice lo que hay y no guarda nada.
      const prueba = await consola.hecha(c, eqB.id, "cambiar_copia_externa", { ...externa, solo_probar: true }, secretosRepo);
      comprobar(/se abre con esa contraseña \(\d+ versiones?\)/.test(prueba.mensaje ?? "") && /Trocea igual/.test(prueba.mensaje ?? ""), `«Probar»: ${prueba.mensaje}`);
      comprobar(!(prueba.mensaje ?? "").includes(nube) && !(prueba.mensaje ?? "").includes(claveNube), "«Probar» no dice rutas ni contraseñas", prueba.mensaje);
      comprobar(!(await consola.equipo(c, eqB.id)).resumen?.repositorios?.find((r) => r.id === repoId)?.externa, "Probar no guardó la copia externa");
      // Con otra contraseña de «la nube»: no se abre y no se guarda nada.
      const mala = await consola.resultado(c, eqB.id, await consola.mandar(c, eqB.id, "cambiar_copia_externa", { ...externa, contrasena_destino: "no es esta" }, secretosRepo), { plazo: 120_000 });
      comprobar(mala.estado === "fallida" && /contraseña no abre/.test(mala.mensaje ?? ""), `Con otra contraseña, fallida: ${mala.mensaje}`);
      // Guardarla y subir ahora: solo lo nuevo.
      const guardada = await consola.hecha(c, eqB.id, "cambiar_copia_externa", externa, secretosRepo, {}, 120_000);
      log(`Copia externa: ${guardada.mensaje}`);
      const ext = await esperar("la copia externa en el resumen de B", async () => (await consola.equipo(c, eqB.id)).resumen?.repositorios?.find((r) => r.id === repoId)?.externa, { plazo: 20_000 });
      comprobar(ext.existente === true && ext.bloqueo_dias === 30 && ext.con_retencion === true, "El resumen dice: a uno que ya existía, bloqueo de 30 días, con retención", ext);
      const antesSubir = Date.now();
      await consola.hecha(c, eqB.id, "subir_ahora", { repo: repoId });
      const subida = await esperar("el resultado de la copia externa en el informe", async () => {
        const inf = informeDe((await consola.equipo(c, eqB.id)).ultimo_informe as any, repoId);
        return inf?.externa?.ultima && new Date(inf.externa.ultima).getTime() >= antesSubir - 2_000 ? inf.externa : null;
      }, { plazo: 180_000, cada: 1000 });
      igual(subida.resultado, "ok", `Copia externa (${subida.mensaje_corto ?? ""})`);
      comprobar(/copias? subidas? \(\d+ ya estaba/.test(subida.mensaje_corto ?? ""), "Solo subió lo que faltaba (lo demás ya estaba en el destino)", subida.mensaje_corto);
      const total = enNube();
      comprobar(total > yaSubidas, `«La nube» tiene lo nuevo (${yaSubidas} → ${total} versiones)`);
      // Sin repetir ninguna: las mismas que en el almacén.
      igual(total, snapshots().length, "Ni una versión repetida en «la nube»");
    }

    // -----------------------------------------------------------------------
    paso("6c. Copia de la consola, restaurarla en otra carpeta y que los equipos vuelvan solos");
    // v1.50 (9b): un ancla de antes de la copia (como la de un resumen por correo).
    const anclaAntes = anclaDe(c.id, ((await consola.ok("GET", `/api/clientes/${c.id}/auditoria?orden=desc&limite=1`)) as EntradaAuditoria[])[0]);
    const sal = salRespaldo();
    await consola.ok("PUT", "/api/servidor/respaldo", { activo: true, publica: await publicaRespaldo(argon2, CLAVE_RESPALDO, sal), sal });
    const hecho = await consola.ok("POST", "/api/servidor/respaldo/ahora");
    comprobar(hecho.ultima?.ok && hecho.ultima.archivo, "«Hacer ahora» hizo la copia", hecho);
    const archivo = path.join(s1.datos, "respaldos", hecho.ultima.archivo);
    comprobar(fs.existsSync(archivo), "El archivo de la copia está en respaldos/", archivo);
    const identidad = (await consola.ok("GET", "/api/servidor")).identidad as string;
    // Después de la copia, una orden más: el equipo va por delante de lo que recuerda la copia.
    const trasCopia = await copiarAhora(consola, c);
    igual(trasCopia.estado, "ok", "Copia después de la copia de la consola");
    // v1.50 (9b): el ancla de la auditoría. La de la copia de la consola y la de ahora (más entradas).
    const cabeza = async (k: Consola) => ((await k.ok("GET", `/api/clientes/${c.id}/auditoria?orden=desc&limite=1`)) as EntradaAuditoria[])[0];
    const anclaAhora = anclaDe(c.id, await cabeza(consola));
    comprobar(anclaAhora.n > anclaAntes.n, "Hay actividad después de la copia de la consola", [anclaAntes, anclaAhora]);
    // B vuelve a conectar y guarda el ancla de ahora (llega firmada en el saludo del canal).
    const anclasB = path.join(B.dir, "privado", "anclas-auditoria.json");
    await B.parar();
    B.arrancar();
    await esperar("que B guarde el ancla de ahora", async () => {
      if (!fs.existsSync(anclasB)) return null;
      const l = (JSON.parse(fs.readFileSync(anclasB, "utf8")).consolas?.[`${identidad}|${c.id}`] ?? []) as { n: number; hash: string }[];
      return l.at(-1)?.n === anclaAhora.n && l.at(-1)?.hash === anclaAhora.hash ? l : null;
    }, { plazo: 60_000, cada: 500 });
    await s1.parar();
    const datosRestaurados = dir("servidor-1-restaurado");
    const rest1 = ejecutar(servidorBin, ["restaurar-respaldo", archivo, "--datos", datosRestaurados, "--confiar-en", identidad], { env: { RESGUARDO_CLAVE_RESPALDO: CLAVE_RESPALDO } });
    comprobar(rest1.codigo === 0, "restaurar-respaldo", rest1.salida);
    const s1r = new Servidor("Servidor 1 (restaurado)", servidorBin, datosRestaurados, s1.puerto, path.join(registros, "servidor-1-restaurado.log"));
    await s1r.arrancar();
    const consolaR = new Consola(s1r);
    await consolaR.entrar(CORREO, CONTRASENA, consola.totp!);
    igual((await consolaR.ok("GET", "/api/servidor")).identidad, identidad, "La misma identidad (la que fijaron los equipos)");
    for (const e of [eqA, eqB]) {
      await esperar(`que ${e.nombre} vuelva a conectar con el servidor restaurado`, async () => (await consolaR.equipo(c, e.id)).conectado, { plazo: 120_000, cada: 1000 });
    }
    const canales = (await consolaR.ok("GET", "/api/servidor/notificaciones")).canales;
    const notasR = (await consolaR.ok("GET", `/api/clientes/${c.id}/notas`)).objetos as any[];
    comprobar(notasR.some((o) => o.tipo === "equipo" && o.titulo === "Caja: llamar a Luis si falla") && notasR.some((o) => o.tipo === "repositorio" && o.comentarios === 1), "Las observaciones y los comentarios vinieron en la copia", notasR);
    comprobar(canales.length === 1 && canales[0].completo, "El canal de correo vino en la copia", canales);
    // Una orden desde el servidor restaurado (que recuerda un número de orden anterior).
    const tras = await copiarAhora(consolaR, c);
    igual(tras.estado, "ok", "Copia pedida desde el servidor restaurado");
    // 9b: la actividad del servidor restaurado vuelve atrás. B lo nota con el ancla que guardó y
    // lo cuenta en su historial: aviso «auditoria_rehecha» («Este servidor…»).
    await esperar("el aviso «auditoria_rehecha» de B en el servidor restaurado", async () => {
      const av = (await consolaR.ok("GET", `/api/clientes/${c.id}/avisos`)) as any[];
      return av.find((a) => a.tipo === "auditoria_rehecha" && a.equipo === eqB.id && /^Este servidor rehízo/.test(a.mensaje)) ?? null;
    }, { plazo: 90_000, cada: 1000 });
    const rehechaB = ((await consolaR.ok("GET", `/api/clientes/${c.id}/equipos/${eqB.id}/historial?tipo=auditoria_rehecha`)) as any[])[0];
    comprobar(rehechaB?.antes?.n === anclaAhora.n && rehechaB?.antes?.hash === anclaAhora.hash && !JSON.stringify(rehechaB).includes("http"), "B cuenta qué ancla no cuadró (sin la dirección de la consola)", rehechaB);
    // «Comprobar con un ancla» (lo mismo que hace la consola, lib/auditoria.ts) con la actividad del restaurado.
    const actividadR: EntradaAuditoria[] = [];
    for (let desde = 0; ; ) {
      const p = (await consolaR.ok("GET", `/api/clientes/${c.id}/auditoria?desde=${desde}&limite=1000`)) as EntradaAuditoria[];
      actividadR.push(...p);
      if (p.length < 1000) break;
      desde = p[p.length - 1].n;
    }
    igual(comprobarAncla(actividadR, anclaAntes).estado, "bien", "El ancla de antes de la copia cuadra con el servidor restaurado");
    comprobar(comprobarAncla(actividadR, anclaAhora).estado !== "bien", "El ancla de después no cuadra (la actividad volvió atrás)", comprobarAncla(actividadR, anclaAhora));
    comprobar(leerAncla(`Ancla: ${lineaAncla(anclaAntes)}`)?.hash === anclaAntes.hash, "La línea del ancla se lee", lineaAncla(anclaAntes));

    // -----------------------------------------------------------------------
    paso("7. Volver a vincular B con otro servidor (misma clave de administración): sube su historial");
    const s2 = new Servidor("Servidor 2", servidorBin, dir("servidor-2"), await puertoLibre(), path.join(registros, "servidor-2.log"));
    await s2.arrancar();
    const consola2 = new Consola(s2);
    await consola2.primerArranque(CORREO, "Ana", CONTRASENA);
    const c2: Cliente = await consola2.ok("POST", "/api/clientes", { nombre: "Café del Sur", espera_min_horas: 1 });
    Object.assign(c2, await consola2.ok("GET", `/api/clientes/${c2.id}`));
    const eqB2 = await consola2.emparejar(c2, B, CLAVE_ADMIN, "navegador");
    const historial = (await esperar("el historial de B en el servidor nuevo", async () => {
      const h = (await consola2.ok("GET", `/api/clientes/${c2.id}/equipos/${eqB2.id}/historial?limite=500`)) as any[];
      return h.filter((x) => x.tipo === "copia").length >= 4 ? h : null;
    }, { plazo: 120_000, cada: 1000 })) as any[];
    const copiasH = historial.filter((x) => x.tipo === "copia");
    comprobar(copiasH.some((x) => x.resultado === "fallo") && copiasH.some((x) => x.resultado === "ok"), "El historial trae las vueltas de antes (también la que falló)", copiasH);
    comprobar(historial.some((x) => x.tipo === "verificacion"), "Y la verificación", historial);
    log(`Historial en el servidor nuevo: ${historial.length} entradas (${copiasH.length} vueltas de copia)`);
    // La consola nueva ve el repositorio con sus versiones sin esperar al informe de 5 minutos.
    await esperar("el repositorio con versiones en el servidor nuevo", async () => {
      const e = await consola2.equipo(c2, eqB2.id);
      const r = e.resumen?.repositorios?.find((x) => x.id === repoId);
      return r && nVersiones(r, informeDe(e.ultimo_informe as any, repoId)) > 0;
    }, { plazo: 60_000, cada: 1000 });

    // -----------------------------------------------------------------------
    paso("8. Varias consolas a la vez: B se gestiona también desde un tercer servidor (otra sal); quitar una deja la otra");
    const s3 = new Servidor("Servidor 3 (en línea)", servidorBin, dir("servidor-3"), await puertoLibre(), path.join(registros, "servidor-3.log"));
    await s3.arrancar();
    const consola3 = new Consola(s3);
    await consola3.primerArranque(CORREO, "Ana", CONTRASENA);
    // En la consola en línea: «Recibir un cliente → Gestionarlo también desde aquí» (cliente nuevo, sal nueva).
    const recibido = await consola3.ok("POST", "/api/clientes/recibir", { nombre: "Café del Sur", sal_cliente: aB64(aleatorio(16)), usos: 5, dias: 7 });
    const srv3 = await consola3.ok("GET", "/api/servidor");
    const codigo = crearCodigo({
      url: s3.url, identidad: srv3.identidad, huella_ca: srv3.huella_ca, ficha: recibido.ficha, sal_cliente: recibido.cliente.sal_cliente,
      nombre: "Consola en línea", cliente: "Café del Sur", caduca: recibido.caduca,
    });
    log(`Código de conexión: ${codigo.length} caracteres`);
    // En la consola de siempre (servidor 2): pegar el código y mandar anadir_consola con la K_cfg de la otra.
    const leido = leerCodigo(codigo, new Date(), s2.url);
    comprobar(typeof leido !== "string", "El código de conexión se lee", leido);
    // La clave de B (cambia en el paso 8b); la K_cfg de la otra consola, con su sal y esa clave.
    let claveB = CLAVE_ADMIN;
    const anadir = async () => cuerpoAnadir(leido, aB64(kCfg(await materialCliente(argon2, claveB, leido.sal_cliente))), c2.sal_cliente);
    const conectada = await consola2.hecha(c2, eqB2.id, "anadir_consola", await anadir(), { claveAdmin: claveB }, {}, 120_000);
    log(`anadir_consola: ${conectada.mensaje}`);
    const c3: Cliente = await consola3.ok("GET", `/api/clientes/${recibido.cliente.id}`);
    await esperar("que B se conecte también al servidor 3", async () => {
      const e = await consola3.equipo(c3, eqB2.id);
      return e.confirmado && e.conectado && e.resumen?.consolas?.length === 2;
    }, { plazo: 90_000, cada: 1000 });
    log("B conectado también a la consola en línea");
    // La consola en línea ve su configuración (con SU K_cfg) y el historial de antes.
    const hist3 = (await consola3.ok("GET", `/api/clientes/${c3.id}/equipos/${eqB2.id}/historial?limite=500`)) as any[];
    comprobar(hist3.some((x) => x.tipo === "copia"), "El historial de B llega también a la consola en línea", hist3.length);
    // Órdenes de las dos (cada una con su seq): una desde la en línea…
    await consola3.hecha(c3, eqB2.id, "cambiar_espera", { horas: 2 }, { claveAdmin: claveB });
    log("Orden desde la consola en línea: hecha");
    // … y la otra consola se entera sola (la espera va con la configuración) y ve de dónde vino.
    await esperar("que el servidor 2 sepa la espera que puso la consola en línea", async () => (await consola2.equipo(c2, eqB2.id)).espera_min_horas === 2, { plazo: 60_000, cada: 1000 });
    await esperar("«Cambiado desde otra consola» en el servidor 2", async () => (await consola2.equipo(c2, eqB2.id)).resumen?.cambio_config?.consola.identidad === srv3.identidad, { plazo: 60_000, cada: 1000 });
    log("La consola local ve el cambio de la en línea");
    await consola2.hecha(c2, eqB2.id, "cambiar_espera", { horas: 3 }, { claveAdmin: claveB });
    await esperar("la espera de la consola local en la en línea", async () => (await consola3.equipo(c3, eqB2.id)).espera_min_horas === 3, { plazo: 60_000, cada: 1000 });

    // Tarea 2 («Equipos que no están en todas las consolas»): la consola local sabe, por el
    // resumen de B, que el cliente también está en la en línea (lib/consolasCliente.ts).
    const eB8 = await consola2.equipo(c2, eqB2.id);
    const otras8 = otrasConsolas([eB8], Date.now());
    comprobar(otras8.length === 1 && otras8[0].identidad === srv3.identidad && otras8[0].nombre === "Consola en línea" && !otras8[0].sin.length, "La local sabe que el cliente está también en la en línea (y B no falta)", otras8);
    // Un equipo nuevo del cliente que solo estuviera aquí faltaría allí, con la frase del alta.
    const soloAqui: Equipo = { ...eB8, id: "equipo-nuevo", nombre: "PORTATIL-NUEVO", resumen: { admite: ["consolas_multiples"], consolas: eB8.resumen!.consolas!.filter((x) => x.esta) } };
    const conNuevo = otrasConsolas([eB8, soloAqui], Date.now());
    igual(conNuevo[0].sin.map((e) => e.id), ["equipo-nuevo"], "El equipo que solo está aquí falta en la en línea");
    igual(fraseEquipo(soloAqui, conNuevo[0], [eB8, soloAqui]), "Este equipo solo está en esta consola; los demás también están en «Consola en línea».", "La frase del alta");
    // Repetir «Conectar también» con un equipo que ya está allí es inofensivo: el equipo
    // contesta «ya gestiona este equipo» y nada cambia (ni allí ni aquí).
    const equipos3 = ((await consola3.ok("GET", `/api/clientes/${c3.id}/equipos`)) as Equipo[]).length;
    const repetida = await consola2.resultado(c2, eqB2.id, await consola2.mandar(c2, eqB2.id, "anadir_consola", await anadir(), { claveAdmin: claveB }));
    comprobar(repetida.estado === "fallida" && /ya gestiona este equipo/.test(repetida.mensaje ?? ""), "Repetir anadir_consola: «ya gestiona este equipo»", repetida);
    igual(((await consola3.ok("GET", `/api/clientes/${c3.id}/equipos`)) as Equipo[]).length, equipos3, "La en línea sigue con los mismos equipos");
    const eB8b = await consola3.equipo(c3, eqB2.id);
    comprobar(eB8b.conectado && eB8b.resumen?.consolas?.length === 2, "B sigue conectado a las dos", eB8b.resumen?.consolas);
    await consola3.hecha(c3, eqB2.id, "cambiar_espera", { horas: 3 }, { claveAdmin: claveB });
    log("Repetir «Conectar también» no cambia nada; la en línea sigue mandando");

    // -----------------------------------------------------------------------
    paso("8a. «Mover a otro sitio…» desde la consola local: la en línea lo ve (progreso y, al terminar, en el historial)");
    // Como MoverRepositorio.svelte: el repositorio nuevo con el mismo troceado y luego el historial, con `mover`.
    const destinoAlmacen = `almacen-${eqA.id.slice(0, 8)}`;
    const nuevoId = `movido-${randomBytes(2).toString("hex")}`;
    await consola2.hecha(
      c2,
      eqB2.id,
      "crear_repositorio",
      { id: nuevoId, nombre: "Copias movidas", destino: { id: destinoAlmacen }, contrasena: Buffer.from(aleatorio(32)).toString("base64url"), parametros_de: { repo: repoId } },
      { claveAdmin: claveB },
      {},
      120_000,
    );
    // Mientras se trae, la en línea pregunta su progreso (puede acabar antes de que llegue: solo se anota).
    let vistoEnLinea: any = null;
    let mirando = true;
    const mirar = (async () => {
      while (mirando && !vistoEnLinea) {
        const p = (await consola3.ok("GET", `/api/clientes/${c3.id}/progreso`).catch(() => [])) as any[];
        vistoEnLinea = p.flatMap((x) => x.tareas ?? []).find((t: any) => t.tipo === "historial" && t.mover) ?? null;
        await dormir(250);
      }
    })();
    const movido = await consola2.hecha(c2, eqB2.id, "copiar_historial", { repo: nuevoId, origen: { repo: repoId }, mover: { paso: "historial" } }, { claveAdmin: claveB }, {}, 300_000);
    mirando = false;
    await mirar;
    log(`copiar_historial (mover): ${movido.mensaje}`);
    if (vistoEnLinea) {
      comprobar(vistoEnLinea.otra_consola === true && vistoEnLinea.origen === repoId && vistoEnLinea.repo === nuevoId, "La en línea lo ve como de otra consola, de qué repositorio a cuál", vistoEnLinea);
      comprobar(!JSON.stringify(vistoEnLinea).includes(s2.url), "Sin la dirección de la otra consola", vistoEnLinea);
      log("La consola en línea vio el movimiento en marcha");
    } else log("(El historial se trajo antes de que la consola en línea viera su progreso: se comprueba el historial)");
    const entrada = (await esperar("«Mover a otro sitio» en el historial de B en la consola en línea", async () => {
      const h = (await consola3.ok("GET", `/api/clientes/${c3.id}/equipos/${eqB2.id}/historial?limite=50`)) as any[];
      return h.find((x) => x.tipo === "historial" && x.mover && x.repo === nuevoId) ?? null;
    }, { plazo: 120_000, cada: 1000 })) as any;
    comprobar(entrada.resultado === "ok" && entrada.origen === repoId && entrada.paso === "historial", "La entrada del historial dice de dónde a dónde y cómo acabó", entrada);
    // Y en la consola que lo mandó, igual.
    await esperar("la misma entrada en la consola local", async () => {
      const h = (await consola2.ok("GET", `/api/clientes/${c2.id}/equipos/${eqB2.id}/historial?limite=50`)) as any[];
      return h.some((x) => x.id === entrada.id) || null;
    }, { plazo: 60_000, cada: 1000 });
    // Terminado: ya no está en marcha en ninguna.
    await esperar("que el movimiento ya no salga en marcha en la en línea", async () => {
      const p = (await consola3.ok("GET", `/api/clientes/${c3.id}/progreso`)) as any[];
      return !p.flatMap((x) => x.tareas ?? []).some((t: any) => t.tipo === "historial") || null;
    }, { plazo: 30_000, cada: 1000 });

    // -----------------------------------------------------------------------
    paso("8a2. Órdenes en espera: la en línea ve una destructiva de la local, la cancela y nunca se aplica; otra se aplica a su hora");
    // v1.49 (docs/consolas-multiples.md §5). Con su espera de verdad (3 h): el equipo la recibe ya y la guarda.
    const srv2Id = (await consola2.ok("GET", "/api/servidor")).identidad;
    const pausa = await consola2.mandar(c2, eqB2.id, "pausar", {}, { claveAdmin: claveB }, { esperar: true });
    const enEspera3 = (await esperar("la orden en espera de la local en el resumen de la en línea", async () => {
      const e = await consola3.equipo(c3, eqB2.id);
      return (e.resumen?.en_espera ?? []).find((x: any) => x.id === pausa.id) ?? null;
    }, { plazo: 60_000, cada: 1000 })) as any;
    comprobar(enEspera3.consola.esta === false && enEspera3.consola.identidad === srv2Id && enEspera3.por === "Ana", "La en línea sabe desde qué consola vino y quién la pidió", enEspera3);
    comprobar(!JSON.stringify(enEspera3).includes(s2.url), "Sin la dirección de la otra consola", enEspera3);
    igual((await consola2.resultado(c2, eqB2.id, pausa, { estados: ["entregada"], plazo: 30_000 })).estado, "entregada", "En la local, entregada (el equipo la tiene en espera)");
    await esperar("el aviso «Orden en espera desde otra consola» en la en línea", async () => {
      const av = (await consola3.ok("GET", `/api/clientes/${c3.id}/avisos?abiertos=1`)) as any[];
      return av.some((a) => a.tipo === "orden_en_espera") || null;
    }, { plazo: 60_000, cada: 1000 });
    log("La consola en línea ve la orden de la local y recibió el aviso");
    // La en línea la cancela (inofensiva, sin clave) y la local se entera.
    const cancelada = await consola3.hecha(c3, eqB2.id, "cancelar_espera", { id: pausa.id });
    log(`cancelar_espera: ${cancelada.mensaje}`);
    // `rechazada` firmada por el equipo, con `detalle.cancelada` (la firma comprobada como en la consola).
    const enLocal = await consola2.resultado(c2, eqB2.id, pausa, { estados: ["rechazada"], plazo: 60_000 });
    comprobar(/otra consola/.test(enLocal.mensaje ?? "") && /"cancelada":true/.test(enLocal.detalle ?? ""), "La local la ve cancelada desde otra consola", enLocal);
    await esperar("la cancelación en el historial común (en la local)", async () => {
      const h = (await consola2.ok("GET", `/api/clientes/${c2.id}/equipos/${eqB2.id}/historial?tipo=orden&limite=100`)) as any[];
      return h.find((x) => x.orden_id === pausa.id && x.resultado === "cancelada" && x.cancelada_desde) ?? null;
    }, { plazo: 60_000, cada: 1000 });
    comprobar(!((await consola2.equipo(c2, eqB2.id)).resumen?.pausado_hasta), "La pausa nunca se aplicó");
    // Otra destructiva con una espera corta (pausar 1 h): no se aplica antes de tiempo, y sí a su hora.
    const pausaCorta = await consola2.mandar(c2, eqB2.id, "pausar", { horas: 1 }, { claveAdmin: claveB }, { esperaS: 25 });
    await dormir(8_000);
    igual((await consola2.resultado(c2, eqB2.id, pausaCorta, { estados: ["entregada"], plazo: 10_000 })).estado, "entregada", "Antes de su hora sigue en espera");
    comprobar(!((await consola2.equipo(c2, eqB2.id)).resumen?.pausado_hasta), "…y no se aplicó");
    const aplicada = await consola2.resultado(c2, eqB2.id, pausaCorta, { plazo: 90_000 });
    igual(aplicada.estado, "hecha", `A su hora se aplica (${aplicada.mensaje})`);
    await esperar("la pausa en la en línea", async () => !!(await consola3.equipo(c3, eqB2.id)).resumen?.pausado_hasta || null, { plazo: 60_000, cada: 1000 });
    await esperar("la orden aplicada en el historial común (en la en línea)", async () => {
      const h = (await consola3.ok("GET", `/api/clientes/${c3.id}/equipos/${eqB2.id}/historial?tipo=orden&limite=100`)) as any[];
      return h.find((x) => x.orden_id === pausaCorta.id && x.resultado === "hecha" && x.identidad === srv2Id) ?? null;
    }, { plazo: 60_000, cada: 1000 });
    // Y se reanuda (inofensiva: al momento) para lo que sigue.
    await consola2.hecha(c2, eqB2.id, "reanudar", {});
    log("Órdenes en espera entre consolas: bien");

    // -----------------------------------------------------------------------
    paso("8a3. Lo que comparten las consolas: nombre, etiquetas y observación del equipo; quitar un destino que se queda vacío");
    {
      // Convivencia: cada consola con su nombre (como antes de actualizar) y nada se pisa
      // mientras nadie lo cambie con la orden.
      await consola3.ok("PATCH", `/api/clientes/${c3.id}/equipos/${eqB2.id}`, { nombre: "B (en línea)" });
      const antes2 = (await consola2.equipo(c2, eqB2.id)).nombre;
      comprobar((await consola2.equipo(c2, eqB2.id)).resumen?.admite?.includes("datos_equipo"), "B admite los datos compartidos");
      comprobar(!(await consola3.equipo(c3, eqB2.id)).resumen?.datos_equipo, "Nadie ha puesto aún los datos del equipo");
      // Renombrar en la local: lo ve la en línea (el servidor lo copia del resumen).
      const nombre = await consola2.hecha(c2, eqB2.id, "nombre_equipo", { nombre: "B Recepción" });
      log(`nombre_equipo: ${nombre.mensaje}`);
      comprobar(antes2 !== "B Recepción", "El nombre de antes era otro", antes2);
      await esperar("el nombre nuevo en la en línea", async () => (await consola3.equipo(c3, eqB2.id)).nombre === "B Recepción", { plazo: 60_000, cada: 1000 });
      igual((await consola2.equipo(c2, eqB2.id)).nombre, "B Recepción", "Y en la local");
      const quien = (await consola3.equipo(c3, eqB2.id)).resumen?.datos_equipo?.nombre;
      comprobar(quien?.esta === false, "La en línea sabe que lo cambió otra consola", quien);
      await esperar("el cambio de nombre en el historial común de la en línea", async () => {
        const h = (await consola3.ok("GET", `/api/clientes/${c3.id}/equipos/${eqB2.id}/historial?tipo=orden&limite=100`)) as any[];
        return h.some((x) => x.orden === "nombre_equipo" && x.resultado === "hecha" && x.identidad !== srv3.identidad) || null;
      }, { plazo: 60_000, cada: 1000 });
      // Etiquetas y observación desde la en línea: llegan a la local.
      await consola3.hecha(c3, eqB2.id, "etiquetas_equipo", { etiquetas: ["Recepción", "Sede norte"] });
      await consola3.hecha(c3, eqB2.id, "observacion_equipo", { texto: "Disco cambiado el 3/10" });
      await esperar("las etiquetas de la en línea en la local", async () => JSON.stringify((await consola2.equipo(c2, eqB2.id)).etiquetas) === JSON.stringify(["Recepción", "Sede norte"]), { plazo: 60_000, cada: 1000 });
      await esperar("la observación de la en línea en la local", async () => {
        const n = await consola2.ok("GET", `/api/clientes/${c2.id}/notas/objeto?tipo=equipo&objeto=${eqB2.id}`);
        return n.observacion?.texto === "Disco cambiado el 3/10" || null;
      }, { plazo: 60_000, cada: 1000 });
      log("Nombre, etiquetas y observación iguales en las dos consolas");

      // Quitar un repositorio de un disco del propio equipo con «Quitar también el destino»: el
      // destino se va del equipo (en las dos consolas) y lo guardado en su carpeta se queda.
      const carpeta = dir("disco-del-equipo");
      fs.mkdirSync(carpeta, { recursive: true });
      const idDestino = `local-${randomBytes(3).toString("hex")}`;
      const idRepo = `prueba-${randomBytes(2).toString("hex")}`;
      const contrasena = Buffer.from(aleatorio(24)).toString("base64url");
      await consola2.hecha(c2, eqB2.id, "crear_repositorio", { id: idRepo, nombre: "En el mismo disco", destino: { id: idDestino, nombre: "Disco o carpeta del equipo", tipo: "local", donde: carpeta }, contrasena }, { claveAdmin: claveB }, {}, 120_000);
      await esperar("el destino nuevo en la en línea", async () => (await consola3.equipo(c3, eqB2.id)).resumen?.destinos?.some((d) => d.id === idDestino) || null, { plazo: 60_000, cada: 1000 });
      // Con un repositorio dentro no se puede quitar (v1.59: con la clave de administración).
      const enUso = await consola3.resultado(c3, eqB2.id, await consola3.mandar(c3, eqB2.id, "quitar_destino", { destino: idDestino }, { claveAdmin: claveB }));
      comprobar(enUso.estado === "fallida" && /En el mismo disco/.test(enUso.mensaje ?? ""), "Un destino con un repositorio no se quita", enUso);
      const quitado = await consola2.hecha(c2, eqB2.id, "quitar_repositorio", { repo: idRepo, quitar_destino: true }, { claveAdmin: claveB, repo: { repo: idRepo, contrasena } }, {}, 120_000);
      comprobar(/aún tiene copias guardadas/.test(quitado.mensaje ?? ""), `Quitar el repositorio y su destino: ${quitado.mensaje}`);
      comprobar(!(quitado.mensaje ?? "").includes(carpeta), "Sin la ruta de la carpeta", quitado.mensaje);
      comprobar(fs.existsSync(path.join(carpeta, idRepo, "config")), "Lo guardado en la carpeta se queda");
      await esperar("el destino quitado también en la en línea", async () => !(await consola3.equipo(c3, eqB2.id)).resumen?.destinos?.some((d) => d.id === idDestino) || null, { plazo: 60_000, cada: 1000 });
      log("Destino vacío quitado; su carpeta sigue con sus copias");
    }

    // -----------------------------------------------------------------------
    paso("8b. Cambiar la clave de administración desde la consola local, con la en línea conectada");
    // Como CambiarClaveAdmin.svelte: verificador del equipo y K_cfg de cada consola (la otra, con su sal).
    const nueva = new ClaveNueva(argon2, CLAVE_NUEVA, c2.sal_cliente);
    const cuerpoClave = await nueva.cuerpo(await consola2.equipo(c2, eqB2.id));
    comprobar(cuerpoClave.k_cfg_consolas?.[srv3.identidad], "La orden lleva la K_cfg nueva de la consola en línea (con su sal)", cuerpoClave);
    const cambio = await consola2.hecha(c2, eqB2.id, "cambiar_clave_admin", cuerpoClave, { claveAdmin: claveB });
    log(`cambiar_clave_admin: ${cambio.mensaje}`);
    const claveVieja = claveB;
    claveB = CLAVE_NUEVA;
    // El equipo sube la etiqueta nueva a las dos (cada una con su K_cfg).
    const k2 = kCfg(await materialCliente(argon2, CLAVE_NUEVA, c2.sal_cliente));
    const k3 = kCfg(await materialCliente(argon2, CLAVE_NUEVA, c3.sal_cliente));
    await esperar("la etiqueta con la clave nueva en el servidor 2", async () => etiquetaValida(k2, await consola2.equipo(c2, eqB2.id)), { plazo: 60_000, cada: 1000 });
    await esperar("la etiqueta con la clave nueva en el servidor 3", async () => etiquetaValida(k3, await consola3.equipo(c3, eqB2.id)), { plazo: 60_000, cada: 1000 });
    // Con la nueva, las órdenes de administración funcionan desde las dos consolas.
    await consola2.hecha(c2, eqB2.id, "cambiar_espera", { horas: 3 }, { claveAdmin: claveB });
    await consola3.hecha(c3, eqB2.id, "cambiar_espera", { horas: 3 }, { claveAdmin: claveB });
    log("Con la clave nueva: órdenes hechas desde las dos consolas");
    // Con la anterior: la consola ya no la da por buena (la etiqueta no cuadra)…
    let paro = "";
    try {
      await consola2.mandar(c2, eqB2.id, "cambiar_espera", { horas: 4 }, { claveAdmin: claveVieja });
    } catch (e) {
      paro = (e as Error).message;
    }
    comprobar(/etiqueta/.test(paro), "La consola no acepta la clave anterior", paro);
    // … y, mandada igualmente, el equipo la rechaza.
    const conVieja = await consola2.mandar(c2, eqB2.id, "cambiar_espera", { horas: 4 }, { claveAdmin: claveVieja }, { sinComprobar: true });
    const rechazo = await consola2.resultado(c2, eqB2.id, conVieja);
    igual(rechazo.estado, "rechazada", `Con la clave anterior, el equipo la rechaza (${rechazo.mensaje})`);
    // La consola en línea recibe el aviso del cambio (no le llegó el resultado de la orden: sí la etiqueta nueva).
    await esperar("el aviso «cambio_clave» en la consola en línea", async () => {
      const av = (await consola3.ok("GET", `/api/clientes/${c3.id}/avisos?abiertos=1`)) as any[];
      return av.some((a) => a.tipo === "cambio_clave" && /otra consola/.test(a.mensaje)) || null;
    }, { plazo: 30_000, cada: 1000 });
    const aud = (await consola2.ok("GET", `/api/clientes/${c2.id}/auditoria?desde=0&limite=500`)) as any[];
    comprobar(aud.some((x) => x.accion === "clave_admin_cambiada"), "La auditoría del servidor 2 lo cuenta", aud.map((x) => x.accion));
    // Quitar la en línea desde la local: la local sigue.
    await consola2.hecha(c2, eqB2.id, "quitar_consola", { identidad: srv3.identidad }, { claveAdmin: claveB });
    await esperar("que B deje de conectarse al servidor 3", async () => !(await consola3.equipo(c3, eqB2.id)).conectado, { plazo: 60_000, cada: 1000 });
    const avisos3 = (await consola3.ok("GET", `/api/clientes/${c3.id}/avisos?abiertos=1`)) as any[];
    comprobar(avisos3.some((a) => /dejó de conectarse/.test(a.mensaje)), "La en línea recibe un último aviso", avisos3);
    log("Quitada la en línea desde la local");
    await consola2.hecha(c2, eqB2.id, "cambiar_espera", { horas: 4 }, { claveAdmin: claveB });
    // Y al revés: otra vez la en línea, y es ella la que deja de gestionarlo («Dejar de gestionar»): la local sigue.
    await consola2.hecha(c2, eqB2.id, "anadir_consola", await anadir(), { claveAdmin: claveB }, {}, 120_000);
    await esperar("B otra vez en el servidor 3", async () => (await consola3.equipo(c3, eqB2.id)).conectado, { plazo: 90_000, cada: 1000 });
    log("Otra vez en la en línea");
    const deja = await consola3.hecha(c3, eqB2.id, "desvincular", { modo: "seguir_local" }, { claveAdmin: claveB });
    comprobar(/siguen gestionando/.test(deja.mensaje ?? ""), "«Dejar de gestionar» solo quita la en línea", deja.mensaje);
    igual((await consola3.equipo(c3, eqB2.id)).modo, "local", "En la en línea, el equipo queda en local");
    const traslocal = await consola2.hecha(c2, eqB2.id, "cambiar_espera", { horas: 5 }, { claveAdmin: claveB });
    log(`La consola local sigue: ${traslocal.mensaje}`);
    const lista = B.cli(["consolas"]);
    comprobar(lista.codigo === 0 && lista.salida.includes(s2.url) && !lista.salida.includes(s3.url), "`resguardo-agente consolas`: solo la local", lista.salida);

    // -----------------------------------------------------------------------
    paso("9. «Mover a otra consola»: B se va del servidor 2 al 3 sin quedarse nunca sin consola");
    // 1) Conectar también a la otra.
    await consola2.hecha(c2, eqB2.id, "anadir_consola", await anadir(), { claveAdmin: claveB }, {}, 120_000);
    // 2) Esperar a que informe allí (como la consola: su resumen aquí la lista con su último contacto).
    await esperar("que B informe en el servidor 3 (visto desde el 2)", async () => {
      const e = await consola2.equipo(c2, eqB2.id);
      return e.resumen?.consolas?.some((x) => x.identidad === srv3.identidad && !x.esta && x.ultimo_contacto);
    }, { plazo: 90_000, cada: 1000 });
    log("B ya informa en la consola de destino");
    // 3) «Dejar esta consola»: quitar_consola de la propia.
    const srv2 = await consola2.ok("GET", "/api/servidor");
    const sale = await consola2.hecha(c2, eqB2.id, "quitar_consola", { identidad: srv2.identidad }, { claveAdmin: claveB });
    comprobar(JSON.parse(sale.detalle ?? "{}").deja_esta_consola === true, "El equipo dice, firmado, que deja esta consola", sale);
    // 4) Comprobación final: aquí ya no está; allí manda.
    igual((await consola2.equipo(c2, eqB2.id)).modo, "local", "En la consola de origen, el equipo queda fuera");
    await consola3.hecha(c3, eqB2.id, "cambiar_espera", { horas: 6 }, { claveAdmin: claveB });
    const solo = B.cli(["consolas"]);
    comprobar(solo.codigo === 0 && solo.salida.includes(s3.url) && !solo.salida.includes(s2.url), "`resguardo-agente consolas`: solo la de destino", solo.salida);
    log("B movido al servidor 3");

    ok = true;
  } catch (e) {
    console.error(`\n✗ FALLO en «${pasoEnCurso()}»:\n  ${(e as Error).stack ?? e}`);
    console.error(`  Registros en ${registros}`);
    process.exitCode = 1;
  } finally {
    buzon.cerrar();
    await pararTodo();
    if (ok && process.env.RESGUARDO_E2E_CONSERVAR !== "1") borrarCarpeta(base);
    else console.log(`Se conserva ${base}`);
  }
  if (ok) console.log("\n✓ Escenario completo.");
}

void principal().catch(async (e) => {
  console.error(e);
  await pararTodo();
  process.exit(1);
});
