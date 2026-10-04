// Datos simulados para `npm run dev:ventana` (solo en desarrollo: no entra en
// la página que lleva el agente). Una copia en marcha con ritmos que suben y
// bajan, y un servicio de mentira para los ajustes y el modo local.
import type { Datos, EstadoBandeja, Punto } from "./tipos";

const inicio = Date.now();
const serie: Punto[] = [];
let modo: string = new URLSearchParams(location.search).get("modo") ?? "local";
const config = {
  v: 1,
  copias: [
    {
      id: "docs",
      nombre: "Documentos",
      repo: "principal",
      carpetas: ["C:\\Users\\Ana\\Documents"],
      exclusiones: ["*.tmp"],
      horario: { dias: [1, 2, 3, 4, 5], horas: ["13:00", "19:00"] },
      activa: true,
      solo_si_cambios: true,
    },
  ],
  escritorio: { ventana: "al_trabajar", avisos: "todo" },
};

function ola(t: number, base: number, f: number, fase = 0) {
  return Math.max(0, base * (0.55 + 0.3 * Math.sin(t / f + fase) + 0.15 * Math.sin(t / (f / 3.1) + fase * 2)));
}

function bandeja(ahora: number): EstadoBandeja {
  const s = (ahora - inicio) / 1000;
  const copiando = new URLSearchParams(location.search).get("quieto") === null;
  const pct = Math.min(0.97, 0.18 + s / 900);
  return {
    text: copiando ? `Copiando «Documentos»… ${Math.floor(pct * 100)} %` : "Tus archivos están protegidos.",
    privacy: modo === "local" ? "Este equipo se administra en el propio equipo, con su clave de administración." : "",
    show: true,
    vinculado: modo !== "sin_clave",
    copias: [
      { clave: "principal#docs", nombre: "Documentos", resultado: "ok", cuando: new Date(ahora - 3 * 3600_000).toISOString(), proxima: new Date(ahora + 2 * 3600_000).toISOString() },
      { clave: "principal#fotos", nombre: "Fotos de la familia", resultado: "warning", cuando: new Date(ahora - 26 * 3600_000).toISOString(), proxima: new Date(ahora + 20 * 3600_000).toISOString() },
      { clave: "principal#conta", nombre: "Contabilidad (Siigo)", resultado: "ok", cuando: new Date(ahora - 40 * 60_000).toISOString(), proxima: new Date(ahora + 20 * 60_000).toISOString() },
    ],
    pedir: true,
    escrito: new Date(ahora).toISOString(),
    escritorio: { ventana: "al_trabajar", avisos: "todo" },
    actividades: copiando
      ? [
          {
            id: "copia:principal#docs@1",
            clave: "copia:principal#docs",
            tipo: "copia",
            nombre: "Documentos",
            fase: "subiendo",
            porcentaje: pct,
            archivos: Math.floor(pct * 48_210),
            archivos_total: 48_210,
            bytes: Math.floor(pct * 61_400_000_000),
            bytes_total: 61_400_000_000,
            velocidad: Math.round(ola(s, 95_000_000, 9)),
            lectura: Math.round(ola(s, 110_000_000, 9)),
            subida: Math.round(ola(s, 38_000_000, 13, 1.3)),
            archivos_s: Math.round(ola(s, 240, 7, 0.6)),
            quedan_s: Math.round((1 - pct) * 900),
            empezo: new Date(inicio - 120_000).toISOString(),
          },
        ]
      : [],
    hechas: [
      { clave: "copia:principal#docs", tipo: "copia", nombre: "Documentos", resultado: "ok", cuando: new Date(ahora - 3 * 3600_000).toISOString(), bytes: 412_000_000 },
      { clave: "verify:principal", tipo: "verificacion", nombre: "Disco USB", resultado: "ok", cuando: new Date(ahora - 30 * 3600_000).toISOString() },
      { clave: "offsite:principal", tipo: "copia_externa", nombre: "Disco USB", resultado: "error", cuando: new Date(ahora - 50 * 3600_000).toISOString() },
    ],
    local: modo === "local",
  };
}

function datos(): Datos {
  const ahora = Date.now();
  const b = bandeja(ahora);
  const a = b.actividades[0];
  // Cinco minutos de historia al empezar, para que se vea la onda entera.
  if (!serie.length && a) {
    for (let t = Math.floor(ahora / 1000) - 300; t < Math.floor(ahora / 1000); t += 2) {
      const s = (t * 1000 - inicio) / 1000;
      serie.push([t, Math.round(ola(s, 110_000_000, 9)), Math.round(ola(s, 38_000_000, 13, 1.3)), Math.round(ola(s, 240, 7, 0.6)), "copia"]);
    }
  }
  if (a) serie.push([Math.floor(ahora / 1000), a.lectura ?? 0, a.subida ?? 0, a.archivos_s ?? 0, "copia"]);
  while (serie.length > 150) serie.shift();
  const hoy = new Date();
  const historial = Array.from({ length: 14 }, (_, i) => {
    const d = new Date(hoy.getTime() - (13 - i) * 86_400_000);
    return { dia: d.toISOString().slice(0, 10), ok: 2 + ((i * 7) % 4), aviso: i % 5 === 2 ? 1 : 0, fallo: i === 9 ? 1 : 0 };
  });
  return { bandeja: b, ventana: { v: 1, escrito: new Date(ahora).toISOString(), serie: [...serie], historial }, ahora: new Date(ahora).toISOString() };
}

export function empezar() {
  const enviar = () => window.__resguardo?.datos(datos());
  enviar();
  setInterval(enviar, 2000);
}

const espera = (ms: number) => new Promise((r) => setTimeout(r, ms));

export async function simular(op: string, c: Record<string, unknown>): Promise<unknown> {
  await espera(250);
  switch (op) {
    case "hola":
      return { v: 1, modo, reto: "x", sal_equipo: "AAAAAAAAAAAAAAAAAAAAAA==" };
    case "desbloquear":
      if (c.clave !== "caballo bateria grapa") throw new Error("La clave de administración no es correcta.");
      return { modo };
    case "crear_clave":
      modo = "local";
      return { modo };
    case "copiar":
      return { mensaje: "Copia pedida: empieza en unos segundos." };
    case "bloquear":
      return null;
    case "servicio": {
      const que = c.que as string;
      const cuerpo = (c.cuerpo ?? {}) as Record<string, unknown>;
      if (que === "estado_local")
        return {
          repositorios: [{ id: "principal", nombre: "Disco USB", destino: "usb", retencion: { diarias: 0, semanales: 0, mensuales: -1, anuales: 0, plazos: { horarias: "15d", diarias: "1y" } }, externa: null }],
          destinos: [{ id: "usb", nombre: "Disco USB", tipo: "local", donde: "E:\\Copias" }],
          config,
          nubes: [],
          resumen: { guarda_copias: null },
          pausado_hasta: null,
          nombre_equipo: "PORTATIL-ANA",
        };
      if (que === "config") {
        Object.assign(config, cuerpo.config as object);
        return { mensaje: "Configuración aplicada: 1 copia activa en 1 repositorio." };
      }
      if (que === "carpetas")
        return {
          entradas: (cuerpo.p as { ruta?: string })?.ruta
            ? [
                { nombre: "Documents", tipo: "dir" },
                { nombre: "Pictures", tipo: "dir" },
                { nombre: "Desktop", tipo: "dir" },
              ]
            : [
                { nombre: "C:\\", tipo: "dir" },
                { nombre: "E:\\", tipo: "dir" },
              ],
          sugerencias: [{ id: "docs", nombre: "Documentos de Ana", rutas: ["C:\\Users\\Ana\\Documents"] }],
        };
      if (que === "explorar") {
        if (cuerpo.que === "versiones")
          return {
            versiones: Array.from({ length: 6 }, (_, i) => ({ id: `a1b2c3d${i}`, cuando: new Date(Date.now() - i * 86_400_000).toISOString(), etiquetas: ["Documentos"], archivos: 48_000 + i, bytes: 61_000_000_000 })),
          };
        if (cuerpo.que === "listar")
          return {
            entradas: [
              { nombre: "Facturas", tipo: "dir" },
              { nombre: "Presupuesto 2026.xlsx", tipo: "archivo", bytes: 182_330, modificado: new Date().toISOString() },
              { nombre: "Contrato.pdf", tipo: "archivo", bytes: 1_020_442, modificado: new Date().toISOString() },
            ],
          };
        if (cuerpo.que === "diferencias")
          return { resumen: { nuevos: 12, cambiados: 3, borrados: 1 }, cambios: [{ ruta: "/C/Users/Ana/Documents/Presupuesto 2026.xlsx", tipo: "cambiado" }], siguiente: null };
      }
      if (que === "historial")
        return Array.from({ length: 12 }, (_, i) => ({
          tipo: i % 4 === 3 ? "verify" : "backup",
          repo: "Disco USB",
          copia: "Documentos",
          cuando: new Date(Date.now() - i * 5 * 3600_000).toISOString(),
          resultado: i === 5 ? "error" : i === 2 ? "warning" : "ok",
          mensaje: i === 5 ? "No se pudo abrir el repositorio: el disco no está conectado." : "Copia correcta.",
          bytes: 120_000_000 + i * 3_000_000,
        }));
      if (que === "kit") return [{ id: "principal", nombre: "Disco USB", destino: "Disco USB", tipo: "local", ubicacion: "E:\\Copias\\principal", id_restic: "8f1c0d2e9a" }];
      return { mensaje: "Hecho (simulado)." };
    }
    default:
      return null;
  }
}
