// Pruebas de «Instalar muchos equipos» (bloque 7 de la 0.7.26, src/lib/despliegue.ts): la línea
// de PowerShell (huella SHA-256 fijada, nada que la shell interprete), la orden codificada, la
// línea de Linux, el número de comprobación de cada equipo y que en bloque solo se confirman los
// que marca la persona. También el código del lote guardado en el navegador (lib/codigo.ts).
//
//   npm run test:vectores
import { comandoCodificado, comillasPs, aConfirmar, errorNombreLote, errorUsosDias, esperando, lineaLinuxVarios, lineaPowerShell, resumenLote, revisar, type EquipoDelLote } from "../src/lib/despliegue";
import { Codigos, PLAZO_GUARDADO_MS, type Almacen } from "../src/lib/codigo";
import { hashCodigo, sasV2, sasV3 } from "../src/lib/cripto/claves";

let total = 0;
let fallos = 0;
function igual(nombre: string, real: unknown, esperado: unknown) {
  total++;
  const ok = JSON.stringify(real) === JSON.stringify(esperado);
  if (!ok) {
    fallos++;
    console.error(`✗ ${nombre}\n   real:     ${JSON.stringify(real)}\n   esperado: ${JSON.stringify(esperado)}`);
  } else console.log(`✓ ${nombre}`);
}
const cierto = (nombre: string, v: boolean) => igual(nombre, v, true);

// --- La línea de PowerShell ---------------------------------------------------
console.log("\n— Línea de PowerShell (lib/despliegue.ts) —");
const SHA = "3f".repeat(32);
const LOTE = "0b6e1f9c-6d2a-4c1e-9a77-2f0c5d8e4b11";
const buena = { servidor: "https://192.0.2.10:8443/", lote: LOTE, sha256: SHA, codigo: "ABCD-EFGH-JKMN-PQRS" };
const linea = lineaPowerShell(buena);
igual(
  "La línea entera (una sola, para Windows PowerShell 5.1)",
  linea,
  [
    "$ErrorActionPreference='Stop'",
    "[Net.ServicePointManager]::SecurityProtocol=[Net.ServicePointManager]::SecurityProtocol -bor 3072",
    "$f=Join-Path $env:TEMP 'Resguardo-Agente-setup.exe'",
    `try{[Net.ServicePointManager]::ServerCertificateValidationCallback={$true};(New-Object Net.WebClient).DownloadFile('https://192.0.2.10:8443/api/agente/instalador/${LOTE}',$f)}finally{[Net.ServicePointManager]::ServerCertificateValidationCallback=$null}`,
    `if((Get-FileHash $f -Algorithm SHA256).Hash -ne '${SHA.toUpperCase()}'){Remove-Item $f;throw 'El instalador descargado no es el esperado: no se ha instalado nada.'}`,
    "$p=Start-Process $f -ArgumentList '/S /CODE=ABCD-EFGH-JKMN-PQRS /SERVIDOR=https://192.0.2.10:8443' -Wait -PassThru",
    "Remove-Item $f",
    "if($p.ExitCode -eq 0){Get-Content (Join-Path $env:ProgramFiles 'Resguardo Agente\\emparejamiento.txt')}else{Write-Host ('No se pudo vincular (salida '+$p.ExitCode+'). Mira el registro del agente.')}",
  ].join(";"),
);
cierto("Una sola línea (sin saltos)", !/[\r\n]/.test(linea));
cierto("La huella va fijada en la línea", linea.includes(`-ne '${SHA.toUpperCase()}'`));
cierto("Si la huella no coincide, borra y no ejecuta nada", /Hash -ne '[0-9A-F]{64}'\)\{Remove-Item \$f;throw/.test(linea));
cierto("Baja de ESTE servidor (nunca de otro sitio)", (linea.match(/https:\/\/[^'\s/]+/g) ?? []).every((u) => u === "https://192.0.2.10:8443"));
cierto("Deja la comprobación de certificados como estaba", linea.includes("finally{[Net.ServicePointManager]::ServerCertificateValidationCallback=$null}"));
igual("Mayúsculas en el id del lote: se normaliza", lineaPowerShell({ ...buena, lote: LOTE.toUpperCase() }), linea);
igual("La huella en minúsculas o mayúsculas: igual", lineaPowerShell({ ...buena, sha256: SHA.toUpperCase() }), linea);

// Nada que no tenga su forma: ni comillas, ni `;`, ni `$(…)`, ni saltos, ni http://.
for (const [que, d] of [
  ["dirección sin https", { ...buena, servidor: "http://192.0.2.10:8443" }],
  ["dirección con comilla", { ...buena, servidor: "https://srv'.ejemplo:8443" }],
  ["dirección con ;", { ...buena, servidor: "https://srv;calc:8443" }],
  ["dirección con $()", { ...buena, servidor: "https://$(calc):8443" }],
  ["dirección con espacio", { ...buena, servidor: "https://srv 1:8443" }],
  ["dirección con salto", { ...buena, servidor: "https://srv\n:8443" }],
  ["lote que no es un uuid", { ...buena, lote: "../ca" }],
  ["lote con comilla", { ...buena, lote: "0b6e1f9c-6d2a-4c1e-9a77-2f0c5d8e4b1'" }],
  ["huella corta", { ...buena, sha256: "3f".repeat(31) }],
  ["huella que no es hex", { ...buena, sha256: "zz".repeat(32) }],
  ["código con comilla", { ...buena, codigo: "ABCD'EFGH" }],
  ["código con espacio", { ...buena, codigo: "ABCD EFGH /S" }],
  ["código con ;", { ...buena, codigo: "ABCD;calc" }],
  ["código corto", { ...buena, codigo: "ABC" }],
] as const)
  igual(`No se compone con ${que}`, lineaPowerShell(d), "");

igual("Comillas simples de PowerShell: se doblan", comillasPs("a'b$c`d"), "'a''b$c`d'");

// La orden codificada: UTF-16LE en base64, que vuelve a dar la misma línea.
const cod = comandoCodificado(linea);
const b64 = cod.replace("powershell.exe -NoProfile -ExecutionPolicy Bypass -EncodedCommand ", "");
igual("Orden codificada: vuelve a dar la línea", Buffer.from(b64, "base64").toString("utf16le"), linea);
cierto("Orden codificada: sin comillas ni espacios en lo codificado", /^[A-Za-z0-9+/=]+$/.test(b64));
igual("Sin línea, sin orden", comandoCodificado(""), "");

// --- La línea de Linux ---------------------------------------------------------
console.log("\n— Línea de Linux —");
const HUELLA = Array(32).fill("5A").join(":");
igual(
  "La de siempre, con la huella y un espacio delante (fuera del historial de bash)",
  lineaLinuxVarios("ABCD-EFGH-JKMN-PQRS", "https://192.0.2.10:8443/", HUELLA),
  ` sudo resguardo-agente vincular ABCD-EFGH-JKMN-PQRS --servidor https://192.0.2.10:8443 --huella-ca ${HUELLA}`,
);
igual("Sin huella válida, nada", lineaLinuxVarios("ABCD-EFGH-JKMN-PQRS", "https://192.0.2.10:8443", ""), "");
igual("Código con $(), nada", lineaLinuxVarios("$(reboot)", "https://192.0.2.10:8443", HUELLA), "");

// --- Formulario ---------------------------------------------------------------
console.log("\n— Equipos, días y nombre —");
igual("10 equipos, 7 días", errorUsosDias(10, 7), null);
igual("0 equipos", errorUsosDias(0, 7), "Entre 1 y 100 equipos.");
igual("101 equipos", errorUsosDias(101, 7), "Entre 1 y 100 equipos.");
igual("2,5 equipos", errorUsosDias(2.5, 7), "Entre 1 y 100 equipos.");
igual("31 días", errorUsosDias(10, 31), "Entre 1 y 30 días.");
igual("Nombre vacío: vale", errorNombreLote("  "), null);
igual("Nombre con comillas", errorNombreLote('a"b'), "Sin comillas ni saltos de línea.");
igual("Nombre largo", errorNombreLote("x".repeat(61)), "Hasta 60 caracteres.");
igual("Resumen activo", resumenLote({ estado: "activo", quedan: 7, usos: 10 }), "Quedan 7 de 10 equipos");
igual("Resumen anulado", resumenLote({ estado: "anulado", quedan: 7, usos: 10 }), "Anulado (ya no admite equipos)");

// --- Los que esperan: el número lo calcula la consola, y solo los marcados ------
console.log("\n— Equipos esperando confirmación —");
const identidad = Buffer.from(new Uint8Array(32).fill(7)).toString("base64");
const llaves = (n: number) => ({ box_pub: Buffer.from(new Uint8Array(32).fill(n)).toString("base64"), sign_pub: Buffer.from(new Uint8Array(32).fill(n + 1)).toString("base64") });
const equipo = (id: string, n: number, sas: (k: ReturnType<typeof llaves>) => string, extra: Partial<EquipoDelLote> = {}): EquipoDelLote => {
  const k = llaves(n);
  return {
    id,
    estado: "unido",
    caduca: "2026-10-14T10:00:00Z",
    unido: "2026-10-07T10:00:00Z",
    ip: "192.0.2.50",
    equipo: { id: `eq-${id}`, nombre: `PUESTO-${n}`, so: "windows", sal_equipo: "AAAA", ...k },
    sas: sas(k),
    sas_version: 3,
    ...extra,
  };
};
const bien = equipo("a", 1, (k) => sasV3(identidad, k.box_pub, k.sign_pub, HUELLA));
const otroBien = equipo("b", 3, (k) => sasV3(identidad, k.box_pub, k.sign_pub, HUELLA));
const enMedio = equipo("c", 5, () => "000 000");
const antiguo = equipo("d", 7, (k) => sasV2(identidad, k.box_pub, k.sign_pub), { sas_version: undefined });
const hecho = equipo("e", 9, (k) => sasV3(identidad, k.box_pub, k.sign_pub, HUELLA), { estado: "dado_de_alta" });
const revs = esperando([bien, otroBien, enMedio, antiguo, hecho]).map((e) => revisar(e, identidad, HUELLA));
igual("Esperan los unidos (no los ya dados de alta)", revs.map((r) => r.e.id), ["a", "b", "c", "d"]);
igual("Coincide el número calculado aquí", revs.map((r) => r.coincide), [true, true, false, true]);
igual("Agente anterior: SAS v2 y aviso de la huella", [revs[3].antiguo, revs[3].sas === antiguo.sas], [true, true]);
cierto("El número tiene la forma «NNN NNN»", revs.every((r) => /^\d{3} \d{3}$/.test(r.sas ?? "")));
igual("Sin marcar ninguno, no se confirma ninguno", aConfirmar(revs, new Set()).length, 0);
igual("Solo los marcados", aConfirmar(revs, new Set(["b"])).map((r) => r.e.id), ["b"]);
igual("Uno marcado cuyo número no coincide no entra", aConfirmar(revs, new Set(["a", "c"])).map((r) => r.e.id), ["a"]);
igual("Uno ya dado de alta no se vuelve a confirmar", aConfirmar(esperando([hecho]).map((e) => revisar(e, identidad, HUELLA)), new Set(["e"])).length, 0);

// --- El código del lote en el navegador ----------------------------------------
console.log("\n— Código del lote guardado en el navegador (lib/codigo.ts) —");
const mapa = new Map<string, string>();
const almacen: Almacen = { getItem: (k) => mapa.get(k) ?? null, setItem: (k, v) => void mapa.set(k, v), removeItem: (k) => void mapa.delete(k) };
let ahora = Date.parse("2026-10-07T10:00:00Z");
const cods = new Codigos(almacen, () => ahora);
const codigoLote = "ABCD-EFGH-JKMN-PQRS";
cods.guardar({ id: LOTE, cliente: "c1", codigo: codigoLote, lote: true, hasta: ahora + 32 * 86_400_000 });
cods.guardar({ id: "corto", cliente: "c1", codigo: "AAAA-BBBB-CC" });
igual("Por su hash (para el alta de cada equipo que se unió con él)", cods.porHash("c1", hashCodigo(codigoLote).toUpperCase()), codigoLote);
igual("De otro cliente, no", cods.porHash("c2", hashCodigo(codigoLote)), null);
ahora += PLAZO_GUARDADO_MS + 1000;
igual("Pasados 8 días: el de 15 min se olvida, el del lote sigue", [cods.de("corto"), cods.de(LOTE, hashCodigo(codigoLote))], [null, codigoLote]);
ahora += 30 * 86_400_000;
igual("Pasado su plazo, también se olvida", cods.de(LOTE), null);
cods.guardar({ id: "largo", cliente: "c1", codigo: codigoLote, lote: true, hasta: ahora + 90 * 86_400_000 });
ahora += PLAZO_GUARDADO_MS + 1000;
igual("Nunca más de 32 días aunque pida más", cods.de("largo"), null);

console.log(`\n${total - fallos}/${total} bien`);
if (fallos) process.exit(1);
