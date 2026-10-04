// Agente de prueba para probar la consola contra un Resguardo Server de verdad
// (sin el agente real): se une con un código, comprueba la identidad del
// servidor en cada consulta, abre los sobres, comprueba la prueba de
// administración y las contraseñas, contesta firmado, sube su configuración
// cifrada con K_cfg y responde en las sesiones (carpetas y versiones de
// mentira). Solo para desarrollo.
//
//   npx tsx scripts/agente-de-prueba.ts --servidor http://127.0.0.1:8470 --codigo ABCD-EFGH-JK [--nombre PRUEBA] [--estado archivo.json]
import fs from "node:fs";
import { ed25519, x25519 } from "@noble/curves/ed25519.js";
import { sha256 } from "@noble/hashes/sha2.js";
import { aB64, aleatorio, deB64, deUtf8, iguales, utf8 } from "../src/lib/cripto/bytes";
import { hashCodigo, identidadServidorValida, mensajeResultado, pruebaCodigo, sasV3 } from "../src/lib/cripto/claves";
import { abrir, sellar } from "../src/lib/cripto/sobre";
import { NIVEL, PIDE_TAMBIEN_ADMIN, type OrdenPlana } from "../src/lib/cripto/ordenes";
import { cifrarConfig, claveDireccion, cifrarMensaje, descifrarMensaje } from "../src/lib/cripto/simetrico";
import type { Configuracion } from "../src/lib/tipos";

const arg = (n: string, def?: string) => {
  const i = process.argv.indexOf(`--${n}`);
  return i > 0 ? process.argv[i + 1] : def;
};
const SERVIDOR = arg("servidor", "http://127.0.0.1:8470")!;
const ARCHIVO = arg("estado", "agente-de-prueba.json")!;

interface Estado {
  id: string;
  secreto: string;
  identidad: string;
  box: string;
  firma: string;
  sal: string;
  verificador: string | null;
  kcfg: string | null;
  ultimoSeq: number;
  contrasenas: Record<string, string>;
  config: Configuracion | null;
  /** Código con el que se unió (para comprobar prueba_codigo en el alta). */
  codigo: string | null;
}

let e: Estado;
const auth = () => ({ Authorization: `Equipo ${e.id}:${e.secreto}`, "Content-Type": "application/json" });
async function post(ruta: string, cuerpo: unknown) {
  const r = await fetch(SERVIDOR + ruta, { method: "POST", headers: auth(), body: JSON.stringify(cuerpo) });
  if (!r.ok) throw new Error(`${ruta}: ${r.status} ${await r.text()}`);
  return r.status === 204 ? null : r.json();
}
const guardar = () => fs.writeFileSync(ARCHIVO, JSON.stringify(e, null, 1));

async function unirse() {
  const codigo = arg("codigo");
  if (!codigo) throw new Error("Falta --codigo");
  const box = x25519.utils.randomSecretKey();
  const firma = ed25519.utils.randomSecretKey();
  const sal = aleatorio(16);
  const box_pub = aB64(x25519.getPublicKey(box));
  const sign_pub = aB64(ed25519.getPublicKey(firma));
  const r = await fetch(`${SERVIDOR}/api/agente/unirse`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ codigo_hash: hashCodigo(codigo), nombre: arg("nombre", "EQUIPO-DE-PRUEBA"), so: arg("so", "Windows 11 Pro"), version: "0.0.0-prueba", box_pub, sign_pub, sal_equipo: aB64(sal), sas_version: 3 }),
  });
  if (!r.ok) throw new Error(`unirse: ${r.status} ${await r.text()}`);
  const j = await r.json();
  // SAS v3: con la huella de la autoridad TLS (este agente de prueba no la fija: usa la que dice el servidor).
  const huella = ((await (await fetch(`${SERVIDOR}/api/servidor`)).json()) as { huella_ca?: string }).huella_ca ?? "";
  const local = sasV3(j.servidor.identidad, box_pub, sign_pub, huella);
  console.log(`Unido como ${j.equipo_id}. Número de comprobación: ${local}${local === j.sas ? "" : ` (¡el servidor dice ${j.sas}!)`}`);
  e = { id: j.equipo_id, secreto: j.secreto, identidad: j.servidor.identidad, box: aB64(box), firma: aB64(firma), sal: aB64(sal), verificador: null, kcfg: null, ultimoSeq: 0, contrasenas: {}, config: null, codigo };
  guardar();
}

async function resultado(orden: { id: string; seq: number }, estado: string, mensaje: string | null, detalle: string | null = null) {
  const texto = mensajeResultado({ id: orden.id, seq: orden.seq, estado, mensaje, detalle });
  await post("/api/agente/resultado", { orden: orden.id, estado, mensaje, detalle, firma: aB64(ed25519.sign(utf8(texto), deB64(e.firma))) });
  console.log(`  → ${estado}: ${mensaje ?? ""}`);
}

async function subirConfig(seq: number) {
  if (!e.kcfg || !e.config) return;
  const c = e.config;
  const resumen = {
    // Como el agente (api-servidor.md §4): sin rutas; «destino» es el nombre; en local, donde = null.
    copias: c.copias.map((k) => ({ id: k.id, nombre: k.nombre, repo: k.repo, horario: k.horario, carpetas: k.carpetas.length, activa: k.activa, ultima: null })),
    repositorios: c.repositorios.map((r) => ({ id: r.id, nombre: r.nombre, destino: c.destinos.find((d) => d.id === r.destino)?.nombre ?? r.destino, retencion: null })),
    destinos: c.destinos.map((d) => ({ ...d, donde: d.tipo === "local" ? null : d.donde })),
    pausado_hasta: null,
  };
  await post("/api/agente/config", { seq, cifrado: aB64(cifrarConfig(deB64(e.kcfg), e.id, seq, c)), resumen });
}

type Bruta = { id: string; tipo: string; seq: number; sellado: string; not_before: string | null; caduca: string };
async function procesar(o: Bruta) {
  console.log(`Orden n.º ${o.seq}: ${o.tipo}`);
  let p: OrdenPlana;
  try {
    p = JSON.parse(deUtf8(abrir(deB64(e.box), deB64(o.sellado))));
  } catch {
    return resultado(o, "rechazada", "No se pudo abrir el sobre.");
  }
  if (p.v !== 2 || p.equipo !== e.id || p.seq !== o.seq || p.tipo !== o.tipo || p.caduca !== o.caduca || p.not_before !== o.not_before) return resultado(o, "rechazada", "La orden no coincide con sus datos en claro.");
  if (p.seq <= e.ultimoSeq) return resultado(o, "rechazada", "Orden repetida.");
  const nivel = NIVEL[p.tipo];
  const pruebaOk = !!p.autorizacion.prueba_admin && !!e.verificador && iguales(sha256(deB64(p.autorizacion.prueba_admin)), deB64(e.verificador));
  if (p.tipo !== "alta" && (nivel === "admin" || PIDE_TAMBIEN_ADMIN.has(p.tipo)) && !pruebaOk) return resultado(o, "rechazada", "La clave de administración no es correcta.");
  if (nivel === "repo") {
    const c = p.autorizacion.clave_repo;
    if (!c || e.contrasenas[c.repo] !== c.contrasena) return resultado(o, "rechazada", "La contraseña del repositorio no es correcta.");
  }
  e.ultimoSeq = p.seq;
  const cuerpo = p.cuerpo as Record<string, unknown>;
  switch (p.tipo) {
    case "alta": {
      const ver = String(cuerpo.verificador);
      if (!p.autorizacion.prueba_admin || !iguales(sha256(deB64(p.autorizacion.prueba_admin)), deB64(ver))) return resultado(o, "rechazada", "La prueba no corresponde al verificador.");
      if (e.verificador) return resultado(o, "rechazada", "Ya estaba dado de alta.");
      if (!e.codigo || p.autorizacion.prueba_codigo !== pruebaCodigo(e.codigo, e.id, ver)) return resultado(o, "rechazada", "El alta no viene de quien tiene el código de emparejamiento.");
      e.codigo = null;
      e.verificador = ver;
      e.kcfg = String(cuerpo.k_cfg);
      e.config = { v: 1, copias: [], repositorios: [], destinos: [] };
      guardar();
      await subirConfig(p.seq);
      return resultado(o, "hecha", "Equipo dado de alta.");
    }
    case "config": {
      // Configuración v1: solo copias (y verificación y bandeja); repositorios y destinos los pone el equipo.
      const cfg = cuerpo.config as Configuracion;
      if (cfg?.v !== 1 || !Array.isArray(cfg.copias)) return resultado(o, "fallida", "Configuración no válida.");
      if (cfg.copias.some((k) => k.gancho)) return resultado(o, "fallida", "Los ganchos aún no están disponibles.");
      e.config = { ...cfg, repositorios: e.config?.repositorios ?? [], destinos: e.config?.destinos ?? [] };
      guardar();
      await subirConfig(p.seq);
      return resultado(o, "hecha", `Configuración aplicada: ${e.config.copias.length} copias.`);
    }
    case "crear_repositorio": {
      const d = cuerpo.destino as { id: string; nombre?: string; tipo?: string; donde?: string };
      if (String(cuerpo.contrasena ?? "").length < 8) return resultado(o, "fallida", "Contraseña demasiado corta.");
      e.contrasenas[String(cuerpo.id)] = String(cuerpo.contrasena);
      e.config ??= { v: 1, copias: [], repositorios: [], destinos: [] };
      if (d.nombre && !e.config.destinos.some((x) => x.id === d.id)) e.config.destinos.push({ id: d.id, nombre: d.nombre, tipo: (d.tipo as "rest") ?? "otro", donde: d.donde });
      e.config.repositorios.push({ id: String(cuerpo.id), nombre: String(cuerpo.nombre), destino: d.id });
      guardar();
      await subirConfig(p.seq);
      return resultado(o, "hecha", "Repositorio creado.");
    }
    case "cambiar_copia_externa": {
      // v1.5: copia externa diaria a otro destino (aquí solo se guarda en la configuración).
      const repo = e.config?.repositorios.find((r) => r.id === cuerpo.repo);
      if (!repo || !e.config) return resultado(o, "fallida", "Ese repositorio no existe en este equipo.");
      if (cuerpo.hora === null) {
        repo.externa = null;
      } else {
        const d = cuerpo.destino as { id: string; nombre?: string; tipo?: string; donde?: string };
        if (!d?.id || d.id === repo.destino) return resultado(o, "fallida", "La copia externa tiene que ir a otro destino.");
        if (d.nombre && !e.config.destinos.some((x) => x.id === d.id)) e.config.destinos.push({ id: d.id, nombre: d.nombre, tipo: (d.tipo as "local") ?? "otro", donde: d.donde });
        if (!e.config.destinos.some((x) => x.id === d.id)) return resultado(o, "fallida", "Ese destino no existe en este equipo.");
        repo.externa = { destino: d.id, hora: String(cuerpo.hora) };
      }
      guardar();
      await subirConfig(p.seq);
      return resultado(o, "hecha", cuerpo.hora === null ? "Copia externa quitada." : `Copia externa cada día a las ${cuerpo.hora}.`);
    }
    case "compartir_acceso": {
      // §10: el acceso al destino del repositorio, firmado y sellado para el otro equipo.
      const repo = e.config?.repositorios.find((r) => r.id === cuerpo.repo);
      const para = cuerpo.para as { equipo?: string; box_pub?: string } | undefined;
      if (!repo || !para?.equipo || !para.box_pub) return resultado(o, "fallida", "Falta el repositorio o el equipo de destino.");
      const destino = e.config!.destinos.find((d) => d.id === repo.destino) ?? { id: repo.destino, nombre: repo.destino, tipo: "otro" };
      const datos = JSON.stringify({ repo: { id: repo.id, nombre: repo.nombre }, destino, ...(cuerpo.incluir_contrasena ? { contrasena: e.contrasenas[repo.id] } : {}) });
      const firma = aB64(ed25519.sign(utf8(`resguardo-acceso-v1|${para.equipo}|${datos}`), deB64(e.firma)));
      const sobre = utf8(JSON.stringify({ datos, firma, de: e.id }));
      return resultado(o, "hecha", "Acceso compartido.", JSON.stringify({ acceso_sellado: aB64(sellar(deB64(para.box_pub), sobre)) }));
    }
    case "importar_repositorio": {
      const id = String(cuerpo.id ?? "");
      e.config ??= { v: 1, copias: [], repositorios: [], destinos: [] };
      if (!/^[A-Za-z0-9_-]{1,64}$/.test(id) || e.config.repositorios.some((r) => r.id === id)) return resultado(o, "fallida", "Id de repositorio no válido o ya en uso.");
      let contrasena: string | undefined;
      let destino: { nombre?: string; tipo?: string; donde?: string };
      if (typeof cuerpo.acceso_sellado === "string") {
        const ab = JSON.parse(deUtf8(abrir(deB64(e.box), deB64(cuerpo.acceso_sellado)))) as { datos: string; firma: string };
        if (!ed25519.verify(deB64(ab.firma), utf8(`resguardo-acceso-v1|${e.id}|${ab.datos}`), deB64(String(cuerpo.sign_pub_origen ?? ""))))
          return resultado(o, "fallida", "El acceso no lo firmó el equipo de origen: no se importa.");
        const d = JSON.parse(ab.datos) as { destino: typeof destino; contrasena?: string };
        destino = d.destino;
        contrasena = d.contrasena ?? (cuerpo.contrasena as string | undefined);
      } else {
        const acc = cuerpo.acceso as { destino: typeof destino; contrasena?: string };
        destino = acc.destino;
        contrasena = acc.contrasena;
      }
      if (!contrasena) return resultado(o, "fallida", "Falta la contraseña del repositorio.");
      e.contrasenas[id] = contrasena;
      e.config.destinos.push({ id: `importado-${id}`, nombre: destino.nombre ?? "Importado", tipo: (destino.tipo as "rest") ?? "otro", donde: destino.donde });
      e.config.repositorios.push({ id, nombre: String(cuerpo.nombre || id), destino: `importado-${id}`, solo_lectura: true });
      guardar();
      await subirConfig(p.seq);
      return resultado(o, "hecha", "Repositorio importado (solo lectura).");
    }
    case "explorar":
    case "elegir_carpetas":
    case "abrir_sesion":
      // El id de la sesión viene dentro del sobre (autenticado); se comprueba que el servidor la tiene abierta.
      if (!abiertas.has(String(cuerpo.sesion))) return resultado(o, "rechazada", "Esa sesión no está abierta en el servidor.");
      sesiones.set(String(cuerpo.sesion), { clave: deB64(String(cuerpo.clave_sesion)), desde: 0, i: 0 });
      return resultado(o, "hecha", "Sesión abierta.");
    default:
      await resultado(o, "en_marcha", null);
      return resultado(o, "hecha", "Hecho (agente de prueba).");
  }
}

// Sesiones abiertas según el servidor (lista de `tomar`).
let abiertas = new Set<string>();
const sesiones = new Map<string, { clave: Uint8Array; desde: number; i: number }>();

async function atenderSesion(id: string) {
  const s = sesiones.get(id);
  if (!s) return;
  const r = await fetch(`${SERVIDOR}/api/agente/sesiones/${id}/mensajes?desde=${s.desde}`, { headers: auth() });
  if (!r.ok) return;
  const lista: { n: number; de: string; cifrado: string }[] = await r.json();
  for (const m of lista) {
    s.desde = Math.max(s.desde, m.n);
    if (m.de !== "consola") continue;
    const k = claveDireccion(s.clave, id, "consola");
    const msg = descifrarMensaje<{ i: number; op: string; ruta?: string }>(k, id, deB64(m.cifrado));
    const resp =
      msg.op === "versiones"
        ? { op: "versiones", versiones: [{ id: "a1b2c3d4", cuando: new Date().toISOString(), archivos: 10, bytes: 1000 }] }
        : msg.op === "carpetas" || msg.op === "listar"
          ? { op: msg.op, ruta: msg.ruta, entradas: msg.ruta ? [{ nombre: "Documentos", tipo: "dir" }, { nombre: "nota.txt", tipo: "archivo", bytes: 12 }] : [{ nombre: "C:", tipo: "dir" }] }
          : msg.op === "sugerencias"
            ? { op: "sugerencias", sugerencias: [] }
            : null;
    if (resp) await enviar(id, s, { ...resp, re: msg.i });
  }
}
async function enviar(id: string, s: { clave: Uint8Array; i: number }, datos: Record<string, unknown>) {
  const k = claveDireccion(s.clave, id, "equipo");
  await post(`/api/agente/sesiones/${id}/mensajes`, { cifrado: aB64(cifrarMensaje(k, id, { ...datos, i: ++s.i })) });
}

async function vuelta() {
  const reto = aB64(aleatorio(32));
  const t = await post("/api/agente/tomar", { reto });
  if (!identidadServidorValida(e.identidad, reto, e.id, t.firma)) throw new Error("¡La identidad del servidor no coincide con la fijada! No se sigue.");
  abiertas = new Set(t.sesiones as string[]);
  for (const o of t.ordenes as Bruta[]) await procesar(o);
  for (const s of t.sesiones as string[]) {
    const ses = sesiones.get(s);
    if (ses && ses.desde === 0 && ses.i === 0) await enviar(s, ses, { op: "lista" });
    await atenderSesion(s);
  }
}

if (fs.existsSync(ARCHIVO) && !arg("codigo")) e = JSON.parse(fs.readFileSync(ARCHIVO, "utf8"));
else await unirse();
await post("/api/agente/informe", { datos: { version: "0.0.0-prueba", servicio: "en_marcha", copias: [] } });
console.log("Escuchando órdenes (Ctrl+C para salir)…");
for (;;) {
  try {
    await vuelta();
  } catch (err) {
    console.error(String(err));
  }
  await new Promise((r) => setTimeout(r, 1500));
}
