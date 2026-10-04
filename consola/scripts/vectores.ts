// Pruebas de la criptografía de la consola:
// 1. Los vectores compartidos con Rust (crates/protocolo/vectors/v1.json,
//    también sus «derivaciones» de la clave de administración).
// 2. El sobre sellado contra libsodium (otra implementación, en las dos direcciones).
// 3. Argon2id con hash-wasm (el del navegador) contra @noble/hashes (otra implementación).
// 4. Ida y vuelta de órdenes, sesiones y relé.
//
//   npm run test:vectores
import fs from "node:fs";
import { createHash, createHmac } from "node:crypto";
import sodium from "libsodium-wrappers";
import { argon2id as argon2Wasm } from "hash-wasm";
import { argon2id as argon2Noble } from "@noble/hashes/argon2.js";
import { ed25519, x25519 } from "@noble/curves/ed25519.js";
import { aB64, aHex, deB64, deUtf8, utf8 } from "../src/lib/cripto/bytes";
import { abrir, sellar } from "../src/lib/cripto/sobre";
import { esDestructiva, NIVEL, PIDE_TAMBIEN_ADMIN } from "../src/lib/cripto/ordenes";
import { almacenDe, errorHorario, errorRegla, estimarVersiones, leerPlazo, leerRegla, nuevaClave, plazoEnPalabras, presetDe, PRESETS, REGLA_POR_DEFECTO, reglaParaOrden, restarPlazo, resumenRegla, textoHorario, textoRegla } from "../src/lib/retencion";
import { errorVerificacion, fraseVerificacion, partesVerificacion } from "../src/lib/verificacion";
import { destinoCuerpo, origenCuerpo, partirDireccion, rutaEnAlmacen, usuarioEnAlmacen } from "../src/lib/direccion";
import { resultadoConError } from "../src/lib/salud";
import { atrasada, cifrasCopia, estadoCopia, explicarError, infCopia, proximaDe, ultimaProgramada, ultimaVuelta } from "../src/lib/copia";
import { bytesRepo, destinoDe, versionDeVuelta } from "../src/lib/repo";
import type { CopiaResumen, Equipo, Informe, ReglaHorario, RepoInforme, TareaEnMarcha } from "../src/lib/tipos";
import { cifrasTarea, porcentaje, textoCorto, textoFase, textoQuedan } from "../src/lib/textoProgreso";
import { baseValida, errorCarpetaDestino, errorCarpetaEspejo, errorCarpetaLocal, errorGancho, errorNombreCarpeta, instanciaValida, limpiar, rutaValida, versionAlMenos } from "../src/lib/ganchos";
import {
  ARGON2,
  type Argon2,
  etiquetaEquipo,
  etiquetaValida,
  hashCodigo,
  huellaParaSas,
  identidadServidorValida,
  kCfg,
  kExp,
  materialCliente,
  mensajeIdentidad,
  mensajeResultado,
  pruebaAdmin,
  resultadoFirmado,
  sasV1,
  sasV2,
  sasV3,
  verificador,
} from "../src/lib/cripto/claves";
import { sellarOrden } from "../src/lib/cripto/ordenes";
import { cifrarConfig, cifrarPlantilla, clavePlantillas, claveDireccion, cifrarMensaje, cifrarTrozo, descifrarConfig, descifrarMensaje, descifrarPlantilla, descifrarTrozo } from "../src/lib/cripto/simetrico";
import { pruebaCodigo } from "../src/lib/cripto/claves";
import { cifrarPaquete, descifrarPaquete } from "../src/lib/cripto/paquete";
import { publicaRespaldo, salRespaldo, secretoRespaldo } from "../src/lib/cripto/respaldo";
import { desplegar, errorIntervalo, errorRegla as errorReglaHorario, errorReglas, expresable, horarioParaEnviar, normalizar, proximaVez, reconocer, reglasDe, ultimaVez } from "../src/lib/horario";
import { diasEnFrase, horarioEnFrase, resumenHorario, resumenReglas } from "../src/lib/formato";
import { ClaveNueva, claveGenerada, estadoCambio, pendientesDeCambio, repartir } from "../src/lib/cambioClave";

const DIR = new URL("../../crates/protocolo/vectors/", import.meta.url);
let fallos = 0;
let total = 0;
function igual(nombre: string, obtenido: unknown, esperado: unknown) {
  total++;
  const ok = JSON.stringify(obtenido) === JSON.stringify(esperado);
  if (!ok) fallos++;
  console.log(`${ok ? "ok  " : "MAL "} ${nombre}${ok ? "" : `\n       obtenido: ${JSON.stringify(obtenido)}\n       esperado: ${JSON.stringify(esperado)}`}`);
}
const cierto = (nombre: string, v: boolean) => igual(nombre, v, true);

const argon2: Argon2 = async (clave, sal) =>
  (await argon2Wasm({
    password: clave,
    salt: sal,
    parallelism: ARGON2.hilos,
    iterations: ARGON2.pasadas,
    memorySize: ARGON2.memoriaKiB,
    hashLength: ARGON2.salida,
    outputType: "binary",
  })) as Uint8Array;

await sodium.ready;

// ---------------------------------------------------------------------------
console.log("\n· Horarios de las copias (lib/horario.ts): «cada N horas» desplegado y reconocido");
{
  const iv = { cada: 1, desde: "07:00", hasta: "19:00" };
  const horas = desplegar(iv);
  igual("cada hora de 7 a 19: 13 horas", horas.length, 13);
  igual("cada hora de 7 a 19: primera y última", [horas[0], horas.at(-1)], ["07:00", "19:00"]);
  igual("se reconoce al volver a abrir", reconocer(horas), iv);
  igual("cada 2 h desde 07:30", desplegar({ cada: 2, desde: "07:30", hasta: "13:00" }), ["07:30", "09:30", "11:30"]);
  igual("lista irregular: no es intervalo", reconocer(["08:00", "13:00", "19:00"]), null);
  igual("dos horas: se enseñan como lista", reconocer(["08:00", "10:00"]), null);
  igual("paso no elegible (5 h): lista", reconocer(["01:00", "06:00", "11:00"]), null);
  igual("hasta antes que desde: vacío", desplegar({ cada: 1, desde: "19:00", hasta: "07:00" }), []);
  igual("intervalo al revés: error", errorIntervalo({ cada: 1, desde: "19:00", hasta: "07:00" }) !== null, true);
  igual("normalizar: ordena y quita repetidas", normalizar(["19:00", "07:00", "19:00", "xx"]), ["07:00", "19:00"]);
  igual("resumen con intervalo", resumenHorario({ dias: [1, 2, 3, 4, 5], horas }), "Cada hora de 7:00 a 19:00, de lunes a viernes · 13 copias al día");
  igual("resumen con lista", resumenHorario({ dias: [1, 2, 3, 4, 5, 6, 7], horas: ["13:00"] }), "A las 13:00, todos los días · 1 copia al día");
  igual("frase de la lista de equipos", horarioEnFrase({ dias: [1, 2, 3, 4, 5], horas: desplegar({ cada: 2, desde: "08:00", hasta: "18:00" }) }), "Cada día laborable, cada 2 horas de 8:00 a 18:00");
  igual("días sueltos", diasEnFrase([6, 7]), "los sábados y domingos");
  igual("días sueltos con miércoles", diasEnFrase([1, 3]), "los lunes y miércoles");
}

// ---------------------------------------------------------------------------
console.log("\n· Horarios con reglas (v1.24, agente ≥ 0.7.9): cada N minutos, cada N días, cada mes");
{
  const L_V = [1, 2, 3, 4, 5];
  const cada10: ReglaHorario = { tipo: "intervalo", dias: L_V, cada_min: 10, desde: "08:00", hasta: "18:00" };
  const dia1: ReglaHorario = { tipo: "mensual", dia: 1, hora: "23:00" };
  // Al abrir: la lista de siempre, como la regla más corta.
  const cadaHora = desplegar({ cada: 1, desde: "07:00", hasta: "19:00" });
  igual("lista por horas → intervalo de 60 min", reglasDe({ dias: [5, 1, 2, 3, 4], horas: cadaHora }), [{ tipo: "intervalo", dias: L_V, cada_min: 60, desde: "07:00", hasta: "19:00" }]);
  const media = ["08:00", "08:30", "09:00", "09:30"];
  igual("lista cada 30 min → intervalo (agente nuevo)", reglasDe({ dias: L_V, horas: media })[0], { tipo: "intervalo", dias: L_V, cada_min: 30, desde: "08:00", hasta: "09:30" });
  igual("lista cada 30 min → horas sueltas (agente anterior)", reglasDe({ dias: L_V, horas: media }, false)[0].tipo, "horas");
  igual("con reglas, las reglas", reglasDe({ dias: [], horas: [], reglas: [cada10, dia1] }), [cada10, dia1]);
  igual("sin horario, sin reglas", reglasDe({ dias: [], horas: [] }), []);

  // Lo que se puede decir como lista de horas (cualquier agente).
  igual("horas e intervalos de horas con los mismos días: lista", expresable([{ tipo: "horas", dias: L_V, horas: ["07:30"] }, { tipo: "intervalo", dias: L_V, cada_min: 120, desde: "08:00", hasta: "12:00" }]), {
    dias: L_V,
    horas: ["07:30", "08:00", "10:00", "12:00"],
  });
  igual("minutos: no", expresable([cada10]), null);
  igual("cada mes: no", expresable([dia1]), null);
  igual("días distintos: no", expresable([{ tipo: "horas", dias: L_V, horas: ["08:00"] }, { tipo: "horas", dias: [6], horas: ["12:00"] }]), null);

  // Lo que se manda.
  const simple: ReglaHorario[] = [{ tipo: "horas", dias: L_V, horas: ["19:00", "13:00"] }];
  igual("agente anterior: la lista", horarioParaEnviar(simple, false), { dias: L_V, horas: ["13:00", "19:00"] });
  igual("agente nuevo, horario de siempre: sin reglas", horarioParaEnviar(simple, true), { dias: L_V, horas: ["13:00", "19:00"] });
  igual("agente nuevo, cada 10 min y el día 1: con reglas", horarioParaEnviar([cada10, dia1], true), { dias: [], horas: [], reglas: [cada10, dia1] });
  const combinado: ReglaHorario[] = [{ tipo: "horas", dias: L_V, horas: ["08:00"] }, { tipo: "intervalo", dias: L_V, cada_min: 120, desde: "12:00", hasta: "16:00" }];
  igual("agente nuevo, dos reglas de horas: con reglas y la lista desplegada", horarioParaEnviar(combinado, true), {
    dias: L_V,
    horas: ["08:00", "12:00", "14:00", "16:00"],
    reglas: combinado,
  });
  igual("agente anterior, dos reglas de horas: solo la lista", horarioParaEnviar(combinado, false), { dias: L_V, horas: ["08:00", "12:00", "14:00", "16:00"] });
  const sabado: ReglaHorario[] = [{ tipo: "horas", dias: L_V, horas: ["08:00"] }, { tipo: "horas", dias: [6], horas: ["12:00"] }];
  igual("agente anterior, días distintos: se quedan las reglas (para el aviso)", horarioParaEnviar(sabado, false).reglas?.length, 2);
  cierto("…y no se pueden enviar", errorReglas(reglasDe(horarioParaEnviar(sabado, false), false), false)?.startsWith("Actualiza") === true);

  // Validación.
  cierto("10 minutos con un agente anterior: «Actualiza el agente…»", errorReglaHorario(cada10, false)?.startsWith("Actualiza el agente para usar esto") === true);
  cierto("10 minutos con uno nuevo: vale", errorReglaHorario(cada10, true) === null);
  cierto("cada 7 minutos: no", errorReglaHorario({ ...cada10, cada_min: 7 }) !== null);
  cierto("día 29: no", errorReglaHorario({ tipo: "mensual", dia: 29, hora: "23:00" }) !== null);
  cierto("último día: vale", errorReglaHorario({ tipo: "mensual", dia: -1, hora: "23:00" }) === null);
  cierto("cada 0 días: no", errorReglaHorario({ tipo: "cada_dias", cada: 0, inicio: "2026-10-01", hora: "23:00" }) !== null);
  cierto("30 de febrero: no", errorReglaHorario({ tipo: "cada_dias", cada: 3, inicio: "2026-02-30", hora: "23:00" }) !== null);
  cierto("agente anterior con días distintos: «Actualiza el agente…»", errorReglas([{ tipo: "horas", dias: L_V, horas: ["08:00"] }, { tipo: "horas", dias: [6], horas: ["12:00"] }], false)?.startsWith("Actualiza") === true);
  cierto("sin reglas: sin horario", errorReglas([]) === "No tiene horario.");

  // Frases.
  igual("resumen del ejemplo", resumenReglas([cada10, dia1]), "Cada 10 minutos de 8:00 a 18:00, de lunes a viernes · y el día 1 de cada mes a las 23:00 · unas 61 copias al día");
  igual("cada 3 días", resumenReglas([{ tipo: "cada_dias", cada: 3, inicio: "2026-10-01", hora: "23:00" }]), "Cada 3 días a las 23:00, desde el 1 de octubre de 2026 · 1 copia cada 3 días");
  igual("el último día", resumenReglas([{ tipo: "mensual", dia: -1, hora: "01:30" }]), "El último día de cada mes a la 1:30 · 1 copia al mes");
  igual("unos días más que otros", resumenReglas([{ tipo: "horas", dias: L_V, horas: ["08:00", "13:00"] }, { tipo: "horas", dias: [6], horas: ["10:00"] }]), "A las 8:00 y 13:00, de lunes a viernes · y a las 10:00, los sábados · hasta 2 copias al día");
  igual("frase con reglas", horarioEnFrase({ dias: [], horas: [], reglas: [cada10, dia1] }), "Cada 10 minutos de 8:00 a 18:00, de lunes a viernes y el día 1 de cada mes a las 23:00");
  igual("frase del formato del agente", horarioEnFrase({ days: [], mode: "rules", times: [], every_hours: 1, from: "", to: "", rules: [{ kind: "monthly", day: -1, time: "22:00" }] }), "El último día de cada mes a las 22:00");

  // Próxima y última vez (hora del navegador).
  const t = (a: number, m: number, d: number, h: number, min = 0) => new Date(a, m - 1, d, h, min).getTime();
  igual("viernes por la tarde: la próxima, el domingo 1 a las 23:00", proximaVez([cada10, dia1], t(2026, 10, 30, 18, 30)), t(2026, 11, 1, 23));
  igual("y después, el lunes a las 8:00", proximaVez([cada10, dia1], t(2026, 11, 1, 23)), t(2026, 11, 2, 8));
  igual("cada 10 min: la última, a las 13:40", ultimaVez([cada10], t(2026, 10, 5, 13, 47)), t(2026, 10, 5, 13, 40));
  const ultimo: ReglaHorario = { tipo: "mensual", dia: -1, hora: "22:00" };
  igual("último día de febrero (bisiesto)", proximaVez([ultimo], t(2028, 2, 10, 0)), t(2028, 2, 29, 22));
  igual("último día de febrero", proximaVez([ultimo], t(2027, 2, 10, 0)), t(2027, 2, 28, 22));
  igual("del 31 de diciembre al 31 de enero", proximaVez([ultimo], t(2026, 12, 31, 22, 30)), t(2027, 1, 31, 22));
  const cada3: ReglaHorario = { tipo: "cada_dias", cada: 3, inicio: "2026-10-01", hora: "23:00" };
  igual("cada 3 días: antes de empezar, el día de inicio", proximaVez([cada3], t(2026, 9, 20, 10)), t(2026, 10, 1, 23));
  igual("cada 3 días: la siguiente", proximaVez([cada3], t(2026, 10, 2, 10)), t(2026, 10, 4, 23));
  igual("cada 3 días: la última (apagado una semana)", ultimaVez([cada3], t(2026, 10, 8, 9)), t(2026, 10, 7, 23));
  igual("cada 3 días: antes de empezar, ninguna", ultimaVez([cada3], t(2026, 10, 1, 22)), null);
  const k: CopiaResumen = { id: "x", nombre: "X", repo: "r", horario: { dias: [], horas: [], reglas: [dia1] }, activa: true };
  igual("proximaDe con reglas", proximaDe(k, null, t(2026, 10, 2, 10)), new Date(t(2026, 11, 1, 23)).toISOString());
  igual("ultimaProgramada con reglas", ultimaProgramada(k.horario, t(2026, 10, 2, 10)), t(2026, 10, 1, 23));
}

// ---------------------------------------------------------------------------
console.log("\n· Vectores v1 (crates/protocolo/vectors/v1.json)");
const v1 = JSON.parse(fs.readFileSync(new URL("v1.json", DIR), "utf8"));
for (const { code, hash } of v1.code_hash) igual(`hash del código «${code}»`, hashCodigo(code), hash);
igual("SAS v1", sasV1(v1.sas.console_sign_pub, v1.sas.endpoint_box_pub), v1.sas.sas);
igual("X25519: pública", aB64(x25519.getPublicKey(deB64(v1.x25519.secret))), v1.x25519.public);
igual("Ed25519: pública", aB64(ed25519.getPublicKey(deB64(v1.ed25519.seed))), v1.ed25519.public);
igual("Ed25519: firma", aB64(ed25519.sign(utf8(v1.ed25519.message_json), deB64(v1.ed25519.seed))), v1.ed25519.signature);
const plano = JSON.parse(v1.signed_envelope.plain);
igual("Sobre firmado: carga", deUtf8(deB64(plano.payload)), v1.ed25519.message_json);
cierto("Sobre firmado: la firma verifica", ed25519.verify(deB64(plano.sig), deB64(plano.payload), deB64(v1.ed25519.public)));
if (v1.signed_envelope.sealed) {
  igual("Sobre sellado de Rust: se abre", deUtf8(abrir(deB64(v1.x25519.secret), deB64(v1.signed_envelope.sealed))), v1.signed_envelope.plain);
}

// ---------------------------------------------------------------------------
console.log("\n· Sobre sellado frente a libsodium");
const destino = sodium.crypto_box_keypair();
const msg = utf8("Hola, equipo: ñandú 🦤 y una contraseña de prueba");
igual("consola → libsodium", deUtf8(sodium.crypto_box_seal_open(sellar(destino.publicKey, msg), destino.publicKey, destino.privateKey)), deUtf8(msg));
igual("libsodium → consola", deUtf8(abrir(destino.privateKey, sodium.crypto_box_seal(msg, destino.publicKey))), deUtf8(msg));
// Con la efímera fija, el resultado es determinista: se compara byte a byte
// con crypto_box_easy de libsodium y el nonce BLAKE2b del sellado.
{
  const esk = deB64(v1.x25519.secret);
  const epk = x25519.getPublicKey(esk);
  const nonce = sodium.crypto_generichash(24, new Uint8Array([...epk, ...destino.publicKey]));
  const esperado = new Uint8Array([...epk, ...sodium.crypto_box_easy(msg, nonce, destino.publicKey, esk)]);
  igual("sobre determinista = epk ‖ crypto_box_easy", aHex(sellar(destino.publicKey, msg, esk)), aHex(esperado));
}
{
  let rechazado = false;
  const s = sellar(destino.publicKey, msg);
  s[60] ^= 1;
  try {
    abrir(destino.privateKey, s);
  } catch {
    rechazado = true;
  }
  cierto("un sobre alterado no se abre", rechazado);
}

// ---------------------------------------------------------------------------
console.log("\n· Argon2id (64 MiB, t=3, p=1): hash-wasm frente a @noble/hashes");
{
  const clave = utf8("correcta caballo batería grapa");
  const sal = utf8("sal-de-equipo-16");
  const t0 = Date.now();
  const a = await argon2(clave, sal);
  const t1 = Date.now();
  const b = argon2Noble(clave, sal, { t: ARGON2.pasadas, m: ARGON2.memoriaKiB, p: ARGON2.hilos, dkLen: ARGON2.salida });
  const t2 = Date.now();
  igual("mismo resultado", aHex(a), aHex(b));
  console.log(`       (hash-wasm ${t1 - t0} ms · JavaScript puro ${t2 - t1} ms)`);
}

// ---------------------------------------------------------------------------
console.log("\n· Derivaciones de la clave de administración");
const salCliente = aB64(utf8("sal-del-cliente-0123456789abcdef"));
const salEquipo = aB64(utf8("sal-del-equipo-0123456789abcdef!"));
const claveAdmin = "Clave de administración de prueba";
const prueba = await pruebaAdmin(argon2, claveAdmin, salEquipo);
igual("prueba de 32 bytes", prueba.length, 32);
igual("verificador = SHA-256(prueba)", aHex(verificador(prueba)), createHash("sha256").update(prueba).digest("hex"));
const material = await materialCliente(argon2, claveAdmin, salCliente);
const kcfg = kCfg(material);
cierto("K_cfg ≠ K_exp", aHex(kcfg) !== aHex(kExp(material)));
const eq = { id: "6f1c2a9e-0000-4000-8000-000000000001", box_pub: aB64(destino.publicKey), sign_pub: v1.ed25519.public, etiqueta: "" };
eq.etiqueta = etiquetaEquipo(kcfg, eq.id, eq.box_pub, eq.sign_pub);
cierto("la etiqueta valida", etiquetaValida(kcfg, eq));
cierto("con otra clave pública, no", !etiquetaValida(kcfg, { ...eq, box_pub: aB64(x25519.getPublicKey(x25519.utils.randomSecretKey())) }));
const sas = sasV2(v1.ed25519.public, eq.box_pub, eq.sign_pub);
cierto("SAS v2 con formato «NNN NNN»", /^\d{3} \d{3}$/.test(sas));

// Identidad del servidor y resultados firmados por el agente.
{
  const seed = deB64(v1.ed25519.seed);
  const reto = aB64(new Uint8Array(32).fill(5));
  const firma = aB64(ed25519.sign(utf8(mensajeIdentidad(reto, eq.id)), seed));
  cierto("prueba de identidad del servidor", identidadServidorValida(v1.ed25519.public, reto, eq.id, firma));
  cierto("con otro equipo, no", !identidadServidorValida(v1.ed25519.public, reto, "otro", firma));
  const o = { id: "orden-1", seq: 7, estado: "hecha", mensaje: "Copia terminada", detalle: null, firma_agente: "" };
  o.firma_agente = aB64(ed25519.sign(utf8(mensajeResultado(o)), seed));
  cierto("resultado firmado por el equipo", resultadoFirmado(v1.ed25519.public, o));
  cierto("resultado alterado, no", !resultadoFirmado(v1.ed25519.public, { ...o, estado: "fallida" }));
}

// ---------------------------------------------------------------------------
console.log("\n· Órdenes, sesiones y relé (ida y vuelta)");
{
  const p = sellarOrden({
    cliente: "c1",
    equipo: { id: eq.id, box_pub: eq.box_pub },
    seq: 18,
    tipo: "pausar",
    cuerpo: { horas: 4 },
    autorizacion: { prueba_admin: aB64(prueba), clave_repo: null },
    esperaHoras: 24,
  });
  const dentro = JSON.parse(deUtf8(sodium.crypto_box_seal_open(deB64(p.sellado), destino.publicKey, destino.privateKey)));
  igual("metadatos en claro = los de dentro", [p.meta.tipo, p.meta.seq, p.meta.caduca, p.meta.not_before], [dentro.tipo, dentro.seq, dentro.caduca, dentro.not_before]);
  cierto("destructiva: not_before ≥ ahora + 24 h", Date.parse(dentro.not_before) >= Date.now() + 24 * 3600_000);
  igual("la prueba viaja dentro", dentro.autorizacion.prueba_admin, aB64(prueba));
}
{
  const clave = new Uint8Array(32).fill(9);
  const k = claveDireccion(clave, "s1", "consola");
  igual("sesión: ida y vuelta", descifrarMensaje(k, "s1", cifrarMensaje(k, "s1", { i: 1, op: "listar", ruta: "/" })), { i: 1, op: "listar", ruta: "/" });
  cierto("sesión: claves distintas por dirección", aHex(k) !== aHex(claveDireccion(clave, "s1", "equipo")));
  const t = cifrarTrozo(clave, "r1", 3, true, utf8("trozo"));
  igual("relé: ida y vuelta", deUtf8(descifrarTrozo(clave, "r1", 3, true, t)), "trozo");
  let rechazado = false;
  try {
    descifrarTrozo(clave, "r1", 4, true, t);
  } catch {
    rechazado = true;
  }
  cierto("relé: un trozo cambiado de sitio no se acepta", rechazado);
}

// ---------------------------------------------------------------------------
console.log("\n· Clave de respaldo de la consola (v1.json → «respaldo_consola», crates/protocolo/src/respaldo_consola.rs)");
if (!v1.respaldo_consola) {
  console.log("MAL  falta «respaldo_consola» en v1.json");
  fallos++;
} else {
  const r = v1.respaldo_consola;
  igual("secreto = HKDF(Argon2id(NFC(clave), sal))", aB64(await secretoRespaldo(argon2, r.clave, r.sal)), r.secreto);
  igual("pública X25519 (lo que recibe el servidor)", await publicaRespaldo(argon2, r.clave, r.sal), r.publica);
  igual("misma pública con la clave en otra forma Unicode (NFD)", await publicaRespaldo(argon2, r.clave.normalize("NFD"), r.sal), r.publica);
  // La pública abre los sobres de libsodium: un sobre sellado para ella se abre con el secreto.
  const secreto = await secretoRespaldo(argon2, r.clave, r.sal);
  const sobre = sodium.crypto_box_seal(new Uint8Array([1, 2, 3]), deB64(r.publica));
  igual("un sobre para la pública se abre con el secreto (libsodium)", Array.from(abrir(secreto, sobre)), [1, 2, 3]);
  cierto("sal nueva de 16 bytes", deB64(salRespaldo()).length === 16);
}

// ---------------------------------------------------------------------------
console.log("\n· Derivaciones v2 (v1.json → «derivaciones», crates/protocolo/src/derivaciones.rs)");
if (!v1.derivaciones) {
  console.log("MAL  faltan las derivaciones en v1.json");
  fallos++;
} else {
  const d = v1.derivaciones;
  igual("parámetros de Argon2id", [d.argon2id.m_kib, d.argon2id.t, d.argon2id.p, d.argon2id.longitud], [ARGON2.memoriaKiB, ARGON2.pasadas, ARGON2.hilos, ARGON2.salida]);
  const pr = await pruebaAdmin(argon2, d.clave_admin, d.sal_equipo);
  igual("prueba_admin = Argon2id(clave, sal_equipo)", aB64(pr), d.prueba_admin);
  igual("verificador = SHA-256(prueba)", aB64(verificador(pr)), d.verificador);
  const m = await materialCliente(argon2, d.clave_admin, d.sal_cliente);
  igual("Argon2id(clave, sal_cliente)", aB64(m), d.argon2id_cliente);
  const kc = kCfg(m);
  igual("K_cfg (HKDF, info resguardo-kcfg-v1)", aB64(kc), d.k_cfg);
  igual("K_exp (HKDF, info resguardo-kexp-v1)", aB64(kExp(m)), d.k_exp);
  igual("etiqueta del equipo", etiquetaEquipo(kc, d.etiqueta.equipo, d.etiqueta.box_pub, d.etiqueta.sign_pub), d.etiqueta.valor);
  cierto("etiquetaValida con la etiqueta de Rust", etiquetaValida(kc, { id: d.etiqueta.equipo, box_pub: d.etiqueta.box_pub, sign_pub: d.etiqueta.sign_pub, etiqueta: d.etiqueta.valor }));
  igual("SAS v2", sasV2(d.sas_v2.identidad_servidor, d.sas_v2.box_pub, d.sas_v2.sign_pub), d.sas_v2.sas);
  const s3 = d.sas_v3;
  igual("SAS v3: huella normalizada", huellaParaSas(s3.huella_ca), s3.huella_normalizada);
  igual("SAS v3", sasV3(s3.identidad_servidor, s3.box_pub, s3.sign_pub, s3.huella_ca), s3.sas);
  igual("SAS v3 con la huella en minúsculas y sin «:»", sasV3(s3.identidad_servidor, s3.box_pub, s3.sign_pub, s3.huella_ca.replaceAll(":", "").toLowerCase()), s3.sas);
  igual("SAS v3 con otra autoridad", sasV3(s3.identidad_servidor, s3.box_pub, s3.sign_pub, s3.huella_otra), s3.sas_otra);
  igual("SAS v3 sin huella", sasV3(s3.identidad_servidor, s3.box_pub, s3.sign_pub, ""), s3.sas_sin_huella);
  cierto("SAS v3 ≠ SAS v2", s3.sas !== d.sas_v2.sas && s3.sas !== s3.sas_otra);
  const id = d.identidad_servidor;
  igual("identidad: pública de la semilla", aB64(ed25519.getPublicKey(deB64(id.seed))), id.publica);
  igual("identidad: texto firmado", mensajeIdentidad(id.reto, id.equipo), id.texto);
  cierto("identidad: la firma de Rust verifica", identidadServidorValida(id.publica, id.reto, id.equipo, id.firma));
  igual("texto del resultado", mensajeResultado({ id: "orden-1", seq: 18, estado: "hecha", mensaje: "Copia hecha.", detalle: null }), d.resultado.texto);
}

// ---------------------------------------------------------------------------
console.log("\n· Simétrico (v1.json → «simetrico», crates/protocolo/src/simetrico.rs) y alta");
{
  const sv = v1.simetrico;
  // Configuración: XChaCha20-Poly1305 con K_cfg, aad = "resguardo-config-v1|equipo|seq".
  const c = sv.config;
  igual("configuración cifrada", aB64(cifrarConfig(deB64(c.k_cfg), c.equipo, c.seq, JSON.parse(c.json), deB64(c.nonce))), c.cifrado);
  igual("configuración: se descifra", JSON.stringify(descifrarConfig(deB64(c.k_cfg), c.equipo, c.seq, deB64(c.cifrado))), c.json);
  let otra = false;
  try {
    descifrarConfig(deB64(c.k_cfg), c.equipo, c.seq + 1, deB64(c.cifrado));
  } catch {
    otra = true;
  }
  cierto("configuración: con otro seq no se abre", otra);
  // Plantillas (v1.20): K_pla de K_cfg; aad = "resguardo-plantilla-v1|cliente|id".
  {
    const p = v1.plantilla;
    igual("plantilla: clave derivada de K_cfg", aB64(clavePlantillas(deB64(p.k_cfg))), p.k_plantillas);
    igual("plantilla cifrada", aB64(cifrarPlantilla(deB64(p.k_plantillas), p.cliente, p.id, JSON.parse(p.json), deB64(p.nonce))), p.cifrado);
    igual("plantilla: se descifra", JSON.stringify(descifrarPlantilla(deB64(p.k_plantillas), p.cliente, p.id, deB64(p.cifrado))), p.json);
    let movida = false;
    try {
      descifrarPlantilla(deB64(p.k_plantillas), p.cliente, "otra", deB64(p.cifrado));
    } catch {
      movida = true;
    }
    cierto("plantilla: con otro id (o de otro cliente) no se abre", movida);
  }
  // Sesión: claves por dirección y mensaje.
  const se = sv.sesion;
  igual("sesión: clave de la consola", aB64(claveDireccion(deB64(se.clave_sesion), se.sesion_id, "consola")), se.k_consola);
  igual("sesión: clave del equipo", aB64(claveDireccion(deB64(se.clave_sesion), se.sesion_id, "equipo")), se.k_equipo);
  igual("sesión: mensaje cifrado", aB64(cifrarMensaje(deB64(se.k_consola), se.sesion_id, JSON.parse(se.json), deB64(se.nonce))), se.cifrado_consola);
  // Relé: aad = relevo_id|n|1 (último).
  const re = sv.relevo;
  igual("relé: trozo cifrado", aB64(cifrarTrozo(deB64(re.clave), re.relevo_id, re.n, re.ultimo, deB64(re.datos), deB64(re.nonce))), re.cifrado);
  igual("relé: se descifra", aB64(descifrarTrozo(deB64(re.clave), re.relevo_id, re.n, re.ultimo, deB64(re.cifrado))), re.datos);
  // NFC: la clave descompuesta da la prueba de Rust.
  const nfc = v1.derivaciones.nfc;
  if (nfc) {
    igual("NFC: prueba con la clave compuesta", aB64(await pruebaAdmin(argon2, nfc.clave_nfc, nfc.sal_equipo)), nfc.prueba_admin);
    igual("NFC: prueba con la clave descompuesta", aB64(await pruebaAdmin(argon2, nfc.clave_nfd, nfc.sal_equipo)), nfc.prueba_admin);
  }
  igual("prueba_codigo: el código se normaliza", pruebaCodigo("abcd-efgh-jk", "e", "V"), pruebaCodigo("ABCD EFGH JK", "e", "V"));
  cierto("prueba_codigo: depende del verificador", pruebaCodigo("ABCD-EFGH-JK", "e", "V") !== pruebaCodigo("ABCD-EFGH-JK", "e", "W"));
  // HMAC-SHA256(clave = código normalizado, "resguardo-alta-v1|e|V"), calculado con Node.
  igual("prueba_codigo = HMAC del código normalizado", pruebaCodigo("abcd-efgh-jk", "e", "V"), createHmac("sha256", "ABCDEFGHJK").update("resguardo-alta-v1|e|V").digest("base64"));
}
{
  // NFC: la «ñ» y las tildes compuestas o descompuestas dan la misma prueba.
  const sal = aB64(new Uint8Array(16).fill(1));
  const a = await pruebaAdmin(argon2, "ca\u00f1a de az\u00facar", sal);
  const b = await pruebaAdmin(argon2, "can\u0303a de azu\u0301car", sal);
  igual("la clave se normaliza a NFC antes de Argon2id", aHex(a), aHex(b));
}

// ---------------------------------------------------------------------------
console.log("\n· Paquete de exportación (v1.json → «paquete», crates/protocolo/src/paquete.rs)");
if (v1.paquete) {
  const pq = v1.paquete;
  const cif = cifrarPaquete(deB64(pq.k_exp), pq.sal_cliente, utf8(pq.json), () => deB64(pq.nonce));
  igual("paquete cifrado", aB64(cif), pq.paquete);
  const des = descifrarPaquete(deB64(pq.k_exp), deB64(pq.paquete));
  igual("paquete: sal de la cabecera", des.sal, pq.sal_cliente);
  igual("paquete: se descifra", deUtf8(des.json), pq.json);
  let recortado = false;
  try {
    descifrarPaquete(deB64(pq.k_exp), deB64(pq.paquete).subarray(0, deB64(pq.paquete).length - 1));
  } catch {
    recortado = true;
  }
  cierto("paquete: recortado no se acepta", recortado);
}

// Ganchos v1.10: las mismas reglas que crates/motor/src/ganchos.rs.
{
  console.log("\n· Ganchos de plantilla (v1.10, motor/ganchos.rs)");
  cierto("instancia «.»", instanciaValida("."));
  cierto("instancia «localhost,1433»", instanciaValida("localhost,1433"));
  cierto("instancia «SERVIDOR-01\\SQLEXPRESS»", instanciaValida("SERVIDOR-01\\SQLEXPRESS"));
  cierto("instancia «a\\b\\c» no", !instanciaValida("a\\b\\c"));
  cierto("instancia «a;b» no", !instanciaValida("a;b"));
  cierto("instancia «a,0» no", !instanciaValida("a,0"));
  cierto("base «Contab 2026»", baseValida("Contab 2026"));
  cierto("base « x» no (espacio al principio)", !baseValida(" x"));
  cierto("base «a;b» no", !baseValida("a;b"));
  cierto("ruta «C:\\ResguardoVolcados»", rutaValida("C:\\ResguardoVolcados"));
  cierto("ruta relativa no", !rutaValida("Volcados"));
  cierto("ruta con «..» no", !rutaValida("C:\\a\\..\\b"));
  cierto("ruta con comilla no", !rutaValida("C:\\a'b"));
  const sql = { tipo: "sqlserver" as const, instancia: ".", bases: ["WO_Empresa", "Contab 2026"], carpeta: "C:\\ResguardoVolcados" };
  cierto("sqlserver válido", errorGancho(sql) === null);
  cierto("sqlserver con base repetida (sin mayúsculas) no", errorGancho({ ...sql, bases: ["a", "A"] }) !== null);
  cierto("sqlserver: «.» no se envía (es la predeterminada)", !("instancia" in limpiar(sql)));
  cierto("carpeta_reciente: 0 horas no", errorGancho({ tipo: "carpeta_reciente", carpeta: "D:\\WO\\Copias", horas: 0 }) !== null);
  cierto("carpeta_reciente: ruta de red vale", errorGancho({ tipo: "carpeta_reciente", carpeta: "\\\\nas\\wo", horas: 26 }) === null);
  igual("carpeta_reciente: extensión con punto", JSON.stringify(limpiar({ tipo: "carpeta_reciente", carpeta: "D:\\WO", horas: 26, extension: "bak" })), JSON.stringify({ tipo: "carpeta_reciente", carpeta: "D:\\WO", horas: 26, extension: ".bak" }));
  // Carpetas locales (v1.10, platform.rs carpeta_local_valida): los mismos casos que su prueba.
  for (const bien of ["E:\\Resguardo-espejo", "D:\\Copias\\Sur"]) cierto(`carpeta local «${bien}»`, errorCarpetaLocal(bien, true) === null);
  for (const mal of ["E:\\", "\\\\nas\\copias", "\\\\?\\E:\\x", "relativa", "E:\\a\\..\\b", "E:/x", "E:\\\\x"]) cierto(`carpeta local «${mal}» no`, errorCarpetaLocal(mal, true) !== null);
  cierto("espejo en C:\\Windows no", errorCarpetaEspejo("C:\\Windows\\Temp\\x", true) !== null);
  cierto("espejo en Program Files no", errorCarpetaEspejo("C:\\Program Files\\Resguardo\\x", true) !== null);
  cierto("carpeta local Linux «/» no", errorCarpetaLocal("/", false) !== null && errorCarpetaLocal("/srv/resguardo", false) === null);
  cierto("versión 0.7.2 admite ganchos", versionAlMenos("0.7.2", "0.7.2") && versionAlMenos("0.10.0", "0.7.2") && !versionAlMenos("0.7.1", "0.7.2"));
}

// Espejo v1.9: quitar un destino (o todo) es destructiva; añadir, no.
{
  console.log("\n· Espejo del Servidor de copias (v1.9) y resultados del agente");
  const carpeta = { tipo: "carpeta" as const, carpeta: "E:\\Espejo" };
  const nube = { tipo: "nube" as const, nube: "Dropbox Oficina", carpeta: "Resguardo/Sur" };
  const ctx = { espejo: { destinos: [carpeta] } };
  cierto("espejo: añadir una nube no es destructiva", !esDestructiva("guarda_copias", { espejo: { destinos: [carpeta, nube], hora: "02:00" } }, 24, ctx));
  cierto("espejo: quitar la carpeta es destructiva", esDestructiva("guarda_copias", { espejo: { destinos: [nube], hora: "02:00" } }, 24, ctx));
  cierto("espejo: la misma carpeta con barra final no cuenta como quitada", !esDestructiva("guarda_copias", { espejo: { destinos: [{ tipo: "carpeta", carpeta: "E:\\Espejo\\" }] } }, 24, ctx));
  cierto("espejo: null es destructiva", esDestructiva("guarda_copias", { espejo: null }, 24, ctx));
  cierto("espejo: el primero, sin espejo antes, no es destructiva", !esDestructiva("guarda_copias", { espejo: { destinos: [carpeta] } }, 24, { espejo: null }));
  cierto("espejo: forma antigua { carpeta } con la misma carpeta", !esDestructiva("guarda_copias", { espejo: { carpeta: "E:\\Espejo", hora: "02:00" } }, 24, ctx));
  cierto("config: vaciar las copias es destructiva", esDestructiva("config", { config: { v: 1, copias: [] } }, 24, { copiasActivas: 2 }));
  cierto("config: desactivarlas todas es destructiva", esDestructiva("config", { config: { v: 1, copias: [{ id: "a", activa: false }] } }, 24, { copiasActivas: 1 }));
  cierto("config: con una activa no es destructiva", !esDestructiva("config", { config: { v: 1, copias: [{ id: "a", activa: false }, { id: "b", activa: true }] } }, 24, { copiasActivas: 2 }));
  cierto("config: sin copias antes no es destructiva", !esDestructiva("config", { config: { v: 1, copias: [] } }, 24, { copiasActivas: 0 }));
  cierto("resultado del agente: «Espejo hecho…» no es error", !resultadoConError("Espejo hecho: 12 archivos nuevos."));
  cierto("resultado del agente: «ERROR: …» es error", resultadoConError("ERROR: Dropbox respondió 429."));
}

// Detalle de una copia (v1.12): estado, última vuelta, atrasada y errores en palabras.
{
  console.log("\n· Detalle de una copia (v1.12, lib/copia.ts)");
  // Viernes 2 de octubre de 2026, 15:00 (hora local).
  const ahora = new Date(2026, 9, 2, 15, 0).getTime();
  const iso = (d: number, h: number, m = 0) => new Date(2026, 9, d, h, m).toISOString();
  const k: CopiaResumen = { id: "docs", nombre: "Documentos", repo: "r1", horario: { dias: [1, 2, 3, 4, 5], horas: ["13:00"] }, carpetas: 2, activa: true, ultima: { cuando: iso(1, 13, 5), estado: "ok" } };
  const repo: RepoInforme = {
    id: "r1",
    nombre: "R1",
    versiones: [
      { id: "aaaa0001", hora: iso(2, 13, 4), copia: "docs", total_bytes: 5_000, anadido: 100, anadido_empaquetado: 60, archivos_nuevos: 1, archivos_cambiados: 2, archivos_sin_cambios: 10, duracion_s: 30, etiquetas: [] },
      { id: "bbbb0002", hora: iso(2, 12, 0), copia: "otra", total_bytes: 9_000, anadido: 1, anadido_empaquetado: 1, archivos_nuevos: 0, archivos_cambiados: 0, archivos_sin_cambios: 1, duracion_s: 5, etiquetas: [] },
    ],
    versiones_leidas: null,
    ejecuciones: [
      { hora: iso(2, 13, 4), copia: "docs", resultado: "ok", mensaje_corto: null, duracion_s: 30 },
      { hora: iso(1, 13, 5), copia: "docs", resultado: "fallo", mensaje_corto: "No se pudo conectar con el servidor de copias.", duracion_s: 4 },
      { hora: iso(2, 12, 0), copia: "otra", resultado: "ok", mensaje_corto: null },
    ],
    espacio: null,
    verificacion: null,
    prueba_restauracion: null,
    externa: null,
    proteccion: null,
  };
  const informe: Informe = { recibido: iso(2, 14), datos: { repos: [repo], proximas: { docs: iso(5, 13) } } };
  const solo = infCopia(repo, "docs")!;
  igual("infCopia: solo sus versiones y vueltas", [solo.versiones.length, solo.ejecuciones.length], [1, 2]);
  const v = ultimaVuelta(k, informe);
  igual("ultimaVuelta: la del informe, más nueva que la del resumen", [v?.cuando, v?.resultado], [iso(2, 13, 4), "ok"]);
  igual("proximaDe: del informe si el resumen no la trae", proximaDe(k, informe), iso(5, 13));
  igual("proximaDe: del horario con un agente que no la dice", proximaDe(k, { recibido: iso(2, 14), datos: {} }, ahora), new Date(2026, 9, 5, 13, 0).toISOString());
  igual("proximaDe: null si el informe dice que no hay", proximaDe(k, { recibido: iso(2, 14), datos: { proximas: { docs: null } } }, ahora), null);
  igual("ultimaProgramada: hoy a las 13:00", ultimaProgramada(k.horario, ahora), new Date(2026, 9, 2, 13, 0).getTime());
  igual("ultimaProgramada: el sábado, la del viernes", ultimaProgramada(k.horario, new Date(2026, 9, 3, 10, 0).getTime()), new Date(2026, 9, 2, 13, 0).getTime());
  cierto("atrasada: no, hecha después de su hora", !atrasada(k, v, false, ahora));
  cierto("atrasada: sí, la última fue ayer y hoy ya tocaba hace 2 h", atrasada(k, { cuando: iso(1, 13, 5), resultado: "ok", mensaje: null }, false, new Date(2026, 9, 2, 15, 30).getTime()));
  cierto("atrasada: no si está en pausa", !atrasada(k, null, true, ahora));
  igual("estado: al día", estadoCopia(k, v, false, ahora).texto, "Al día");
  igual("estado: con error", estadoCopia(k, { cuando: iso(2, 13, 4), resultado: "fallo", mensaje: null }, false, ahora).texto, "Con error");
  igual("estado: desactivada", estadoCopia({ ...k, activa: false }, v, false, ahora).texto, "Desactivada");
  igual("estado: nunca hecha", estadoCopia({ ...k, horario: { dias: [6], horas: ["13:00"] }, ultima: null }, null, false, ahora).texto, "Sin copias todavía");
  const cf = cifrasCopia(solo);
  igual("cifras: vueltas, correctas, fallidas, duración media, tamaño, añadido", [cf.vueltas, cf.correctas, cf.fallidas, cf.duracionMedia, cf.tamano, cf.anadido], [2, 1, 1, 17, 5_000, 60]);
  igual("error: sin conexión", explicarError("No se pudo conectar con el servidor de copias.").titulo, "No se pudo llegar al destino");
  igual("error: archivos en uso", explicarError("3 archivos en uso no se pudieron leer.").titulo, "Algunos archivos no se pudieron leer");
  igual("error: volcado", explicarError("El volcado de SQL Server falló: inicio de sesión").ayuda, "si-volcado");
  igual("error: desconocido", explicarError("algo raro").ayuda, "si-copia-falla");
  {
    const vs = [{ ...repo.versiones[0], hora: iso(2, 13, 3) }];
    igual("versionDeVuelta: la que empezó durante la vuelta", versionDeVuelta(vs, { hora: iso(2, 13, 4), copia: "docs", resultado: "ok", mensaje_corto: null, duracion_s: 70 })?.id, "aaaa0001");
    igual("versionDeVuelta: ninguna para «sin cambios»", versionDeVuelta(vs, { hora: iso(2, 13, 4), copia: "docs", resultado: "sin_cambios", mensaje_corto: null }), null);
    igual("versionDeVuelta: ninguna de otra copia", versionDeVuelta(vs, { hora: iso(2, 13, 4), copia: "otra", resultado: "ok", mensaje_corto: null }), null);
  }
  igual("bytesRepo: del informe si el resumen no lo trae", bytesRepo({ id: "r1", nombre: "R1", destino: "d" }, repo), 5_000);
}

// Carpetas de destino y carpetas nuevas (v1.15): las mismas reglas que el agente (sesiones_v2.rs).
{
  for (const bien of ["Resguardo", "Copias 2026", "Ñandú", "COM0", "CONSOLA"]) igual(`nombre de carpeta válido: ${bien}`, errorNombreCarpeta(bien), null);
  for (const mal of ["", " x", "x ", ".", "..", "a/b", "a\\b", "a:b", "a*", 'a"b', "fin.", "CON", "con.txt", "LPT1", "x".repeat(101)])
    igual(`nombre de carpeta no válido: ${JSON.stringify(mal).slice(0, 20)}`, errorNombreCarpeta(mal) !== null, true);
  igual("destino: E:\\Resguardo vale", errorCarpetaDestino("E:\\Resguardo", true), null);
  igual("destino: la raíz no", errorCarpetaDestino("E:\\", true) !== null, true);
  igual("destino de un repositorio: la raíz de otro disco sí", errorCarpetaDestino("E:\\", true, true), null);
  igual("destino de un repositorio: C:\\ no", errorCarpetaDestino("C:\\", true, true) !== null, true);
  igual("destino de un repositorio: / no", errorCarpetaDestino("/", false, true) !== null, true);
  igual("destino: Windows no", errorCarpetaDestino("C:\\Windows\\Temp", true) !== null, true);
  igual("destino: Program Files no", errorCarpetaDestino("c:\\program files\\x", true) !== null, true);
  igual("destino: ProgramData\\ResguardoAgente no", errorCarpetaDestino("C:\\ProgramData\\ResguardoAgente", true) !== null, true);
  igual("destino: C:\\WindowsCopias sí", errorCarpetaDestino("C:\\WindowsCopias", true), null);
  igual("destino: /srv/copias sí", errorCarpetaDestino("/srv/copias", false), null);
  igual("destino: /etc/x no", errorCarpetaDestino("/etc/x", false) !== null, true);
  igual("volcados: no en Windows", errorGancho({ tipo: "sqlserver", bases: ["WO"], carpeta: "C:\\Windows\\Volcados" }) !== null, true);
}

// ---------------------------------------------------------------------------
console.log("\n· Repositorios que ya existen (v1.14: adoptar_repositorio y copiar_historial)");
igual("rest con carpeta", partirDireccion("rest", "http://192.168.1.30:8001/Siigo"), { donde: "http://192.168.1.30:8001", ruta: "Siigo" });
igual("rest con «rest:» y barra final", partirDireccion("rest", "rest:https://nas:8000/ana/portatil/"), { donde: "https://nas:8000/ana", ruta: "portatil" });
igual("rest en la raíz", partirDireccion("rest", "https://nas:8000/"), { donde: "https://nas:8000", ruta: "" });
igual("local de Windows", partirDireccion("local", "D:\\Copias\\Siigo\\"), { donde: "D:\\Copias", ruta: "Siigo" });
igual("local en la raíz del disco", partirDireccion("local", "E:\\Siigo"), { donde: "E:\\", ruta: "Siigo" });
igual("local de Linux", partirDireccion("local", "/srv/copias/siigo"), { donde: "/srv/copias", ruta: "siigo" });
igual("b2 cubo:carpeta", partirDireccion("b2", "b2:cubo:siigo"), { donde: "cubo", ruta: "siigo" });
igual("b2 con subcarpeta", partirDireccion("b2", "cubo:clientes/siigo"), { donde: "cubo:clientes", ruta: "siigo" });
igual("s3", partirDireccion("s3", "s3.amazonaws.com/cubo/siigo"), { donde: "s3.amazonaws.com/cubo", ruta: "siigo" });
igual("usuario en el almacén", usuarioEnAlmacen("https://192.168.1.50:8002/servidor-01/"), "servidor-01");
igual("usuario en el almacén, con «rest:» y sin barra", usuarioEnAlmacen("rest:https://almacen:8002/servidor-01"), "servidor-01");
igual("sin usuario: no es un almacén", usuarioEnAlmacen("https://192.168.1.50:8002"), "");
igual("dos niveles: no es un almacén", usuarioEnAlmacen("https://nas:8000/ana/portatil"), "");
igual("ruta en un almacén de Linux", rutaEnAlmacen("/mnt/restic/resguardo/", "servidor-01", "siigo"), "/mnt/restic/resguardo/servidor-01/siigo");
igual("ruta en un almacén de Windows", rutaEnAlmacen("E:\\Resguardo", "servidor-01", "siigo"), "E:\\Resguardo\\servidor-01\\siigo");
{
  const r = { tipo: "rest" as const, direccion: "https://nas:8000/Siigo", usuario: "ana", secreto: "s3creto", ca: "", contrasena: "clave" };
  igual("destino sin certificado vacío", destinoCuerpo(r, { id: "d1" }), { id: "d1", tipo: "rest", donde: "https://nas:8000", usuario: "ana", secreto: "s3creto" });
  igual("origen", origenCuerpo({ ...r, tipo: "local", direccion: "D:\\Copias\\Siigo" }), { destino: { tipo: "local", donde: "D:\\Copias" }, ruta: "Siigo", contrasena: "clave" });
}
igual("nivel de las órdenes nuevas", [NIVEL.adoptar_repositorio, NIVEL.copiar_historial], ["admin", "admin"]);

// ---------------------------------------------------------------------------
console.log("\n· Retención en el almacén (v1.22)");
igual("niveles (como protocolo/ordenes.rs)", [NIVEL.clave_almacen, PIDE_TAMBIEN_ADMIN.has("clave_almacen"), NIVEL.retencion_almacen, NIVEL.aplicar_retencion_almacen], ["repo", true, "admin", "admin"]);
igual("poner la regla espera", esDestructiva("retencion_almacen", { usuario: "caja-1", repo: "caja", retencion: REGLA_POR_DEFECTO }), true);
igual("quitarla no espera", esDestructiva("retencion_almacen", { usuario: "caja-1", repo: "caja", quitar: true }), false);
igual("aplicar ahora espera", esDestructiva("aplicar_retencion_almacen", { usuario: "caja-1", repo: "caja" }), true);
igual("la clave no espera", esDestructiva("clave_almacen", { repo: "caja", clave: "x" }), false);
igual("leer la regla del resumen", leerRegla("7 diarias · 4 semanales · 12 mensuales · 2 anuales"), { diarias: 7, semanales: 4, mensuales: 12, anuales: 2 });
igual("leer la regla con comas y sin anuales", leerRegla("7 diarias, 4 semanales, 12 mensuales"), { diarias: 7, semanales: 4, mensuales: 12, anuales: 0 });
igual("sin regla", leerRegla(null), null);
igual("regla vacía no vale", errorRegla({ diarias: 0, semanales: 0, mensuales: 0, anuales: 0 }) !== null, true);
igual("regla de más de 1000 no vale", errorRegla({ diarias: 1001, semanales: 0, mensuales: 0, anuales: 0 }) !== null, true);
// v1.28: horarias, plazos y «siempre» (como `Retencion` del agente y `restic forget`).
{
  const siigo = PRESETS.find((p) => p.id === "contables")!.regla;
  igual("Siigo: texto como el agente", textoRegla(siigo), "horarias 15 días · diarias 1 año · mensuales siempre");
  igual("lo de antes: texto de siempre", textoRegla(REGLA_POR_DEFECTO), "7 diarias · 4 semanales · 12 mensuales · 2 anuales");
  igual("Siigo: en palabras", resumenRegla(siigo).split(".")[0], "Conserva una por hora durante 15 días, una por día durante 1 año y una por mes siempre");
  igual("Siigo vale", errorRegla(siigo), null);
  igual("Siigo con un agente anterior: «actualízalo»", /actualízalo/.test(errorRegla(siigo, false) ?? ""), true);
  igual("lo de antes con un agente anterior: vale", errorRegla(REGLA_POR_DEFECTO, false), null);
  igual("plazo que no vale", errorRegla({ ...REGLA_POR_DEFECTO, plazos: { diarias: "15 días" } }) !== null, true);
  igual("solo plazos: vale", errorRegla({ diarias: 0, semanales: 0, mensuales: 0, anuales: 0, plazos: { diarias: "30d" } }), null);
  igual("plazos de restic", [leerPlazo("1y6m"), leerPlazo("0d"), leerPlazo("15w"), leerPlazo("300y")], [{ anos: 1, meses: 6, dias: 0, horas: 0 }, null, null, null]);
  igual("plazo en palabras", plazoEnPalabras("1y6m"), "1 año y 6 meses");
  igual("para la orden: sin vacíos", reglaParaOrden({ horarias: 0, diarias: 0, semanales: 0, mensuales: -1, anuales: 0, plazos: { horarias: "15d", diarias: "", semanales: null } }), {
    diarias: 0,
    semanales: 0,
    mensuales: -1,
    anuales: 0,
    plazos: { horarias: "15d" },
  });
  igual("el preset de una regla", [presetDe(siigo), presetDe(REGLA_POR_DEFECTO), presetDe({ ...REGLA_POR_DEFECTO, diarias: 8 })], ["contables", "clasica", "personalizada"]);
  igual("31 de marzo menos un mes (Go)", restarPlazo(new Date(2026, 2, 31, 10), { anos: 0, meses: 1, dias: 0, horas: 0 }).getTime(), new Date(2026, 2, 3, 10).getTime());
  // Una copia cada hora durante dos años: 360 horarias + ~350 diarias + las mensuales de antes.
  const ahora = new Date(2026, 9, 4, 12, 0);
  const e = estimarVersiones(siigo, undefined, ahora);
  cierto(`Siigo cada hora: unas 710 versiones (${e.versiones}), 12 más cada año (${e.porAno})`, e.versiones >= 705 && e.versiones <= 730 && e.porAno === 12);
  const e2 = estimarVersiones(PRESETS.find((p) => p.id === "30d6m")!.regla, () => ["13:00"], ahora);
  cierto(`Diarias 30 días, semanales 6 meses con una copia al día: unas 51 (${e2.versiones}), sin crecer`, e2.versiones >= 48 && e2.versiones <= 54 && e2.porAno === 0);
  const e3 = estimarVersiones(REGLA_POR_DEFECTO, () => ["13:00"], ahora);
  cierto(`7/4/12/2 con una al día: unas 20 (${e3.versiones})`, e3.versiones >= 18 && e3.versiones <= 22 && e3.porAno === 0);
}
// v1.28: verificación automática (como `VerificacionAuto` del agente).
igual("verificación: partes como el agente", [0, 1, 5, 10, 60, 100].map(partesVerificacion), [null, 52, 20, 10, 2, null]);
igual("verificación: en palabras", fraseVerificacion({ cada_dias: 7, porcentaje: 10 }), "Cada 7 días, el 10 % de los datos: todo el repositorio en 10 verificaciones (unos 70 días).");
igual("verificación: solo estructura", fraseVerificacion({ cada_dias: 1, porcentaje: 0 }), "Cada día, solo la estructura (rápida: no lee los datos).");
igual("verificación: límites", [errorVerificacion({ cada_dias: 0, porcentaje: 5 }) !== null, errorVerificacion({ cada_dias: 32, porcentaje: 5 }) !== null, errorVerificacion({ cada_dias: 31, porcentaje: 100 })], [true, true, null]);
igual("horario como el agente (domingos)", textoHorario({ dias: [7], hora: "03:00" }), "los domingos a las 03:00");
igual("horario como el agente (varios)", textoHorario({ dias: [4, 1], hora: "22:30" }), "lunes y jueves a las 22:30");
igual("horario como el agente (todos)", textoHorario({ dias: [1, 2, 3, 4, 5, 6, 7], hora: "03:00" }), "cada día a las 03:00");
igual("horario sin días no vale", errorHorario({ dias: [], hora: "03:00" }) !== null, true);
igual("hora mal escrita no vale", errorHorario({ dias: [7], hora: "3:00" }) !== null, true);
igual("clave del almacén: 32 bytes en base64url", /^[A-Za-z0-9_-]{43}$/.test(nuevaClave()), true);
{
  const almacen = { id: "0a0e1b2c-0000-4000-8000-0000000000e5", nombre: "ALMACEN-01", modo: "gestionado", resumen: { guarda_copias: { activo: true, retenciones: [{ usuario: "servidor-01", repo: "Siigo", retencion: REGLA_POR_DEFECTO, horario: { dias: [7], hora: "03:00" } }] } } } as unknown as Equipo;
  const destino = { id: "almacen-0a0e1b2c", nombre: "ALMACEN-01", tipo: "rest" as const, donde: "https://192.168.1.50:8002/servidor-01/" };
  const adoptado = { id: "siigo-adoptado", nombre: "Siigo", destino: destino.id, ruta: "Siigo" };
  const en = almacenDe(adoptado, destino, [almacen]);
  igual("almacén de un repositorio adoptado (usuario y carpeta)", [en?.almacen.nombre, en?.usuario, en?.carpeta, en?.admite, en?.retencion?.repo], ["ALMACEN-01", "servidor-01", "Siigo", true, "Siigo"]);
  igual("uno creado con «Copiar en…»: su id es la carpeta", almacenDe({ id: "almacen-a1b2", nombre: "X", destino: destino.id }, destino, [almacen])?.carpeta, "almacen-a1b2");
  igual("en otro rest-server no hay almacén", almacenDe(adoptado, { ...destino, id: "otro", nombre: "NAS" }, [almacen]), null);
  igual("un almacén con agente anterior no la admite", almacenDe(adoptado, destino, [{ ...almacen, resumen: { guarda_copias: { activo: true } } } as Equipo])?.admite, false);
  // El agente pone en `repositorios[].destino` el NOMBRE del destino (api-servidor.md §4), no su id:
  // la consola lo encuentra igual (antes, solo por id: ningún repositorio de verdad tenía destino).
  const otro = { id: "nas-1", nombre: "NAS", tipo: "rest" as const, donde: "https://nas:8000/x/" };
  igual("destino de un repositorio por su nombre (como lo da el agente)", destinoDe([otro, destino], { destino: "ALMACEN-01" })?.id, destino.id);
  igual("destino de un repositorio por su id", destinoDe([otro, destino], { destino: destino.id })?.id, destino.id);
  igual("destino que no está", destinoDe([otro], { destino: "ALMACEN-01" }), undefined);
  const delAgente = { id: "almacen-a1b2", nombre: "X", destino: "ALMACEN-01" };
  igual("«Retención en el almacén» con el resumen real del agente", almacenDe(delAgente, destinoDe([otro, destino], delAgente), [almacen])?.usuario, "servidor-01");
}

console.log("\n· Progreso en vivo (v1.25)");
{
  const NB = " ";
  const t: TareaEnMarcha = { tipo: "copia", repo: "r", copia: "k", nombre: "Documentos", fase: "subiendo", porcentaje: 0.4299, archivos: 120, archivos_total: 3000, bytes: 4_000_000_000, bytes_total: 10_000_000_000, velocidad: 12_000_000, quedan_s: 190 };
  igual("porcentaje entero, sin redondear hacia arriba", porcentaje(t), 42);
  igual("cifras de una copia", cifrasTarea(t), [`120 de ${(3000).toLocaleString("es")} archivos`, `4${NB}GB de 10${NB}GB`, `12${NB}MB/s`, `quedan ~3${NB}min`]);
  igual("chip", textoCorto(t), `Copiando… 42${NB}%`);
  igual("preparando: sin porcentaje", textoCorto({ tipo: "copia", repo: "r", fase: "preparando" }), "Preparando…");
  igual("fase de los ganchos", textoFase({ tipo: "copia", repo: "r", fase: "antes_de_copiar" }), "Antes de copiar: volcados y comprobaciones");
  igual("una verificación dice su etapa", textoFase({ tipo: "verificar", repo: "r", fase: "en_marcha", etapa: "Leyendo el 5 % de los datos…" }), "Leyendo el 5 % de los datos");
  igual("copia externa: versiones", cifrasTarea({ tipo: "copia_externa", repo: "r", fase: "en_marcha", versiones: 2, versiones_total: 5 }), ["2 de 5 versiones"]);
  igual("queda poco", textoQuedan(30), "queda menos de 1 min");
  igual("quedan horas", textoQuedan(4_980), `quedan ~1${NB}h 23${NB}min`);
  igual("sin estimación", textoQuedan(null), null);
}

// El simulador hace lo que el agente con las órdenes que abren una sesión
// (servidor_v2.rs: «hecha», firmada, al abrirla) y cerrarla no cambia la orden:
// antes la daba por «hecha» sin firmar y Órdenes decía «Firma no válida».
{
  console.log("\n· Sesiones del simulador: el resultado sigue firmado al cerrar");
  const mock = await import("../src/mock/estado");
  const { procesarOrden, cerrarSesion } = await import("../src/mock/agente");
  await mock.sembrar();
  const estado = mock.estado; // después de sembrar (es un «let» del módulo)
  // Un equipo con la contraseña de algún repositorio (la que pide «explorar»).
  const e = estado.equipos.find((x) => Object.keys(x.contrasenas).length)!;
  const [repo, contrasena] = Object.entries(e.contrasenas)[0];
  for (const [i, tipo] of ["explorar", "abrir_sesion"].entries()) {
    const seq = 9000 + i;
    const sesion = crypto.randomUUID();
    const plana = { v: 2, cliente: e.cliente, equipo: e.id, seq, nonce: "n", emitida: new Date().toISOString(), caduca: new Date(Date.now() + 600_000).toISOString(), not_before: null, tipo, cuerpo: { sesion, clave_sesion: aB64(new Uint8Array(32).fill(7)) }, autorizacion: tipo === "explorar" ? { clave_repo: { repo, contrasena } } : {}, responder_a: null };
    const o = { ...estado.ordenes[0], id: crypto.randomUUID(), cliente: e.cliente, equipo: e.id, seq, tipo, caduca: plana.caduca, estado: "pendiente" as const, mensaje: null, detalle: null, firma_agente: null, not_before: null, sesion, relevo: null, sellado: aB64(sellar(deB64(e.box_pub), utf8(JSON.stringify(plana)))) };
    estado.ordenes.push(o);
    await procesarOrden(o);
    cierto(`${tipo}: firmada al abrir la sesión`, o.estado === "hecha" && resultadoFirmado(e.sign_pub, o));
    cerrarSesion(sesion);
    cierto(`${tipo}: sigue firmada al cerrarla`, o.estado === "hecha" && resultadoFirmado(e.sign_pub, o));
  }
}

// Notificaciones (api-servidor.md §13): el vector de la firma del webhook del contrato
// (el mismo que comprueba transporte.rs) con otra implementación, y qué cambios de un
// canal piden el código de la aplicación de autenticación (lo mismo que el servidor).
{
  console.log("\n· Notificaciones: firma del webhook y cambios que piden el código");
  const firma = createHmac("sha256", "secreto-compartido-123").update('1791100800.{"a":1}').digest("hex");
  igual("firma del webhook (vector del contrato)", firma, "d0524a455d5fce0165253c72ecdb2ae9f8f8e523d44886ab7a258a155dd39931");
  const { cambioSensible, reglasPorDefecto, textoSeveridades } = await import("../src/lib/notificaciones");
  const previo = {
    id: "k",
    tipo: "correo" as const,
    nombre: "Correo",
    activo: true,
    config: { host: "smtp.ejemplo.com", puerto: 587, seguridad: "starttls" as const, usuario: "avisos@ejemplo.com", remitente: "Resguardo <avisos@ejemplo.com>" },
    secretos: { contrasena: "configurado" as const },
    reglas: reglasPorDefecto(),
    completo: true,
    actualizado: "",
    por: "",
  };
  cierto("crear un canal pide el código", cambioSensible(null, { tipo: "webhook" }));
  cierto("nombre, encendido y reglas, no", !cambioSensible(previo, { nombre: "Otro", activo: false, reglas: { ...reglasPorDefecto(), severidades: ["critico"] } }));
  cierto("la misma configuración, no", !cambioSensible(previo, { config: { ...previo.config, host: " SMTP.ejemplo.com " } }));
  cierto("otro servidor de correo (se llevaría la contraseña), sí", cambioSensible(previo, { config: { ...previo.config, host: "smtp.de-otro.com" } }));
  cierto("un secreto nuevo o quitarlo, sí", cambioSensible(previo, { secretos: { contrasena: "" } }));
  igual("gravedades en palabras", textoSeveridades(["importante", "critico"]), "Crítico e importante");
  igual("ninguna", textoSeveridades([]), "Nada al momento");
}

// v1.35: varias consolas a la vez. El código de conexión va y vuelve igual, se
// rechaza caducado, de esta misma consola o dañado, y `anadir_consola` lleva lo
// que el agente comprueba (consolas_v2.rs, `pedido`).
{
  console.log("\n· Varias consolas: código de conexión y anadir_consola");
  const { crearCodigo, cuerpoAnadir, huellaConPuntos, huellaHex, leerCodigo } = await import("../src/lib/conexion");
  const datos = {
    url: "https://consola.ejemplo.com",
    identidad: aB64(new Uint8Array(32).fill(5)),
    huella_ca: Array(32).fill("ab").join(":"),
    ficha: "ABCDEFGHIJKLMNOPQRSTUVWXYZ234567ABCDEFG",
    sal_cliente: aB64(new Uint8Array(16).fill(3)),
    nombre: "Consola en línea",
    cliente: "Café del Sur",
    caduca: new Date(Date.now() + 7 * 86_400_000).toISOString(),
  };
  const codigo = crearCodigo(datos);
  cierto("empieza por RGC1. y no lleva espacios ni «+/=»", codigo.startsWith("RGC1.") && !/[\s+/=]/.test(codigo));
  const leido = leerCodigo(` ${codigo.slice(0, 40)}\n${codigo.slice(40)} `);
  igual("va y vuelve (aunque se pegue partido)", leido, { ...datos, huella_ca: "AB".repeat(32) });
  cierto("caducado", String(leerCodigo(crearCodigo({ ...datos, caduca: new Date(Date.now() - 1000).toISOString() }))).includes("caducó"));
  cierto("de esta misma consola", String(leerCodigo(codigo, new Date(), "https://consola.ejemplo.com")).includes("misma"));
  cierto("sin https", typeof leerCodigo(crearCodigo({ ...datos, url: "http://consola.ejemplo.com" })) === "string");
  cierto("dañado", typeof leerCodigo(codigo.slice(0, -6)) === "string");
  cierto("otra cosa", String(leerCodigo('{ "url": "https://x" }')).includes("RGC1"));
  igual("huella con «:»", huellaConPuntos("abcd"), "AB:CD");
  igual("huella sin «:»", huellaHex("ab:CD:ef"), "ABCDEF");
  const c = cuerpoAnadir(leido as Exclude<typeof leido, string>, aB64(new Uint8Array(32).fill(4)), aB64(new Uint8Array(16).fill(1)));
  igual("anadir_consola lleva lo que pide el agente", Object.keys(c).sort(), ["ficha", "huella_ca", "identidad", "k_cfg", "nombre", "sal_cliente", "sal_origen", "url"]);
  igual("la huella, con «:» (como la de GET /api/servidor)", c.huella_ca.split(":").length, 32);
  igual("nivel de las órdenes de consolas", [NIVEL.anadir_consola, NIVEL.quitar_consola], ["admin", "admin"]);
}

// «Mapa de la protección» (lib/mapa.ts): columnas, espejo, alternativa en texto y grupos con muchos equipos.
{
  console.log("\n· Mapa de la protección: columnas, espejo, frases y grupos");
  const { construirMapa, raices } = await import("../src/lib/mapa");
  const ahora = Date.parse("2026-10-04T12:00:00Z");
  const h = (horas: number) => new Date(ahora - horas * 3600_000).toISOString();
  const base = { so: "Windows 11", version_agente: "0.7.13", box_pub: "", sign_pub: "", sal_equipo: "", etiqueta: null, modo: "gestionado" as const, confirmado: true, conectado: true, ultimo_contacto: h(0.1), estado_servicio: "en_marcha" as const, siguiente_seq: 1 };
  const almacen: Equipo = {
    ...base,
    id: "alm",
    nombre: "ALMACEN-01",
    rol: "almacenamiento",
    resumen: {
      guarda_copias: {
        activo: true,
        espejo: {
          hora: "02:00",
          destinos: [
            { tipo: "carpeta", carpeta: "Disco 2", ultima: h(10), resultado: "Espejo hecho." },
            { tipo: "nube", nube: "Dropbox", carpeta: "Sur", ultima: h(2), resultado: "ERROR: Dropbox respondió 429." },
          ],
        },
      },
    },
  };
  const pc = (i: number, ok = true): Equipo => ({
    ...base,
    id: `pc${i}`,
    nombre: `PC-${String(i).padStart(2, "0")}`,
    rol: "agente",
    resumen: {
      destinos: [{ id: "d", nombre: "Almacén Sur", tipo: "rest", equipo_almacen: "alm" }],
      repositorios: [{ id: "r", nombre: `Docs ${i}`, destino: "d", versiones: 3, bytes: 1e9, ultima_version: h(3) }],
      copias: [{ id: "k", nombre: "Docs", repo: "r", activa: true, ultima: { cuando: h(3), estado: ok ? "ok" : "fallo" }, proxima: h(-5) }],
    },
  });
  const m = construirMapa([pc(1), almacen], {}, { cliente: "c", ahora });
  igual("columnas: equipo, repositorio, almacén y los dos espejos", m.nodos.map((n) => `${n.col}:${n.nombre}`), ["0:PC-01", "1:Docs 1", "2:ALMACEN-01", "3:Dropbox", "3:Disco 2"]);
  igual("el espejo que falla, en rojo", m.aristas.filter((a) => a.tipo === "espejo").map((a) => a.tono), ["ok", "bad"]);
  igual("en palabras (espacios finos aparte)", m.frases.map((x) => x.replace(/[  ]/g, " ")), ["PC-01 copia «Docs 1» a ALMACEN-01 (al día, hace 3 h).", "ALMACEN-01 se refleja en Disco 2 (al día, hace 10 h) y Dropbox (falló hace 2 h)."]);
  const vivo = construirMapa([pc(1), almacen], {}, { cliente: "c", ahora, enVivo: (_e, _r, t) => (t === "copia" ? "Copiando 40 %" : null) });
  cierto("con una copia en marcha, el trazo se mueve", vivo.aristas.filter((a) => a.tipo !== "espejo").every((a) => a.vivo));
  const muchos = [...Array.from({ length: 14 }, (_, i) => pc(i + 1, i !== 4)), almacen];
  const g = construirMapa(muchos, {}, { cliente: "c", ahora });
  igual("15 equipos: el que falla, suelto; los 13 al día, en un grupo", g.nodos.filter((n) => n.col === 0).map((n) => n.nombre), ["PC-05", "13 equipos"]);
  igual("el grupo lleva una píldora por destino", g.nodos.filter((n) => n.col === 1).map((n) => n.nombre), ["Docs 5", "13 repositorios"]);
  const solo = construirMapa(muchos, {}, { cliente: "c", ahora, raiz: { perspectiva: "equipos", id: "pc2" } });
  igual("un equipo elegido: solo lo suyo", solo.nodos.filter((n) => n.col < 2).map((n) => n.nombre), ["PC-02", "Docs 2"]);
  const delAlmacen = construirMapa(muchos, {}, { cliente: "c", ahora, raiz: { perspectiva: "equipos", id: "alm" }, agruparDesde: 99 });
  igual("el almacén elegido: lo que guardan en él los demás", delAlmacen.nodos.filter((n) => n.col === 0).length, 14);
  igual("raíces por destino", raices(muchos, "destinos"), [{ id: "al:alm", texto: "ALMACEN-01" }]);

  console.log("\n· ¿Cuándo se llena? (lib/llenado.ts): ritmo, plazo y aviso");
  const { previsiones, plazoEnPalabras } = await import("../src/lib/llenado");
  igual("plazos en palabras", [20, 100, 430, 900, 2000].map(plazoEnPalabras), ["~20 días", "~3 meses", "~14 meses", "~2,5 años", "más de 5 años"]);
  // 30 versiones, una al día, de 1 GB nuevo cada una: 1 GB al día.
  const versiones = Array.from({ length: 30 }, (_, i) => ({ id: `v${i}`, hora: h(i * 24 + 1), copia: "k", total_bytes: 1e11, anadido: 1e9, anadido_empaquetado: 1e9, archivos_nuevos: 1, archivos_cambiados: 0, archivos_sin_cambios: 0, duracion_s: 60, etiquetas: [] }));
  const inf: Informe = { recibido: h(0), datos: { repos: [{ id: "r", nombre: "Docs", versiones, versiones_leidas: null, ejecuciones: [], espacio: { en_disco_bytes: 5e10, sin_comprimir: null, ratio: null, leido: null }, verificacion: null, prueba_restauracion: null, externa: null, proteccion: null }] } };
  const lleno = (libre: number) => ({ ...almacen, resumen: { guarda_copias: { ...almacen.resumen!.guarda_copias!, espacio: { libre, total: 1e12 } } } });
  const [alm, disco2, dropbox] = previsiones([pc(1), lleno(60e9)], { pc1: inf }, "c", ahora);
  igual("el almacén: ritmo y plazo (30 GB en los 29 días que cubren las versiones; 60 GB libres)", [alm.nombre, Math.round((alm.porDia ?? 0) / 1e6), Math.round(((alm.lleno ?? 0) - ahora) / 86_400_000), alm.tono, alm.estado], ["ALMACEN-01", 1033, 58, "warn", "Queda poco sitio"]);
  igual("los destinos del espejo, sin su espacio: sin plazo", [disco2.nombre, disco2.lleno, dropbox.nombre, dropbox.estado], ["Disco 2", null, "Dropbox", "Sin capacidad"]);
  igual("la historia se reconstruye hacia atrás", [alm.serie.length, Math.round((alm.serie.at(-1)!.v - alm.serie[0].v) / 1e9)], [60, 30]);
  igual("con mucho sitio: sin aviso", previsiones([pc(1), lleno(900e9)], { pc1: inf }, "c", ahora)[0].tono, "ok");

  console.log("\n· Línea de tiempo de las versiones (lib/lineaTiempo.ts): retención, colores y eje");
  const { retencionDe, huecosDeCopia, marcasEje, zoomInicial } = await import("../src/lib/lineaTiempo");
  const { seQuedan } = await import("../src/lib/retencion");
  // Una versión cada 12 h durante 20 días, con «3 diarias y 2 semanales».
  const vs = Array.from({ length: 40 }, (_, i) => ({ id: `v${i}`, hora: new Date(ahora - i * 12 * 3600_000).toISOString() }));
  const regla = { diarias: 3, semanales: 2, mensuales: 0, anuales: 0 };
  const mot = retencionDe(vs, regla)!;
  igual("por qué se queda cada una (y las que la próxima retención quitaría)", [...mot.values()].filter(Boolean).length, seQuedan(vs.map((v) => new Date(v.hora)), regla).filter(Boolean).length);
  igual("las tres más recientes de días distintos son «diarias»", vs.filter((v) => mot.get(v.id) === "diarias").length, 3);
  cierto("la más antigua, ya sin semanales que gastar, se quitaría", mot.get("v39") === null);
  const hu = huecosDeCopia([{ id: "a" }, { id: "b" }, { id: "c" }, { id: "d" }], ["d", "b", "a", "c", null]);
  igual("colores por orden de configuración; la cuarta, «otras»", [hu.get("a"), hu.get("b"), hu.get("c"), hu.get("d")], [0, 1, 2, 3]);
  igual("eje del mes: el 1 y cada 5 días (sin el 30, pegado al 1)", marcasEje(Date.parse("2026-09-28T00:00:00"), Date.parse("2026-10-12T00:00:00"), "mes").map((m) => new Date(m.t).getDate()), [1, 5, 10]);
  igual("escala inicial: la menor con 6 versiones a la vista", zoomInicial(vs.map((v) => Date.parse(v.hora)), ahora), "semana");
}

console.log("\n· Cambiar la clave de administración (lib/cambioClave.ts)");
{
  const NUEVA = "otra clave de administración, bien larga";
  const sal = (n: number) => aB64(new Uint8Array(16).fill(n));
  const nueva = new ClaveNueva(argon2, NUEVA, sal(1));
  const equipo = {
    id: "e1",
    sal_equipo: sal(2),
    resumen: {
      consolas: [
        { id: "a", nombre: "Esta", url: "https://a", identidad: "IA", sal_cliente: sal(1), ultimo_contacto: null, desde: null, esta: true },
        { id: "b", nombre: "En línea", url: "https://b", identidad: "IB", sal_cliente: sal(3), ultimo_contacto: null, desde: null, esta: false },
        { id: "c", nombre: "Sin sal", url: "https://c", identidad: "IC", sal_cliente: null, ultimo_contacto: null, desde: null, esta: false },
      ],
    },
  } as unknown as Equipo;
  const c = await nueva.cuerpo(equipo);
  igual("verificador = SHA-256(Argon2id(nueva, sal_equipo))", c.verificador, aB64(verificador(await pruebaAdmin(argon2, NUEVA, sal(2)))));
  igual("k_cfg con la sal de esta consola", c.k_cfg, aB64(kCfg(await materialCliente(argon2, NUEVA, sal(1)))));
  igual("k_cfg_consolas: solo las otras con sal", Object.keys(c.k_cfg_consolas ?? {}), ["IB"]);
  igual("la de la otra, con su sal", c.k_cfg_consolas?.IB, aB64(kCfg(await materialCliente(argon2, NUEVA, sal(3)))));
  igual("sin otras consolas, sin k_cfg_consolas", "k_cfg_consolas" in (await nueva.cuerpo({ sal_equipo: sal(2), resumen: null } as unknown as Equipo)), false);
  igual("otras consolas por nombre", ClaveNueva.otrasConsolas([equipo]), ["En línea", "Sin sal"]);
  // Quién tiene qué clave, por su etiqueta.
  const kA = kCfg(await materialCliente(argon2, "clave anterior de prueba", sal(1)));
  const kN = deB64(c.k_cfg);
  const eq = (id: string, k: Uint8Array | null) => ({ id, box_pub: "B", sign_pub: "S", etiqueta: k ? etiquetaEquipo(k, id, "B", "S") : null });
  const r = repartir([eq("x", kA), eq("y", kN), eq("z", null)], kA, kN);
  igual("repartir por etiqueta", [r.conActual.map((e) => e.id), r.yaNueva.map((e) => e.id), r.otra.map((e) => e.id)], [["x"], ["y"], ["z"]]);
  // A medias: la última orden de cada equipo, si no ha terminado.
  const o = (equipo: string, estado: string, emitida: string) => ({ tipo: "cambiar_clave_admin", equipo, estado: estado as never, emitida, caduca: "2026-10-11T10:00:00+02:00" });
  igual(
    "pendientes del cambio",
    pendientesDeCambio([
      o("e1", "hecha", "2026-10-04T10:00:00+02:00"),
      o("e2", "pendiente", "2026-10-04T10:00:00+02:00"),
      o("e3", "rechazada", "2026-10-04T11:00:00+02:00"),
      o("e3", "pendiente", "2026-10-04T10:00:00+02:00"),
      { ...o("e4", "pendiente", "2026-10-04T10:00:00+02:00"), tipo: "config" },
    ]).map((p) => p.equipo),
    ["e2"],
  );
  igual(
    "estados",
    ["hecha", "entregada", "en_marcha", "caducada", "fallida"].map((e) => estadoCambio({ estado: e as never })),
    ["aplicada", "pendiente", "en_marcha", "cancelada", "rechazada"],
  );
  const g = claveGenerada((n) => new Uint8Array(n).map((_, i) => i * 7));
  cierto("clave generada: 5 grupos de 5, sin letras que se confundan", /^([A-HJ-NP-Z2-9]{5}-){4}[A-HJ-NP-Z2-9]{5}$/.test(g));
}

console.log(`\n${total - fallos} de ${total} comprobaciones correctas.`);
if (fallos) process.exit(1);
