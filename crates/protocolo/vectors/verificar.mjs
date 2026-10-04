// Comprueba los vectores de v1.json con la criptografía de Node (sin dependencias):
// hash del código de emparejamiento, SAS, X25519 (clave pública) y Ed25519 (firma).
// El sobre sellado (XSalsa20-Poly1305) necesita libsodium: lo comprobará la consola
// web con su implementación.
//
//   node crates/protocolo/vectors/verificar.mjs
import crypto from "node:crypto";
import fs from "node:fs";

const v = JSON.parse(fs.readFileSync(new URL("./v1.json", import.meta.url), "utf8"));
const b64 = (b) => Buffer.from(b).toString("base64");
const sha256 = (s) => crypto.createHash("sha256").update(s).digest();
let fails = 0;
const check = (name, got, want) => {
  const ok = got === want;
  if (!ok) fails++;
  console.log(`${ok ? "ok  " : "MAL "} ${name}${ok ? "" : `: ${got} ≠ ${want}`}`);
};

// Código de emparejamiento: solo letras y cifras, en mayúsculas.
for (const { code, hash } of v.code_hash) {
  const norm = code.replace(/[^0-9a-z]/gi, "").toUpperCase();
  check(`code_hash(${code})`, sha256(norm).toString("hex"), hash);
}

// SAS.
const h = sha256(`resguardo-sas-v1|${v.sas.console_sign_pub}|${v.sas.endpoint_box_pub}`);
const n = h.readUInt32BE(0) % 1_000_000;
check("sas", `${String(Math.floor(n / 1000)).padStart(3, "0")} ${String(n % 1000).padStart(3, "0")}`, v.sas.sas);

// Claves a partir de los 32 bytes (PKCS#8 con el OID de cada curva).
const pkcs8 = (oid, raw) => crypto.createPrivateKey({ key: Buffer.concat([Buffer.from(`302e020100300506032b65${oid}04220420`, "hex"), raw]), format: "der", type: "pkcs8" });
const rawPub = (priv) => Buffer.from(crypto.createPublicKey(priv).export({ format: "jwk" }).x, "base64url");

const x = pkcs8("6e", Buffer.from(v.x25519.secret, "base64"));
check("x25519 public", b64(rawPub(x)), v.x25519.public);

const ed = pkcs8("70", Buffer.from(v.ed25519.seed, "base64"));
check("ed25519 public", b64(rawPub(ed)), v.ed25519.public);
check("ed25519 firma", b64(crypto.sign(null, Buffer.from(v.ed25519.message_json), ed)), v.ed25519.signature);

const plain = JSON.parse(v.signed_envelope.plain);
check("sobre: payload", Buffer.from(plain.payload, "base64").toString(), v.ed25519.message_json);
check("sobre: firma", plain.sig, v.ed25519.signature);

// Derivaciones de la clave de administración (docs/api-servidor.md, §1).
const d = v.derivaciones;
const raw = (x) => Buffer.from(x, "base64");
if (typeof crypto.argon2Sync === "function") {
  const a2 = (sal) =>
    crypto.argon2Sync("argon2id", { message: Buffer.from(d.clave_admin), nonce: raw(sal), memory: d.argon2id.m_kib, passes: d.argon2id.t, parallelism: d.argon2id.p, tagLength: 32 });
  check("argon2id (prueba_admin)", b64(a2(d.sal_equipo)), d.prueba_admin);
  check("argon2id (cliente)", b64(a2(d.sal_cliente)), d.argon2id_cliente);
} else {
  console.log("--   argon2id: este Node no tiene crypto.argon2Sync (Node ≥ 24.7); se usan los valores del archivo");
}
check("verificador", b64(sha256(raw(d.prueba_admin))), d.verificador);
const hk = (info) => b64(Buffer.from(crypto.hkdfSync("sha256", raw(d.argon2id_cliente), Buffer.alloc(0), info, 32)));
check("k_cfg", hk("resguardo-kcfg-v1"), d.k_cfg);
check("k_exp", hk("resguardo-kexp-v1"), d.k_exp);
const e = d.etiqueta;
check("etiqueta", crypto.createHmac("sha256", raw(d.k_cfg)).update(`resguardo-etiqueta-v1|${e.equipo}|${e.box_pub}|${e.sign_pub}`).digest("base64"), e.valor);
const h2 = sha256(`resguardo-sas-v2|${d.sas_v2.identidad_servidor}|${d.sas_v2.box_pub}|${d.sas_v2.sign_pub}`);
const n2 = h2.readUInt32BE(0) % 1_000_000;
check("sas_v2", `${String(Math.floor(n2 / 1000)).padStart(3, "0")} ${String(n2 % 1000).padStart(3, "0")}`, d.sas_v2.sas);
const s3 = d.sas_v3;
const seis = (h) => {
  const n = h.readUInt32BE(0) % 1_000_000;
  return `${String(Math.floor(n / 1000)).padStart(3, "0")} ${String(n % 1000).padStart(3, "0")}`;
};
const norm = (h) => h.replace(/[^0-9a-fA-F]/g, "").toUpperCase();
check("sas_v3 (huella normalizada)", norm(s3.huella_ca), s3.huella_normalizada);
const v3 = (h) => seis(sha256(`resguardo-sas-v3|${s3.identidad_servidor}|${s3.box_pub}|${s3.sign_pub}|${norm(h)}`));
check("sas_v3", v3(s3.huella_ca), s3.sas);
check("sas_v3 (otra autoridad)", v3(s3.huella_otra), s3.sas_otra);
check("sas_v3 (sin huella)", v3(""), s3.sas_sin_huella);
const srv = pkcs8("70", raw(d.identidad_servidor.seed));
check("identidad del servidor (pública)", b64(rawPub(srv)), d.identidad_servidor.publica);
check("identidad del servidor (texto)", `resguardo-servidor-v1|${d.identidad_servidor.reto}|${d.identidad_servidor.equipo}`, d.identidad_servidor.texto);
check("identidad del servidor (firma)", b64(crypto.sign(null, Buffer.from(d.identidad_servidor.texto), srv)), d.identidad_servidor.firma);

// La clave de administración se normaliza a NFC antes de Argon2id.
const nfc = d.nfc;
check("nfc: normalización", nfc.clave_nfd.normalize("NFC"), nfc.clave_nfc);
check("nfc: la NFD es distinta", String(nfc.clave_nfd !== nfc.clave_nfc), "true");
if (typeof crypto.argon2Sync === "function") {
  const p = crypto.argon2Sync("argon2id", { message: Buffer.from(nfc.clave_nfd.normalize("NFC")), nonce: raw(nfc.sal_equipo), memory: d.argon2id.m_kib, passes: d.argon2id.t, parallelism: d.argon2id.p, tagLength: 32 });
  check("nfc: prueba_admin", b64(p), nfc.prueba_admin);
}

// XChaCha20-Poly1305 (sesiones, relé y configuración): HChaCha20 + ChaCha20-Poly1305 de Node.
const rotl = (x, n) => ((x << n) | (x >>> (32 - n))) >>> 0;
function hchacha20(key, nonce16) {
  const s = new Uint32Array(16);
  s.set([0x61707865, 0x3320646e, 0x79622d32, 0x6b206574]);
  for (let i = 0; i < 8; i++) s[4 + i] = key.readUInt32LE(i * 4);
  for (let i = 0; i < 4; i++) s[12 + i] = nonce16.readUInt32LE(i * 4);
  const qr = (a, b, c, d) => {
    s[a] = (s[a] + s[b]) >>> 0; s[d] = rotl(s[d] ^ s[a], 16);
    s[c] = (s[c] + s[d]) >>> 0; s[b] = rotl(s[b] ^ s[c], 12);
    s[a] = (s[a] + s[b]) >>> 0; s[d] = rotl(s[d] ^ s[a], 8);
    s[c] = (s[c] + s[d]) >>> 0; s[b] = rotl(s[b] ^ s[c], 7);
  };
  for (let r = 0; r < 10; r++) {
    qr(0, 4, 8, 12); qr(1, 5, 9, 13); qr(2, 6, 10, 14); qr(3, 7, 11, 15);
    qr(0, 5, 10, 15); qr(1, 6, 11, 12); qr(2, 7, 8, 13); qr(3, 4, 9, 14);
  }
  const out = Buffer.alloc(32);
  [0, 1, 2, 3, 12, 13, 14, 15].forEach((w, i) => out.writeUInt32LE(s[w], i * 4));
  return out;
}
function xchacha(key, nonce, datos, aad) {
  const sub = hchacha20(key, nonce.subarray(0, 16));
  const n12 = Buffer.concat([Buffer.alloc(4), nonce.subarray(16, 24)]);
  const c = crypto.createCipheriv("chacha20-poly1305", sub, n12, { authTagLength: 16 });
  c.setAAD(Buffer.from(aad));
  return Buffer.concat([nonce, c.update(datos), c.final(), c.getAuthTag()]);
}
const sm = v.simetrico;
const hkdfSal = (ikm, sal, info) => Buffer.from(crypto.hkdfSync("sha256", ikm, Buffer.from(sal), info, 32));
const se = sm.sesion;
check("sesión: k_consola", b64(hkdfSal(raw(se.clave_sesion), se.sesion_id, "resguardo-sesion-v1|consola")), se.k_consola);
check("sesión: k_equipo", b64(hkdfSal(raw(se.clave_sesion), se.sesion_id, "resguardo-sesion-v1|equipo")), se.k_equipo);
check("sesión: mensaje", b64(xchacha(raw(se.k_consola), raw(se.nonce), Buffer.from(se.json), `resguardo-sesion-v1|${se.sesion_id}`)), se.cifrado_consola);
const re = sm.relevo;
check("relé: trozo", b64(xchacha(raw(re.clave), raw(re.nonce), raw(re.datos), `${re.relevo_id}|${re.n}|${re.ultimo ? 1 : 0}`)), re.cifrado);
const cf = sm.config;
check("configuración", b64(xchacha(raw(cf.k_cfg), raw(cf.nonce), Buffer.from(cf.json), `resguardo-config-v1|${cf.equipo}|${cf.seq}`)), cf.cifrado);

// Paquete de exportación: cabecera + trozos con longitud, como el relé pero con su propio id.
const pq = v.paquete;
check("paquete: k_exp", pq.k_exp, d.k_exp);
const trozoPq = xchacha(raw(pq.k_exp), raw(pq.nonce), Buffer.from(pq.json), "resguardo-cliente-v1|0|1");
const len = Buffer.alloc(4);
len.writeUInt32BE(trozoPq.length);
check("paquete", b64(Buffer.concat([Buffer.from(`RESGUARDO-CLIENTE-1\n${pq.sal_cliente}\n`), len, trozoPq])), pq.paquete);

if (fails) process.exit(1);
console.log("Vectores correctos.");
