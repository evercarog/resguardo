// «Agente» simulado: abre de verdad los sobres de las órdenes con la llave
// X25519 del equipo, comprueba la prueba de administración (contra el
// verificador) o la contraseña del repositorio, firma los resultados con su
// Ed25519 y contesta en las sesiones cifradas. Así la consola se prueba de
// extremo a extremo sin servidor ni agente reales.
import { randomUUID } from "node:crypto";
import { ed25519 } from "@noble/curves/ed25519.js";
import { sha256 } from "@noble/hashes/sha2.js";
import { aB64, deB64, deUtf8, iguales, utf8 } from "../lib/cripto/bytes";
import { abrir, sellar } from "../lib/cripto/sobre";
import { etiquetaEquipo, mensajeResultado, pruebaCodigo } from "../lib/cripto/claves";
import { enCarpetaDelSistema, errorCarpetaEspejo, errorCarpetaLocal, errorGancho, errorNombreCarpeta, ganchosDe, MAX_GANCHOS, VERSION_GANCHOS, versionAlMenos } from "../lib/ganchos";
import { claveEspejo, destinosDeCuerpo, esDestructiva, NIVEL, PIDE_TAMBIEN_ADMIN, type DestinoEspejo, type OrdenPlana } from "../lib/cripto/ordenes";
import { claveDireccion, cifrarConfig, cifrarMensaje, cifrarTrozo, descifrarMensaje, TROZO } from "../lib/cripto/simetrico";
import type * as T from "../lib/tipos";
import { errorHorario, errorRegla, textoHorario, textoRegla } from "../lib/retencion";
import { errorReglas, horaValida, proximaVez, reglasDe, VERSION_REGLAS } from "../lib/horario";
import { errorCadenas } from "../lib/cadenas";
import { auditar, estado, type EquipoMock, type OrdenMock, type SesionMock } from "./estado";
import { zipSinComprimir } from "./zip";
import { empezarCopia, empezarHistorial, empezarTarea } from "./progreso";
import { operarDetalle, OPS_DETALLE, VERSION_DETALLE } from "./detalle";
import { buscarTodas, OP_BUSCAR, VERSION_BUSCAR } from "./buscar";
import { observacionDelEquipo } from "./notas";

const espera = (ms: number) => new Promise((r) => setTimeout(r, ms));
/** Claves del almacén que algún equipo dueño ya añadió a su repositorio (clave_almacen). */
const clavesAlmacen = new Set<string>();
const ahora = () => new Date().toISOString();

/** Avisa a quien espera mensajes de una sesión (espera larga). */
export const esperando = new Map<string, Set<() => void>>();
export function despertar(sesion: string) {
  for (const f of esperando.get(sesion) ?? []) f();
  esperando.delete(sesion);
}

function resultado(e: EquipoMock, o: OrdenMock, estadoO: T.EstadoOrden, mensaje: string | null, detalle: string | null = null) {
  o.estado = estadoO;
  o.mensaje = mensaje;
  o.detalle = detalle;
  o.actualizada = ahora();
  o.firma_agente = aB64(ed25519.sign(utf8(mensajeResultado(o)), e.secretaFirma));
}

// ---------------------------------------------------------------------------
// Configuración de ejemplo (rutas solo dentro del cifrado)
// ---------------------------------------------------------------------------

const CARPETAS: Record<string, string[]> = {
  documentos: ["C:\\Users\\recepcion\\Documents", "C:\\Users\\recepcion\\Desktop", "D:\\Escaneos"],
  siigo: ["C:\\SIIWI01", "C:\\Users\\contador\\Documents"],
  gerencia: ["C:\\Users\\gerente\\Desktop", "C:\\Users\\gerente\\Documents"],
  caja: ["C:\\Facturas"],
  proyectos: ["E:\\Proyectos", "E:\\Audio", "F:\\Entregas", "C:\\Users\\estudio\\Documents"],
  "subida-nube": ["/srv/resguardo/repos"],
  compartidas: ["E:\\Compartido\\Administracion", "E:\\Compartido\\Ventas", "E:\\Compartido\\Planos"],
};

export function configInicial(e: EquipoMock): T.Configuracion {
  const r = e.resumen ?? {};
  return {
    v: 1,
    copias: (r.copias ?? []).map((c) => ({
      id: c.id,
      nombre: c.nombre,
      repo: c.repo,
      carpetas: CARPETAS[c.id] ?? ["C:\\Users\\Public\\Documents"],
      exclusiones: ["*.tmp", "~$*", "Thumbs.db"],
      horario: typeof c.horario === "object" && c.horario ? c.horario : { dias: [1, 2, 3, 4, 5], horas: ["13:00"] },
      activa: c.activa !== false,
      ...(c.tras ? { tras: c.tras } : {}),
      // CONTABILIDAD (agente 0.7.2) vuelca su base de SQL Server antes de copiar «Siigo».
      gancho: c.id === "siigo" && versionAlMenos(e.version_agente, VERSION_GANCHOS) ? [{ tipo: "sqlserver" as const, bases: ["SIIGO_ALTAMAR"], carpeta: "C:\\ResguardoVolcados" }] : null,
    })),
    repositorios: (r.repositorios ?? []).map((x) => ({
      id: x.id,
      nombre: x.nombre,
      destino: x.destino,
      retencion: x.retencion_regla ?? { diarias: 7, semanales: 4, mensuales: 12, anuales: 2 },
      ...(x.solo_lectura ? { solo_lectura: true } : {}),
      // En la configuración, la copia externa lleva el id del destino (en el resumen, su nombre).
      ...(x.externa ? { externa: { destino: (r.destinos ?? []).find((d) => d.nombre === x.externa!.destino)?.id ?? x.externa.destino, hora: x.externa.hora } } : {}),
    })),
    destinos: r.destinos ?? [],
    verificacion: { cada_dias: 7, porcentaje: 5 },
    bandeja: { visible: true, avisos: true },
    // v1.28: la verificación automática que ya tiene (solo un agente que la entiende).
    ...(r.admite?.includes("verificacion_auto")
      ? { verificaciones: Object.fromEntries((r.repositorios ?? []).filter((x) => x.verificacion_auto).map((x) => [x.id, { cada_dias: x.verificacion_auto!.cada_dias, porcentaje: x.verificacion_auto!.porcentaje, ...(x.verificacion_auto!.horario ? { horario: x.verificacion_auto!.horario } : {}) }])) }
      : {}),
    // Tarea 8: la prueba de restauración automática que ya tiene (solo un agente que la entiende).
    ...(r.admite?.includes("prueba_auto") ? { pruebas_restauracion: Object.fromEntries((r.repositorios ?? []).filter((x) => x.prueba_auto).map((x) => [x.id, { cada_dias: x.prueba_auto!.cada_dias }])) } : {}),
  };
}

/** Las 03:00 de dentro de `dias` días (la próxima verificación automática, como el agente). */
function a3(dias: number) {
  const d = new Date();
  d.setDate(d.getDate() + dias);
  d.setHours(3, 0, 0, 0);
  return d.toISOString();
}

export function guardarConfig(e: EquipoMock, cfg: T.Configuracion, seq: number) {
  if (!e.kcfg) return;
  e.config = { seq, cifrado: aB64(cifrarConfig(e.kcfg, e.id, seq, cfg)), resumen: e.resumen };
}

/** La parte en claro que se deriva de la configuración (sin rutas). */
function resumenDe(e: EquipoMock, cfg: T.Configuracion): T.ResumenEquipo {
  const previo = e.resumen ?? {};
  return {
    ...previo,
    destinos: cfg.destinos,
    repositorios: cfg.repositorios.map((r) => ({
      ...(previo.repositorios?.find((p) => p.id === r.id) ?? { versiones: 0, bytes: 0 }),
      id: r.id,
      nombre: r.nombre,
      destino: r.destino,
      retencion: r.retencion ? textoRegla(r.retencion) : null,
      // v1.28: la regla tal cual y la verificación automática (como el agente que la entiende).
      ...(previo.admite?.includes("retencion_plazos") ? { retencion_regla: r.retencion ?? null } : {}),
      ...(previo.admite?.includes("verificacion_auto")
        ? (() => {
            const v0 = cfg.verificaciones?.[r.id];
            // v1.40: el horario, solo si el agente lo entiende (uno anterior lo ignora).
            const v = v0 && !previo.admite?.includes("verificacion_horario") ? { cada_dias: v0.cada_dias, porcentaje: v0.porcentaje } : v0;
            const antes = previo.repositorios?.find((p) => p.id === r.id)?.verificacion_auto;
            const igual = antes && v && antes.cada_dias === v.cada_dias && antes.porcentaje === v.porcentaje && JSON.stringify(antes.horario ?? null) === JSON.stringify(v.horario ?? null);
            const proxima = v?.horario ? new Date(proximaVez(reglasDe(v.horario), Date.now()) ?? Date.now()).toISOString() : a3(1);
            return { verificacion_auto: v ? (igual ? antes : { ...v, proxima, todo_leido: null }) : null };
          })()
        : {}),
      // Tarea 8: la prueba de restauración automática (la primera, a las 04:00 siguientes).
      ...(previo.admite?.includes("prueba_auto") ? { prueba_auto: cfg.pruebas_restauracion?.[r.id] ? { cada_dias: cfg.pruebas_restauracion[r.id].cada_dias, proxima: a3(1) } : null } : {}),
    })),
    copias: cfg.copias.map((c) => ({
      ...(previo.copias?.find((p) => p.id === c.id) ?? {}),
      id: c.id,
      nombre: c.nombre,
      repo: c.repo,
      horario: c.horario,
      carpetas: c.carpetas.length,
      activa: c.activa,
      tras: c.tras ?? null,
      // v1.16: como el agente 0.7.7 (los anteriores ni lo leen: siempre encendido).
      ...(versionAlMenos(e.version_agente, "0.7.7") ? { solo_si_cambios: c.solo_si_cambios !== false } : {}),
    })),
  };
}

// ---------------------------------------------------------------------------
// Órdenes
// ---------------------------------------------------------------------------

/** Procesa una orden recién llegada, como haría el agente por el WebSocket. */
export async function procesarOrden(o: OrdenMock) {
  const e = estado.equipos.find((x) => x.id === o.equipo);
  if (!e) return;
  if (!e.conectado) return; // se quedará «pendiente» hasta que vuelva
  await espera(700);
  if (o.estado === "cancelada") return;
  o.estado = "entregada";
  o.actualizada = ahora();

  let plana: OrdenPlana;
  try {
    plana = JSON.parse(deUtf8(abrir(e.secretaBox, deB64(o.sellado))));
  } catch {
    return resultado(e, o, "rechazada", "No se pudo abrir la orden: no estaba sellada para este equipo.");
  }
  // Lo de fuera tiene que coincidir con lo de dentro.
  if (plana.v !== 2 || plana.equipo !== e.id || plana.cliente !== o.cliente || plana.seq !== o.seq || plana.tipo !== o.tipo || plana.caduca !== o.caduca || plana.not_before !== o.not_before) {
    return resultado(e, o, "rechazada", "La orden no coincide con sus datos en claro.");
  }
  if (plana.seq <= e.ultimoSeqAceptado) return resultado(e, o, "rechazada", "Orden repetida (seq ya usado).");

  // Autorización según el nivel.
  const nivel = NIVEL[plana.tipo];
  if (!nivel) return resultado(e, o, "rechazada", "Tipo de orden desconocido.");
  const pruebaOk = () => {
    const p = plana.autorizacion?.prueba_admin;
    return !!p && !!e.verificador && iguales(sha256(deB64(p)), e.verificador);
  };
  const repoOk = () => {
    const c = plana.autorizacion?.clave_repo;
    return !!c && e.contrasenas[c.repo] !== undefined && e.contrasenas[c.repo] === c.contrasena;
  };
  let autorizada = true;
  let motivo = "";
  if (plana.tipo === "alta") {
    autorizada = !e.verificador; // solo la primera vez
    motivo = "Este equipo ya estaba dado de alta.";
  } else if (nivel === "admin" || PIDE_TAMBIEN_ADMIN.has(plana.tipo)) {
    if (!pruebaOk()) {
      autorizada = false;
      motivo = "La clave de administración no es correcta.";
    }
  }
  if (autorizada && nivel === "repo" && !repoOk()) {
    autorizada = false;
    motivo = "La contraseña del repositorio no es correcta.";
  }
  if (!autorizada) {
    e.intentosFallidos++;
    if (e.intentosFallidos >= 3) {
      estado.avisos.push({
        id: randomUUID(),
        cliente: o.cliente,
        equipo: e.id,
        tipo: "intentos_fallidos",
        mensaje: `Hubo ${e.intentosFallidos} intentos fallidos en ${e.nombre}.`,
        creado: ahora(),
        visto_por: null,
        abierto: true,
      });
    }
    auditar(o.cliente, null, "orden.rechazada", e.nombre, { seq: o.seq, tipo: o.tipo });
    return resultado(e, o, "rechazada", motivo);
  }
  e.intentosFallidos = 0;
  e.ultimoSeqAceptado = plana.seq;

  // Lo destructivo espera a su not_before (el servidor lo tiene «pendiente»).
  if (plana.not_before && Date.parse(plana.not_before) > Date.now()) {
    o.estado = "pendiente";
    o.actualizada = ahora();
    return;
  }
  await ejecutar(e, o, plana);
}

/** Detalle cifrado para la efímera de la consola (si la mandó). */
const detallePara = (plana: OrdenPlana, datos: unknown) => (plana.responder_a ? aB64(sellar(deB64(plana.responder_a), utf8(JSON.stringify(datos)))) : null);

async function ejecutar(e: EquipoMock, o: OrdenMock, plana: OrdenPlana) {
  const c = plana.cuerpo as Record<string, unknown>;
  o.estado = "en_marcha";
  o.actualizada = ahora();
  switch (plana.tipo) {
    case "alta": {
      // Como el agente: la prueba tiene que corresponder al verificador, y la
      // prueba del código, a quien vio el código de emparejamiento.
      const ver = String(c.verificador);
      const prueba = plana.autorizacion.prueba_admin;
      if (!prueba || !iguales(sha256(deB64(prueba)), deB64(ver))) return resultado(e, o, "rechazada", "La prueba de administración no corresponde al verificador.");
      // v1.48: con el código generado en el navegador, este «equipo» simulado solo conoce su hash
      // (el de verdad lo tiene: se lo escribieron o venía en el instalador): solo mira que haya prueba.
      const sinCodigo = e.codigoEmparejamiento === "" && /^[A-Za-z0-9+/]{43}=$/.test(String(plana.autorizacion.prueba_codigo ?? ""));
      if (!sinCodigo && (!e.codigoEmparejamiento || plana.autorizacion.prueba_codigo !== pruebaCodigo(e.codigoEmparejamiento, e.id, ver)))
        return resultado(e, o, "rechazada", "El alta no viene de quien tiene el código de emparejamiento.");
      e.codigoEmparejamiento = null;
      e.verificador = deB64(ver);
      e.kcfg = deB64(String(c.k_cfg));
      e.confirmado = true;
      guardarConfig(e, configInicial(e), plana.seq);
      return resultado(e, o, "hecha", "Equipo dado de alta: ya obedece a la clave de administración del cliente.");
    }
    case "cambiar_clave_admin": {
      // Como el agente: verificador y K_cfg nuevos; la etiqueta, con la K_cfg nueva (la sube con su configuración).
      const ver = deB64(String(c.verificador ?? ""));
      const kcfg = deB64(String(c.k_cfg ?? ""));
      if (ver.length !== 32 || kcfg.length !== 32) return resultado(e, o, "fallida", "Verificador o K_cfg no válidos.");
      e.verificador = ver;
      e.kcfg = kcfg;
      e.etiqueta = etiquetaEquipo(kcfg, e.id, e.box_pub, e.sign_pub);
      return resultado(e, o, "hecha", "Clave de administración cambiada.");
    }
    case "config": {
      const cfg = c.config as T.Configuracion;
      if (cfg?.v !== 1 || !Array.isArray(cfg.copias)) return resultado(e, o, "fallida", "Configuración no válida.");
      // Ganchos de plantilla (v1.10): como el agente, solo desde la 0.7.2 y sin un campo de más.
      const conGancho = cfg.copias.filter((k) => ganchosDe(k.gancho).length);
      if (conGancho.length && !versionAlMenos(e.version_agente, VERSION_GANCHOS)) return resultado(e, o, "fallida", "Los ganchos aún no los admite este agente: actualízalo.");
      for (const k of conGancho) {
        const gs = ganchosDe(k.gancho);
        if (gs.length > MAX_GANCHOS) return resultado(e, o, "fallida", `«${k.nombre}»: como mucho ${MAX_GANCHOS} ganchos.`);
        for (const g of gs) {
          const permitidos = g.tipo === "sqlserver" ? ["tipo", "instancia", "bases", "carpeta"] : g.tipo === "carpeta_reciente" ? ["tipo", "carpeta", "horas", "extension"] : null;
          if (!permitidos || Object.keys(g).some((x) => !permitidos.includes(x))) return resultado(e, o, "fallida", "Configuración no válida: gancho desconocido o con campos de más.");
          const err = errorGancho(g);
          if (err) return resultado(e, o, "fallida", err);
        }
      }
      // Tarea 7c: «después de la anterior», como el agente con `admite: "cadenas"` (uno anterior lo ignora).
      if (!e.resumen?.admite?.includes("cadenas")) for (const k of cfg.copias) delete k.tras;
      const errCadena = errorCadenas(cfg.copias);
      if (errCadena) return resultado(e, o, "fallida", errCadena);
      // Horario (v1.24): un agente ≥ 0.7.9 usa las reglas si las hay; uno anterior no las lee y usa la lista de horas.
      for (const k of cfg.copias) {
        if (k.tras && !k.horario.reglas?.length && !k.horario.horas.length) continue;
        if (k.horario.reglas?.length && !versionAlMenos(e.version_agente, VERSION_REGLAS)) delete k.horario.reglas;
        if (k.horario.reglas?.length) {
          const err = errorReglas(k.horario.reglas);
          if (err) return resultado(e, o, "fallida", `Copia «${k.nombre}»: ${err}`);
        } else if (!k.horario.dias.length || !k.horario.horas.length || k.horario.horas.some((h) => !horaValida(h))) {
          return resultado(e, o, "fallida", `Copia «${k.nombre}»: el horario necesita días y horas válidas (HH:MM).`);
        }
      }
      const conocidos = new Set((e.resumen?.repositorios ?? []).map((x) => x.id));
      const sinRepo = cfg.copias.find((k) => !conocidos.has(k.repo));
      if (sinRepo) return resultado(e, o, "fallida", `La copia «${sinRepo.nombre}» usa un repositorio que este equipo no tiene.`);
      // Los repositorios y destinos los escribe el equipo (de crear_repositorio): los de la consola se ignoran.
      const previa = configInicial(e);
      cfg.repositorios = previa.repositorios;
      cfg.destinos = previa.destinos;
      // v1.28: un agente anterior no lee `verificaciones` (y no las guarda).
      if (!e.resumen?.admite?.includes("verificacion_auto")) delete cfg.verificaciones;
      else if (!cfg.verificaciones) cfg.verificaciones = previa.verificaciones;
      // Tarea 8: igual con la prueba de restauración automática.
      if (!e.resumen?.admite?.includes("prueba_auto")) delete cfg.pruebas_restauracion;
      else if (!cfg.pruebas_restauracion) cfg.pruebas_restauracion = previa.pruebas_restauracion;
      for (const [r, p] of Object.entries(cfg.pruebas_restauracion ?? {})) {
        if (!conocidos.has(r)) return resultado(e, o, "fallida", `La prueba de restauración automática es de un repositorio que este equipo no tiene: «${r}».`);
        if (!(p.cada_dias >= 1 && p.cada_dias <= 31)) return resultado(e, o, "fallida", "La prueba de restauración automática va de cada día a cada 31 días.");
      }
      for (const [r, v] of Object.entries(cfg.verificaciones ?? {})) {
        if (!conocidos.has(r)) return resultado(e, o, "fallida", `La verificación automática es de un repositorio que este equipo no tiene: «${r}».`);
        const conReglas = v.horario && reglasDe(v.horario).length && e.resumen?.admite?.includes("verificacion_horario");
        if (conReglas && errorReglas(reglasDe(v.horario!))) return resultado(e, o, "fallida", `Horario de la verificación: ${errorReglas(reglasDe(v.horario!))}`);
        if ((!conReglas && !(v.cada_dias >= 1 && v.cada_dias <= 31)) || !(v.porcentaje >= 0 && v.porcentaje <= 100)) return resultado(e, o, "fallida", "La verificación automática va de cada día a cada 31 días.");
      }
      e.resumen = resumenDe(e, cfg);
      guardarConfig(e, cfg, plana.seq);
      const n = cfg.copias.reduce((s, x) => s + x.carpetas.length, 0);
      return resultado(e, o, "hecha", `Configuración aplicada: ${cfg.copias.length} copias, ${n} carpetas.`, detallePara(plana, { carpetas: n }));
    }
    case "copiar_ahora": {
      // Como el agente: la orden solo la pide; la copia se ve avanzar en «progreso» (v1.25)
      // y cuenta como hecha al terminar.
      await espera(1200);
      const copia = e.resumen?.copias?.find((x) => x.id === c.copia);
      if (copia)
        empezarCopia(e.cliente, e.id, copia.repo, copia.id, copia.nombre, !!e.informes[0]?.datos.copias?.find((x) => x.id === copia.id)?.ganchos?.length, () => {
          copia.ultima = { cuando: ahora(), estado: "ok", bytes: 214_000_000 };
        });
      return resultado(e, o, "hecha", "Copia pedida: empieza en unos segundos.");
    }
    case "subir_ahora":
      await espera(1200);
      empezarTarea(e.cliente, e.id, String(c.repo ?? ""), "copia_externa", 30_000);
      return resultado(e, o, "hecha", "Subida pedida: empieza en unos segundos.");
    case "verificar_ahora":
      // La verificación se ve en marcha (v1.25) mientras el equipo responde.
      empezarTarea(e.cliente, e.id, String(c.repo ?? ""), "verificar", 20_000);
      await espera(21_500);
      return resultado(e, o, "hecha", "Verificación correcta: sin errores en el 5 % revisado.");
    case "probar_restauracion":
      await espera(3000);
      return resultado(e, o, "hecha", "Prueba de restauración correcta: 25 archivos comparados.");
    case "pausar": {
      const h = Number(c.horas ?? 0);
      if (e.resumen) e.resumen.pausado_hasta = h > 0 ? new Date(Date.now() + h * 3600_000).toISOString() : "indefinido";
      return resultado(e, o, "hecha", "Copias en pausa.");
    }
    case "reanudar":
      if (e.resumen) e.resumen.pausado_hasta = null;
      return resultado(e, o, "hecha", "Copias reanudadas.");
    case "crear_repositorio": {
      // restic init tarda: unos 8 s «en marcha», para ver el hueco «Creando…» en su lista.
      o.estado = "en_marcha";
      o.actualizada = ahora();
      await espera(7500);
      const repoId = String(c.id);
      if (String(c.contrasena ?? "").length < 8) return resultado(e, o, "fallida", "La contraseña del repositorio necesita al menos 8 caracteres.");
      const d = c.destino as { id: string; nombre?: string; tipo?: T.DestinoResumen["tipo"]; donde?: string; equipo_almacen?: string };
      e.contrasenas[repoId] = String(c.contrasena);
      e.resumen ??= {};
      if (d.nombre && !(e.resumen.destinos ?? []).some((x) => x.id === d.id))
        e.resumen.destinos = [...(e.resumen.destinos ?? []), { id: d.id, nombre: d.nombre, tipo: d.tipo ?? "otro", donde: d.tipo === "local" ? undefined : d.donde, equipo_almacen: d.tipo === "rest" ? (d.equipo_almacen ?? null) : null }];
      e.resumen.repositorios = [...(e.resumen.repositorios ?? []), { id: repoId, nombre: String(c.nombre), destino: d.id, versiones: 0, bytes: 0 }];
      guardarConfig(e, configInicial(e), plana.seq);
      return resultado(e, o, "hecha", "Repositorio creado (restic init).");
    }
    case "guarda_copias": {
      e.resumen ??= {};
      // Espejo nocturno a otra carpeta (otro disco): { carpeta, hora } o null para quitarlo.
      // §3b: confirmar lo que falta de golpe en el almacén (espera, como el agente).
      if ("espejo_freno" in c) {
        if (!plana.not_before) return resultado(e, o, "rechazada", "Confirmar lo que falta en el almacén reduce la protección: falta la espera (not_before).");
        const f = c.espejo_freno as DestinoEspejo;
        const d = e.resumen.guarda_copias?.espejo?.destinos?.find((x) => claveEspejo(x) === claveEspejo(f));
        if (!d) return resultado(e, o, "fallida", "Ese destino ya no está en el espejo.");
        d.freno = null;
        return resultado(e, o, "hecha", "Confirmado: la próxima vez que se copie al espejo se anota lo que ya no está en el almacén y se borrará pasados sus días.");
      }
      if ("espejo" in c) {
        // v1.9: { destinos: [carpeta | nube], hora?, limite_kib? } con la lista
        // entera (o la forma antigua { carpeta, hora }); null lo quita todo.
        // Como el agente: quitar un destino que ya estaba exige la espera.
        const g = e.resumen.guarda_copias;
        if (!g?.activo) return resultado(e, o, "fallida", "Este equipo no guarda copias.");
        const actuales = g.espejo?.destinos ?? [];
        if (c.espejo === null) {
          if (actuales.length && !plana.not_before) return resultado(e, o, "rechazada", "Quitar el espejo es destructivo: falta la espera (not_before).");
          g.espejo = null;
          return resultado(e, o, "hecha", "Espejo quitado: lo ya copiado se queda en sus destinos.");
        }
        const es = c.espejo as { destinos?: DestinoEspejo[]; carpeta?: string; hora?: string; limite_kib?: number };
        const destinos = destinosDeCuerpo(es);
        const hora = es.hora ?? g.espejo?.hora ?? "02:00";
        if (!destinos.length || !/^([01]\d|2[0-3]):[0-5]\d$/.test(hora)) return resultado(e, o, "fallida", "Faltan los destinos o la hora (HH:MM).");
        for (const d of destinos) {
          if (d.tipo === "nube" && !(g.nubes ?? []).some((n) => n.nombre === d.nube)) return resultado(e, o, "fallida", `No hay ninguna nube conectada llamada «${d.nube}» en este equipo.`);
          if (!d.carpeta?.trim()) return resultado(e, o, "fallida", "A un destino le falta la carpeta.");
          // Como el agente (v1.10): carpeta de un disco del equipo, fuera de Windows, programas y Resguardo.
          const malCarpeta = d.tipo === "carpeta" ? errorCarpetaEspejo(d.carpeta, /windows/i.test(e.so)) : null;
          if (malCarpeta) return resultado(e, o, "fallida", malCarpeta);
        }
        // Como el agente: quitar un destino, o repositorios de su selección, exige la espera.
        if (esDestructiva("guarda_copias", c, undefined, { espejo: g.espejo }) && !plana.not_before) return resultado(e, o, "rechazada", "Quitar un destino del espejo (o repositorios de él) es destructivo: falta la espera (not_before).");
        const previos = new Map(actuales.map((d) => [claveEspejo(d), d]));
        g.espejo = {
          hora,
          ultima: g.espejo?.ultima ?? null,
          resultado: g.espejo?.resultado ?? null,
          limite_kib: es.limite_kib ?? null,
          destinos: destinos.map((d) => ({ ...d, ultima: previos.get(claveEspejo(d))?.ultima ?? null, resultado: previos.get(claveEspejo(d))?.resultado ?? null })),
        };
        if (destinos.some((d) => (d as { horario?: unknown }).horario)) return resultado(e, o, "hecha", `Espejo a ${destinos.length === 1 ? "1 destino" : `${destinos.length} destinos`} con su horario (solo añade).`);
        return resultado(e, o, "hecha", `Espejo a ${destinos.length === 1 ? "1 destino" : `${destinos.length} destinos`} cada noche a las ${hora} (solo añade).`);

      }
      if (c.activo === true) {
        const malCarpeta = errorCarpetaLocal(String(c.carpeta ?? ""), /windows/i.test(e.so));
        if (malCarpeta) return resultado(e, o, "fallida", malCarpeta);
        e.resumen.guarda_copias = { activo: true, puerto: Number(c.puerto ?? 8000), solo_red_local: c.solo_red_local !== false, usuarios: 0 };
        return resultado(e, o, "hecha", "Servidor de copias en marcha (solo añadir, TLS propio).");
      }
      if (c.activo === false) {
        e.resumen.guarda_copias = null;
        return resultado(e, o, "hecha", "Ya no guarda copias de otros equipos.");
      }
      // Tarea 7b: zonas (otros discos con su puerto), como el agente con `admite: "zonas_almacen"`.
      const g7 = e.resumen.guarda_copias;
      const admiteZonasMock = !!e.resumen.admite?.includes("zonas_almacen");
      if (c.zona && typeof c.zona === "object" && admiteZonasMock && g7?.activo) {
        const z = c.zona as { id?: string; nombre?: string; carpeta?: string; puerto?: number };
        if (z.id) {
          const x = (g7.zonas ?? []).find((y) => y.id === z.id);
          if (!x) return resultado(e, o, "fallida", "Esa zona ya no está en este almacén.");
          x.nombre = String(z.nombre ?? "").trim() || x.nombre;
          return resultado(e, o, "hecha", `Zona renombrada: «${x.nombre}».`);
        }
        const carpeta = String(z.carpeta ?? "").trim();
        const puerto = Number(z.puerto);
        const malCarpeta = errorCarpetaLocal(carpeta, /windows/i.test(e.so));
        if (malCarpeta) return resultado(e, o, "fallida", malCarpeta);
        if (!Number.isInteger(puerto) || puerto < 1024 || puerto === g7.puerto || (g7.zonas ?? []).some((y) => y.puerto === puerto)) return resultado(e, o, "fallida", `El puerto ${puerto} ya es de este almacén o no vale: elige otro.`);
        const id = `z${[...crypto.getRandomValues(new Uint8Array(3))].map((b) => b.toString(16).padStart(2, "0")).join("")}`;
        const nombre = String(z.nombre ?? "").trim() || (/^[a-z]:/i.test(carpeta) ? `Disco ${carpeta[0].toUpperCase()}` : carpeta.split(/[\\/]/).pop() || "Otra zona");
        g7.zonas = [...(g7.zonas ?? []), { id, nombre, carpeta, puerto, usuarios: 0, escucha: true, espacio: { libre: 900_000_000_000, total: 1_000_000_000_000, leido: new Date().toISOString() }, repositorios: [] }];
        return resultado(e, o, "hecha", `Zona «${nombre}» lista en el puerto ${puerto} (solo añadir).`, JSON.stringify({ zona: id }));
      }
      if (typeof c.quitar_zona === "string" && admiteZonasMock && g7) {
        if (!plana.not_before) return resultado(e, o, "rechazada", "Quitar una zona reduce la protección: falta la espera (not_before).");
        const x = (g7.zonas ?? []).find((y) => y.id === c.quitar_zona);
        if (!x) return resultado(e, o, "fallida", "Esa zona ya no está en este almacén.");
        g7.zonas = (g7.zonas ?? []).filter((y) => y !== x);
        return resultado(e, o, "hecha", `Zona «${x.nombre}» quitada: sus equipos ya no pueden copiar allí (lo guardado se queda en su carpeta).`);
      }
      if (typeof c.anadir === "string") {
        if (!e.resumen.guarda_copias?.activo) return resultado(e, o, "fallida", "Este equipo no guarda copias.");
        if (!plana.responder_a) return resultado(e, o, "rechazada", "Falta responder_a para entregar el acceso.");
        // Tarea 7b: en una zona (un agente anterior la ignoraría: la consola lo comprueba).
        const zonaPedida = admiteZonasMock && typeof c.zona === "string" ? (e.resumen.guarda_copias.zonas ?? []).find((y) => y.id === c.zona) : undefined;
        if (admiteZonasMock && typeof c.zona === "string" && !zonaPedida) return resultado(e, o, "fallida", "Esa zona ya no está en este almacén.");
        if (zonaPedida) {
          const usuario = `${(estado.equipos.find((x) => x.id === c.anadir)?.nombre ?? "equipo").toLowerCase().replace(/[^a-z0-9]+/g, "-")}-2`;
          const host = c.local === true ? "localhost" : "192.168.1.20";
          const acceso = {
            usuario,
            contrasena: aB64(crypto.getRandomValues(new Uint8Array(18))),
            destino: { tipo: "rest", donde: `https://${host}:${zonaPedida.puerto}/${usuario}/`, usuario, secreto: aB64(crypto.getRandomValues(new Uint8Array(18))), ca_pem: "-----BEGIN CERTIFICATE-----\nMIIB(simulado)\n-----END CERTIFICATE-----\n" },
            huella_tls: "3F:A2:91:0C:7D:44:E8:12:5B:C0:9A:61:2E:F3:88:D7:41:0B:6C:9E:25:73:AA:5D:08:E4:C1:7F:39:B2:66:1A",
            zona: zonaPedida.id,
          };
          zonaPedida.usuarios += 1;
          return resultado(e, o, "hecha", `Equipo cliente «${usuario}» añadido en la zona «${zonaPedida.nombre}».`, JSON.stringify({ sellado: aB64(sellar(deB64(plana.responder_a), utf8(JSON.stringify(acceso)))) }));
        }
        const cliente = estado.equipos.find((x) => x.id === c.anadir);
        const usuario = (cliente?.nombre ?? "equipo").toLowerCase().replace(/[^a-z0-9]+/g, "-");
        // v1.28: `local: true`, su propio almacén (por localhost).
        const host = c.local === true && e.resumen.admite?.includes("almacen_propio") ? "localhost" : "192.168.1.20";
        const acceso = {
          usuario,
          contrasena: aB64(crypto.getRandomValues(new Uint8Array(18))),
          destino: { tipo: "rest", donde: `https://${host}:${e.resumen.guarda_copias.puerto ?? 8000}/${usuario}/`, usuario, secreto: aB64(crypto.getRandomValues(new Uint8Array(18))), ca_pem: "-----BEGIN CERTIFICATE-----\nMIIB(simulado)\n-----END CERTIFICATE-----\n" },
          huella_tls: "3F:A2:91:0C:7D:44:E8:12:5B:C0:9A:61:2E:F3:88:D7:41:0B:6C:9E:25:73:AA:5D:08:E4:C1:7F:39:B2:66:1A",
        };
        e.resumen.guarda_copias.usuarios = (e.resumen.guarda_copias.usuarios ?? 0) + 1;
        return resultado(e, o, "hecha", `Acceso creado para «${usuario}».`, JSON.stringify({ sellado: aB64(sellar(deB64(plana.responder_a), utf8(JSON.stringify(acceso)))) }));
      }
      if (typeof c.quitar === "string") {
        if (e.resumen.guarda_copias) e.resumen.guarda_copias.usuarios = Math.max(0, (e.resumen.guarda_copias.usuarios ?? 1) - 1);
        return resultado(e, o, "hecha", `Acceso de «${c.quitar}» quitado.`);
      }
      return resultado(e, o, "fallida", "Cuerpo no válido.");
    }
    // --- Nubes del espejo (conectar desde la consola, OAuth con PKCE) --------
    case "conectar_nube": {
      const g = e.resumen?.guarda_copias;
      if (!g?.activo) return resultado(e, o, "fallida", "Este equipo no guarda copias.");
      const nombre = String(c.nombre ?? "").trim();
      if (!/^[\p{L}\p{N} _.-]{1,40}$/u.test(nombre)) return resultado(e, o, "fallida", "Nombre de nube no válido.");
      // §3c (agente con `admite: "espejo_destinos"`): B2, S3, SFTP, SMB y WebDAV con sus datos; el agente prueba que entra.
      if (["b2", "s3", "sftp", "smb", "webdav"].includes(String(c.tipo))) {
        if (!e.resumen?.admite?.includes("espejo_destinos")) return resultado(e, o, "fallida", "Tipo de nube no admitido.");
        const p = (c.parametros ?? {}) as Record<string, string>;
        if (String(p.contrasena ?? p.clave ?? "").includes("mal")) return resultado(e, o, "fallida", "No se pudo entrar en ese destino: acceso denegado.");
        g.nubes = [...(g.nubes ?? []).filter((n) => n.nombre !== nombre), { nombre, tipo: String(c.tipo) }];
        return resultado(e, o, "hecha", `«${nombre}» conectado: entra y ya se puede usar como destino del espejo.`);
      }
      if (c.tipo !== "dropbox") return resultado(e, o, "fallida", "Tipo de nube no admitido.");
      if (typeof c.refresh_token !== "string" || !c.refresh_token || typeof c.app_key !== "string") return resultado(e, o, "fallida", "Falta el permiso de Dropbox.");
      // Como el agente: el token se guarda protegido en el equipo; el resumen solo lleva nombre y tipo.
      g.nubes = [...(g.nubes ?? []).filter((n) => n.nombre !== nombre), { nombre, tipo: "dropbox" }];
      return resultado(e, o, "hecha", `Dropbox «${nombre}» conectada (solo Aplicaciones/Resguardo).`);
    }
    case "quitar_nube": {
      const g = e.resumen?.guarda_copias;
      const nombre = String(c.nombre ?? "");
      if (!g?.nubes?.some((n) => n.nombre === nombre)) return resultado(e, o, "fallida", `No hay ninguna nube «${nombre}» en este equipo.`);
      const usada = g.espejo?.destinos?.some((d) => d.tipo === "nube" && d.nube === nombre);
      if (usada && !plana.not_before) return resultado(e, o, "rechazada", "Desconectar una nube que usa el espejo es destructivo: falta la espera (not_before).");
      g.nubes = g.nubes.filter((n) => n.nombre !== nombre);
      // Como el agente: quita ese destino del espejo, y el espejo si se queda vacío.
      if (g.espejo?.destinos) {
        g.espejo.destinos = g.espejo.destinos.filter((d) => !(d.tipo === "nube" && d.nube === nombre));
        if (!g.espejo.destinos.length) g.espejo = null;
      }
      return resultado(e, o, "hecha", `«${nombre}» desconectada: el equipo olvidó su permiso.`);
    }
    // --- F6: cambiar de servidor, respaldo y restaurar en otro equipo -------
    case "cambiar_servidor": {
      const url = String(c.url ?? "");
      if (!/^https:\/\//.test(url) || !c.ficha || deB64(String(c.identidad ?? "")).length !== 32) return resultado(e, o, "fallida", "Faltan la dirección, la identidad o la ficha del servidor nuevo.");
      // Como el agente: «en marcha» y, cuando ya está en el nuevo, «hecha» (el antiguo lo marca trasladado).
      resultado(e, o, "en_marcha", `Dándose de alta en ${url}…`);
      e.resumen ??= {};
      e.resumen.traslado = { estado: "en_marcha", hacia: url, hasta: new Date(Date.now() + 24 * 3600_000).toISOString() };
      setTimeout(() => {
        if (e.resumen) e.resumen.traslado = null;
        if (/falla/.test(url)) return resultado(e, o, "fallida", `${url} no respondió en 24 h: el equipo sigue en este servidor.`);
        e.modo = "trasladado";
        e.conectado = false;
        resultado(e, o, "hecha", `El equipo ya está en ${url} (identidad comprobada).`);
      }, 7000);
      return;
    }
    // --- v1.35: varias consolas a la vez (docs/consolas-multiples.md) ------------
    case "anadir_consola": {
      const url = String(c.url ?? "");
      const identidad = String(c.identidad ?? "");
      if (!/^https:\/\//.test(url) || !c.ficha || deB64(identidad).length !== 32 || deB64(String(c.k_cfg ?? "")).length !== 32) return resultado(e, o, "fallida", "Faltan la dirección, la identidad, la ficha o la K_cfg de la otra consola.");
      e.resumen ??= {};
      const lista = (e.resumen.consolas ??= [{ id: "principal", nombre: "", url: "https://127.0.0.1:5180", identidad: estado.servidor.identidad, sal_cliente: null, ultimo_contacto: ahora(), desde: null, esta: true }]);
      if (lista.some((x) => x.identidad === identidad)) return resultado(e, o, "fallida", "Esa consola ya gestiona este equipo.");
      if (lista.length >= 5) return resultado(e, o, "fallida", "Este equipo ya tiene 5 consolas: quita alguna antes.");
      const nombre = String(c.nombre ?? "") || new URL(url).host;
      // Como el agente: «en marcha», se da de alta allí (comprueba la autoridad TLS y la identidad) y después «hecha».
      resultado(e, o, "en_marcha", `Conectando también a ${nombre}…`);
      setTimeout(() => {
        if (/falla/.test(url)) return resultado(e, o, "fallida", `No se pudo conectar a ${nombre}: no responde en esa dirección y puerto.`);
        lista.push({ id: randomUUID(), nombre, url, identidad, sal_cliente: String(c.sal_cliente ?? "") || null, ultimo_contacto: ahora(), desde: ahora(), esta: false });
        resultado(e, o, "hecha", `Conectado también a ${nombre} (${url}): esa consola ya gestiona este equipo.`);
      }, 4000);
      return;
    }
    case "quitar_consola": {
      const lista = e.resumen?.consolas ?? [];
      const x = lista.find((y) => y.identidad === c.identidad);
      if (!x) return resultado(e, o, "fallida", "Esa consola no gestiona este equipo.");
      if (lista.length < 2) return resultado(e, o, "fallida", "Es la única consola de este equipo: para dejarla, usa «Desvincular».");
      e.resumen!.consolas = lista.filter((y) => y !== x);
      if (x.esta) {
        e.modo = "local";
        e.conectado = false;
      }
      return resultado(e, o, "hecha", `Quitada la consola ${x.nombre} (${x.url}): ya no gestiona este equipo.`);
    }
    // v1.56: el nombre, las etiquetas y la observación del equipo, iguales en todas sus consolas
    // (lo guarda el equipo y el servidor lo copia de su resumen) y olvidar un destino sin uso.
    case "nombre_equipo":
    case "etiquetas_equipo":
    case "observacion_equipo": {
      if (!e.resumen?.admite?.includes("datos_equipo")) return resultado(e, o, "rechazada", "Este agente aún no admite esta orden: actualízalo.");
      const campo = { cuando: ahora(), consola: "Consola de pruebas", esta: true, por: plana.por ?? null };
      e.resumen.datos_equipo ??= {};
      if (plana.tipo === "nombre_equipo") {
        const n = String(c.nombre ?? "").trim();
        if (!n || n.length > 80) return resultado(e, o, "fallida", "Escribe un nombre (hasta 80 caracteres, sin caracteres de control).");
        e.resumen.datos_equipo.nombre = { valor: n, ...campo };
        e.nombre = n;
        return resultado(e, o, "hecha", `Nombre del equipo: «${n}». Lo verán así todas sus consolas.`);
      }
      if (plana.tipo === "etiquetas_equipo") {
        const xs = Array.isArray(c.etiquetas) ? (c.etiquetas as unknown[]).map((x) => String(x).trim()).filter(Boolean) : null;
        if (!xs || xs.length > 10 || xs.some((x) => x.length > 32 || x.includes(","))) return resultado(e, o, "fallida", "Etiqueta no válida (hasta 32 caracteres, sin comas).");
        e.resumen.datos_equipo.etiquetas = { valor: xs, ...campo };
        e.etiquetas = xs;
        return resultado(e, o, "hecha", xs.length ? `Etiquetas: ${xs.join(", ")}. Las verán así todas sus consolas.` : "Etiquetas quitadas en todas sus consolas.");
      }
      const t = String(c.texto ?? "").replace(/\r\n?/g, "\n").trim();
      if (t.length > 2000) return resultado(e, o, "fallida", "Como mucho 2000 caracteres.");
      e.resumen.datos_equipo.observacion = { valor: t, ...campo };
      observacionDelEquipo(e.cliente, e.id, t, `${plana.por ?? "El equipo"} (desde la consola «Consola de pruebas»)`);
      return resultado(e, o, "hecha", t ? "Observación guardada: la verán todas sus consolas." : "Observación quitada en todas sus consolas.");
    }
    case "quitar_destino": {
      const d = e.resumen?.destinos?.find((x) => x.id === c.destino);
      if (!d || !e.resumen) return resultado(e, o, "fallida", "Ese destino ya no está en este equipo.");
      const usos = (e.resumen.repositorios ?? []).flatMap((r) => [
        ...(r.destino === d.id || r.destino === d.nombre ? [`el repositorio «${r.nombre}»`] : []),
        ...(r.externa?.destino_id === d.id ? [`la copia externa de «${r.nombre}»`] : []),
        ...((r.derivadas ?? []).some((x) => x.destino_id === d.id) ? [`una copia derivada de «${r.nombre}»`] : []),
      ]);
      if (usos.length) return resultado(e, o, "fallida", `No se puede quitar: lo usa ${usos.join(", ")}. Quita eso antes.`);
      e.resumen.destinos = e.resumen.destinos!.filter((x) => x !== d);
      return resultado(e, o, "hecha", d.tipo === "local" ? `Destino «${d.nombre}» quitado del equipo. Su carpeta aún tiene copias guardadas (1 repositorio): no se ha borrado nada; si ya no las quieres, bórralas a mano.` : `Destino «${d.nombre}» quitado del equipo, con sus credenciales. Lo guardado allí se queda.`);
    }
    // v1.49: cancelar una orden en espera (de cualquiera de las consolas del equipo).
    case "cancelar_espera": {
      const lista = e.resumen?.en_espera ?? [];
      const x = lista.find((y) => y.id === c.id);
      if (!x) return resultado(e, o, "fallida", "Esa orden ya no está esperando: se aplicó, se canceló o caducó.");
      e.resumen!.en_espera = lista.filter((y) => y !== x);
      return resultado(e, o, "hecha", `Cancelada: ${x.descripcion ?? x.tipo} (la mandó «${x.consola.nombre ?? "otra consola"}»). No se aplicará.`);
    }
    case "servidores_respaldo": {
      const lista = (c.servidores ?? []) as { url?: string; identidad?: string; ficha?: string }[];
      const dias = Number(c.dias ?? 3);
      if (!Array.isArray(lista) || lista.length > 3) return resultado(e, o, "fallida", "Como mucho tres servidores de respaldo.");
      if (!(dias >= 1 && dias <= 30)) return resultado(e, o, "fallida", "Los días sin respuesta deben estar entre 1 y 30.");
      if (lista.some((x) => !/^https:\/\//.test(x.url ?? "") || !x.ficha)) return resultado(e, o, "fallida", "A algún servidor le falta la dirección o la ficha.");
      e.resumen ??= {};
      e.resumen.servidores_respaldo = lista.map((x) => ({ url: x.url!, identidad_corta: (x.identidad ?? "").slice(0, 8) }));
      e.resumen.respaldo_dias = dias;
      return resultado(e, o, "hecha", `${lista.length} servidores de respaldo (si este no responde en ${dias} días).`);
    }
    case "compartir_acceso": {
      const repo = e.resumen?.repositorios?.find((x) => x.id === c.repo);
      if (!repo) return resultado(e, o, "fallida", "Ese repositorio no lo gestiona este servidor.");
      const para = c.para as { equipo?: string; box_pub?: string } | undefined;
      if (!para?.equipo || deB64(para.box_pub ?? "").length !== 32) return resultado(e, o, "fallida", "Falta el equipo de destino o su clave.");
      const d = e.resumen?.destinos?.find((x) => x.id === repo.destino) ?? { id: repo.destino, nombre: repo.destino, tipo: "rest", donde: "https://192.168.1.20:8000/" };
      const datos: Record<string, unknown> = { repo: { id: repo.id, nombre: repo.nombre }, destino: { ...d, usuario: "simulado", secreto: "simulado" } };
      if (c.incluir_contrasena === true) datos.contrasena = e.contrasenas[repo.id];
      const texto = JSON.stringify(datos);
      const firma = aB64(ed25519.sign(utf8(`resguardo-acceso-v1|${para.equipo}|${texto}`), e.secretaFirma));
      const sobre = JSON.stringify({ datos: texto, firma, de: e.id });
      return resultado(e, o, "hecha", "Acceso compartido, sellado para el otro equipo.", JSON.stringify({ acceso_sellado: aB64(sellar(deB64(para.box_pub!), utf8(sobre))) }));
    }
    case "importar_repositorio": {
      const id = String(c.id ?? "");
      if (!/^[A-Za-z0-9_-]{1,64}$/.test(id) || e.resumen?.repositorios?.some((r) => r.id === id)) return resultado(e, o, "fallida", "Id de repositorio no válido o ya en uso.");
      let origenRepo: string;
      let contrasena: string | undefined;
      let destino: { nombre?: string; tipo?: T.DestinoResumen["tipo"]; donde?: string };
      if (typeof c.acceso_sellado === "string") {
        let abierto: { datos: string; firma: string; de: string };
        try {
          abierto = JSON.parse(deUtf8(abrir(e.secretaBox, deB64(c.acceso_sellado))));
        } catch {
          return resultado(e, o, "fallida", "Acceso sellado no válido.");
        }
        const ok = ed25519.verify(deB64(abierto.firma), utf8(`resguardo-acceso-v1|${e.id}|${abierto.datos}`), deB64(String(c.sign_pub_origen ?? "")));
        if (!ok) return resultado(e, o, "fallida", "El acceso no lo firmó el equipo de origen: no se importa.");
        const d = JSON.parse(abierto.datos) as { repo: { id: string }; destino: typeof destino; contrasena?: string };
        origenRepo = d.repo.id;
        destino = d.destino;
        contrasena = d.contrasena ?? (c.contrasena as string | undefined);
      } else {
        const acc = c.acceso as { destino?: typeof destino; repo?: string; contrasena?: string } | undefined;
        if (!acc?.destino?.donde || !acc.repo) return resultado(e, o, "fallida", "Faltan los datos del kit.");
        origenRepo = acc.repo;
        destino = acc.destino;
        contrasena = acc.contrasena;
      }
      if (!contrasena) return resultado(e, o, "fallida", "Falta la contraseña del repositorio.");
      // «Comprueba que se abre»: la contraseña tiene que ser la del repositorio de origen.
      const dueno = estado.equipos.find((x) => x.contrasenas[origenRepo] !== undefined);
      if (!dueno || dueno.contrasenas[origenRepo] !== contrasena) return resultado(e, o, "fallida", "No se pudo abrir el repositorio: revisa la contraseña y los datos del destino.");
      const original = dueno.resumen?.repositorios?.find((r) => r.id === origenRepo);
      e.resumen ??= {};
      const idDestino = `importado-${id}`;
      e.resumen.destinos = [...(e.resumen.destinos ?? []), { id: idDestino, nombre: destino.nombre ?? "Importado", tipo: destino.tipo ?? "otro", donde: destino.tipo === "local" ? undefined : destino.donde }];
      e.resumen.repositorios = [...(e.resumen.repositorios ?? []), { ...(original ?? { versiones: 0, bytes: 0 }), id, nombre: String(c.nombre || id), destino: idDestino, solo_lectura: true, externa: null }];
      e.contrasenas[id] = contrasena;
      guardarConfig(e, configInicial(e), plana.seq);
      return resultado(e, o, "hecha", `Repositorio importado (solo lectura, ${original?.versiones ?? 0} versiones).`);
    }
    // --- v1.14: venir de la app de escritorio -----------------------------------
    // Simulación: la contraseña «mala» no abre; una dirección con «nada» no tiene
    // repositorio; con «:8001» el servidor es de solo añadir (como el de Siigo).
    case "adoptar_repositorio": {
      const d = (c.destino ?? {}) as { id?: string; nombre?: string; tipo?: T.DestinoResumen["tipo"]; donde?: string };
      const contrasena = String(c.contrasena ?? "");
      if (!contrasena) return resultado(e, o, "fallida", "Falta la contraseña del repositorio.");
      if (/nada/i.test(`${d.donde}/${c.ruta}`)) return resultado(e, o, "fallida", "No hay ningún repositorio en esa dirección. Revisa la dirección y el nombre de la carpeta del repositorio.");
      if (/mala/i.test(contrasena)) return resultado(e, o, "fallida", "La contraseña no abre ese repositorio: revisa que sea la suya (la del kit o la que usaba la app de escritorio).");
      const soloAnadir = d.tipo === "rest" ? /:8001/.test(String(d.donde)) : null;
      const ultima = new Date(Date.now() - 86400_000).toISOString();
      const enUso = e.resumen?.repositorios?.find((r) => r.id === `adoptado-${String(c.ruta)}`)?.nombre ?? null;
      const info = { versiones: 412, ultima, solo_anadir: soloAnadir, en_uso: enUso, equipos: ["PC-CONTABLE", "PORTATIL-GERENCIA"], etiquetas: ["escritorio"] };
      const servidor = soloAnadir ? " El servidor es de solo añadir: la retención se aplica en él." : "";
      if (c.solo_probar === true) {
        const datos = plana.responder_a ? info : { versiones: info.versiones, ultima, solo_anadir: soloAnadir, en_uso: enUso };
        const det = detallePara(plana, datos);
        return resultado(e, o, "hecha", `Se abre con esa contraseña: 412 versiones, la última de ayer.${servidor}`, det ? JSON.stringify({ sellado: det }) : JSON.stringify(datos));
      }
      if (enUso) return resultado(e, o, "fallida", `Este equipo ya usa ese repositorio («${enUso}»): no hace falta adoptarlo otra vez.`);
      const id = String(c.id ?? "");
      if (!/^[A-Za-z0-9_-]{1,64}$/.test(id) || e.resumen?.repositorios?.some((r) => r.id === id)) return resultado(e, o, "fallida", "Id de repositorio no válido o ya en uso.");
      e.resumen ??= {};
      if (d.id && d.tipo && !(e.resumen.destinos ?? []).some((x) => x.id === d.id))
        e.resumen.destinos = [...(e.resumen.destinos ?? []), { id: d.id, nombre: d.nombre ?? String(d.donde ?? "Destino"), tipo: d.tipo, donde: d.tipo === "local" ? undefined : d.donde }];
      e.resumen.repositorios = [
        ...(e.resumen.repositorios ?? []),
        { id, nombre: String(c.nombre || id), destino: String(d.id), versiones: 412, bytes: 37_500_000_000, ultima_version: ultima, solo_anadir: soloAnadir, externa: null },
      ];
      e.contrasenas[id] = contrasena;
      guardarConfig(e, configInicial(e), plana.seq);
      return resultado(e, o, "hecha", `Repositorio «${String(c.nombre || id)}» adoptado con todo su historial (412 versiones, la última de ayer). Las copias pueden guardar ya en él.${servidor}`, JSON.stringify(info));
    }
    case "copiar_historial": {
      const repo = e.resumen?.repositorios?.find((r) => r.id === c.repo);
      if (!repo) return resultado(e, o, "fallida", "Ese repositorio no lo gestiona este servidor.");
      if (repo.solo_lectura) return resultado(e, o, "fallida", "Ese repositorio es de solo lectura (importado de otro equipo): trae el historial a uno propio.");
      const origen = (c.origen ?? {}) as { repo?: string; contrasena?: string };
      if (origen.repo === repo.id) return resultado(e, o, "fallida", "El origen es el mismo repositorio: elige otro.");
      if (/mala/i.test(String(origen.contrasena ?? ""))) return resultado(e, o, "fallida", "Repositorio de origen: la contraseña no abre ese repositorio.");
      // Como el agente: «en marcha» con el progreso y, al final, «hecha». De un
      // repositorio de este equipo (`origen.repo`, «Mover a otro sitio…»), sus
      // versiones y lo que ocupa; lo ya traído no se repite.
      const deAqui = origen.repo ? e.resumen?.repositorios?.find((r) => r.id === origen.repo) : undefined;
      if (origen.repo && !deAqui) return resultado(e, o, "fallida", "El repositorio de origen no lo gestiona este equipo.");
      const total = deAqui ? (deAqui.versiones ?? 0) : 40;
      const ya = deAqui ? Math.min(total, repo.versiones ?? 0) : 0;
      const faltan = total - ya;
      let hechas = 0;
      resultado(e, o, "en_marcha", `Trayendo el historial: 0 de ${faltan} versiones…`);
      // v1.47: a la vista de todas las consolas (progreso `historial`; con `mover`, un paso de «Mover a otro sitio…»).
      const paso = (c.mover as { paso?: string } | undefined)?.paso;
      empezarHistorial(e.cliente, e.id, repo.id, repo.nombre, faltan, Math.max(1, Math.ceil(faltan / Math.max(8, Math.ceil(faltan / 6)))) * 2000, { origen: deAqui?.id ?? null, nombre_origen: deAqui?.nombre ?? null, mover: !!paso, paso: paso === "ultimo" ? "ultimo" : paso ? "historial" : null, otra_consola: false });
      const t = setInterval(() => {
        hechas = Math.min(faltan, hechas + Math.max(8, Math.ceil(faltan / 6)));
        if (hechas < faltan) return resultado(e, o, "en_marcha", `Trayendo el historial: ${hechas} de ${faltan} versiones…`);
        clearInterval(t);
        repo.versiones = deAqui ? total : (repo.versiones ?? 0) + total;
        if (deAqui) {
          repo.bytes = deAqui.bytes;
          repo.ultima_version = deAqui.ultima_version;
        }
        resultado(e, o, "hecha", `Historial traído: ${faltan} versiones nuevas.${ya ? ` ${ya} ya estaban.` : ""}`);
      }, 2000);
      return;
    }
    case "cambiar_copia_externa": {
      // Como el agente: el repositorio tiene que tener copias activas en este
      // equipo y el destino tiene que ser otro; hora null la quita.
      const repo = e.resumen?.repositorios?.find((x) => x.id === c.repo);
      if (!repo) return resultado(e, o, "fallida", "Ese repositorio no existe en este equipo.");
      if (c.hora === null) {
        const antes = repo.externa?.destino;
        repo.externa = null;
        guardarConfig(e, configInicial(e), plana.seq);
        return resultado(e, o, "hecha", antes ? `Ya no se copia cada día a «${antes}».` : "No tenía copia externa.");
      }
      if (!(e.resumen?.copias ?? []).some((k) => k.repo === repo.id && k.activa !== false)) return resultado(e, o, "fallida", "Ese repositorio no tiene copias activas en este equipo.");
      if (!/^([01]\d|2[0-3]):[0-5]\d$/.test(String(c.hora))) return resultado(e, o, "fallida", "La hora no es válida (HH:MM).");
      const d = c.destino as { id: string; nombre?: string; tipo?: T.DestinoResumen["tipo"]; donde?: string };
      if (!d?.id) return resultado(e, o, "fallida", "Falta el destino.");
      if (d.id === repo.destino) return resultado(e, o, "fallida", "La copia externa tiene que ir a otro destino.");
      // v1.46: a uno que ya existe (con su contraseña), con bloqueo de objetos, o solo probar.
      const existente = c.existente === true;
      const bloqueo = Number(c.bloqueo_dias ?? 0) || null;
      if (existente && typeof c.ruta !== "string") return resultado(e, o, "fallida", "Falta la carpeta del repositorio que ya existe.");
      if (existente && !String(c.contrasena_destino ?? "")) return resultado(e, o, "fallida", "La contraseña no abre ese repositorio: revisa que sea la suya.");
      const efecto = bloqueo
        ? c.retencion
          ? ` Con bloqueo de ${bloqueo} días: la retención de allí solo quita versiones de más de ${bloqueo} días (forget, sin prune) y no libera espacio.`
          : ` Con bloqueo de ${bloqueo} días: allí no se borra nada (sin retención propia).`
        : "";
      const probado = existente ? "El repositorio que ya existe se abre con esa contraseña (213 versiones). Trocea igual que el origen: cada subida solo sube lo que falte allí." : "";
      if (c.solo_probar === true) return resultado(e, o, "hecha", `${probado || "El destino responde y allí aún no hay ningún repositorio: se creará al guardar, con el mismo troceado que el origen."}${efecto}`);
      let destino = e.resumen!.destinos?.find((x) => x.id === d.id);
      if (!destino) {
        if (!d.tipo || !d.donde || (!d.nombre && !existente)) return resultado(e, o, "fallida", "Ese destino no existe en este equipo.");
        destino = { id: d.id, nombre: d.nombre || (d.tipo === "b2" ? "Backblaze B2" : d.tipo === "s3" ? "S3" : "Destino"), tipo: d.tipo, donde: d.donde };
        e.resumen!.destinos = [...(e.resumen!.destinos ?? []), destino];
      }
      const antes = repo.externa?.destino_id === destino.id ? repo.externa : null;
      repo.externa = {
        destino: destino.nombre,
        destino_id: destino.id,
        hora: String(c.hora),
        existente: existente || !!antes?.existente || null,
        bloqueo_dias: "bloqueo_dias" in c ? bloqueo : (antes?.bloqueo_dias ?? null),
        con_retencion: !!c.retencion,
      };
      guardarConfig(e, configInicial(e), plana.seq);
      const sep = /windows/i.test(e.so) ? "\\" : "/";
      const ruta = !existente && destino.tipo === "local" && destino.donde ? ` (${destino.donde.replace(/[\\/]+$/, "")}${sep}${repo.id})` : "";
      return resultado(e, o, "hecha", `Copia externa a «${destino.nombre}»${ruta} cada día a las ${c.hora}.${probado ? ` ${probado}` : ""}${efecto}`);
    }
    // Tarea 4b: las demás copias derivadas (como el agente con `admite: "derivadas"`).
    case "cambiar_derivada": {
      const repo = e.resumen?.repositorios?.find((x) => x.id === c.repo);
      if (!repo) return resultado(e, o, "fallida", "Ese repositorio no lo gestiona este servidor.");
      if (!e.resumen?.admite?.includes("derivadas")) return resultado(e, o, "fallida", "Tipo de orden desconocido.");
      const id = String(c.id ?? "");
      if (!/^[a-z0-9_-]{1,40}$/.test(id) || id === "externa") return resultado(e, o, "fallida", "Id de copia derivada no válido.");
      const d = c.destino as { id: string; nombre?: string; tipo?: T.DestinoResumen["tipo"]; donde?: string; nube?: string };
      if (!d?.id) return resultado(e, o, "fallida", "Falta el destino.");
      if (d.id === repo.destino || e.resumen?.destinos?.find((x) => x.id === d.id)?.nombre === repo.destino) return resultado(e, o, "fallida", "La copia tiene que ir a otro destino (otro disco, otro servidor o la nube).");
      if (d.tipo === "nube" && !(e.resumen?.nubes ?? []).some((n) => n.nombre === d.nube)) return resultado(e, o, "fallida", `La nube «${d.nube}» no está conectada en este equipo: conéctala antes.`);
      if (c.solo_probar === true) return resultado(e, o, "hecha", "El destino responde y allí aún no hay ningún repositorio: se creará al guardar, con el mismo troceado que el origen.");
      if (!(e.resumen?.copias ?? []).some((k) => k.repo === repo.id && k.activa !== false)) return resultado(e, o, "fallida", "Ese repositorio aún no tiene copias activas en este equipo: aplica antes una configuración con alguna copia.");
      let destino = e.resumen!.destinos?.find((x) => x.id === d.id);
      if (!destino) {
        if (!d.tipo || (!d.donde && d.tipo !== "nube")) return resultado(e, o, "fallida", `No hay ningún destino «${d.id}» en este equipo.`);
        destino = { id: d.id, nombre: d.nombre || "Destino", tipo: d.tipo, donde: d.donde, ...(d.nube ? { nube: d.nube } : {}) };
        e.resumen!.destinos = [...(e.resumen!.destinos ?? []), destino];
      }
      const cuando = c.tras_copia === true ? { tras_copia: true } : c.horario ? { horario: c.horario as T.Horario } : { hora: String(c.hora ?? "") };
      const filtro = c.filtro as T.FiltroVersiones | undefined;
      const nueva: T.DerivadaResumen = {
        id,
        destino: destino.nombre,
        destino_id: destino.id,
        cuando,
        bloqueo_dias: Number(c.bloqueo_dias ?? 0) || null,
        filtro: filtro ? { ...filtro, ...(Array.isArray(filtro.carpetas) ? { carpetas: filtro.carpetas.length } : {}) } : null,
        verificacion: (c.verificacion as T.VerificacionAuto | undefined) ?? null,
        activa: true,
        con_retencion: !!c.retencion,
      };
      repo.derivadas = [...(repo.derivadas ?? []).filter((x) => x.id !== id), nueva];
      guardarConfig(e, configInicial(e), plana.seq);
      return resultado(e, o, "hecha", `Copia derivada a «${destino.nombre}» guardada. Repositorio creado en el destino, con el mismo troceado que el origen.`);
    }
    case "quitar_derivada": {
      const repo = e.resumen?.repositorios?.find((x) => x.id === c.repo);
      if (!repo?.derivadas?.some((x) => x.id === c.id)) return resultado(e, o, "fallida", "Esa copia derivada ya no está.");
      repo.derivadas = repo.derivadas.filter((x) => x.id !== c.id);
      guardarConfig(e, configInicial(e), plana.seq);
      return resultado(e, o, "hecha", "Copia derivada quitada (lo ya copiado sigue en su destino).");
    }
    case "cambiar_destino": {
      const d = e.resumen?.destinos?.find((x) => x.id === c.destino);
      if (!d) return resultado(e, o, "fallida", "Ese destino no existe en este equipo.");
      if (typeof c.donde === "string" && c.donde) d.donde = c.donde;
      return resultado(e, o, "hecha", `Destino «${d.nombre}» actualizado: todos sus repositorios se abren con los datos nuevos.`);
    }
    // --- v1.22: retención en el almacén ------------------------------------------
    // El equipo dueño añade la clave del almacén (restic key add); el almacén la
    // guarda con la regla y la aplica en local a su hora (aquí, al pedirlo).
    case "clave_almacen": {
      const repo = e.resumen?.repositorios?.find((x) => x.id === c.repo);
      if (!repo) return resultado(e, o, "fallida", "Ese repositorio no lo gestiona este servidor.");
      if (typeof c.clave !== "string" || c.clave.length < 20) return resultado(e, o, "fallida", "Clave del almacén no válida.");
      clavesAlmacen.add(c.clave);
      await espera(1200);
      return resultado(e, o, "hecha", `Clave del almacén añadida a «${repo.nombre}»: el almacén ya puede aplicar su retención.`);
    }
    case "retencion_almacen": {
      const g = e.resumen?.guarda_copias;
      if (!g?.activo) return resultado(e, o, "fallida", "Este equipo no guarda copias.");
      const usuario = String(c.usuario ?? "");
      const repo = String(c.repo ?? "");
      if (!(g.repositorios ?? []).some((u) => u.usuario === usuario && u.repos.includes(repo))) return resultado(e, o, "fallida", `No hay ningún repositorio «${usuario}/${repo}» en este almacén.`);
      g.retenciones ??= [];
      const previa = g.retenciones.find((r) => r.usuario === usuario && r.repo === repo);
      if (c.quitar === true) {
        g.retenciones = g.retenciones.filter((r) => r !== previa);
        return resultado(e, o, "hecha", previa ? `Este almacén ya no aplica la retención de «${usuario}/${repo}» y ha borrado su clave del repositorio.` : `«${usuario}/${repo}» no tenía retención en este almacén.`);
      }
      if (!plana.not_before) return resultado(e, o, "rechazada", "Poner la retención borra versiones: falta la espera (not_before).");
      const r = c.retencion as T.Regla;
      const h0 = c.horario as T.HorarioRetencion;
      // v1.40: las reglas, solo un almacén que las entiende (uno anterior las ignora).
      const h = h0.reglas?.length && e.resumen?.admite?.includes("retencion_almacen_horario") ? h0 : { dias: h0.dias, hora: h0.hora };
      const err = errorRegla(r, !!g && !!e.resumen?.admite?.includes("retencion_plazos")) ?? errorHorario(h) ?? (h.reglas?.length ? errorReglas(h.reglas) : null);
      if (err) return resultado(e, o, "fallida", err);
      if (!previa && typeof c.clave !== "string") return resultado(e, o, "fallida", "Falta la clave del almacén para este repositorio.");
      const abre = typeof c.clave === "string" ? clavesAlmacen.has(c.clave) : previa?.clave === "ok";
      const nueva: T.RetencionAlmacen = {
        usuario,
        repo,
        retencion: r,
        texto: textoRegla(r),
        horario: h,
        horario_texto: h.reglas?.length ? "según su horario" : textoHorario(h),
        verificar: c.verificar !== false,
        clave: abre ? "ok" : "pendiente",
        ultima: previa?.ultima ?? null,
        resultado: previa?.resultado ?? null,
        mensaje: previa?.mensaje ?? null,
        versiones: previa?.versiones ?? null,
        proxima: new Date(Date.now() + 3 * 86400_000).toISOString(),
      };
      g.retenciones = [...g.retenciones.filter((x) => x !== previa), nueva];
      const m = `Este almacén aplicará la retención de «${usuario}/${repo}» (${nueva.texto}) ${nueva.horario_texto}.`;
      return resultado(e, o, "hecha", abre ? m : `${m} La clave del almacén aún no abre este repositorio: falta que el equipo dueño la añada.`);
    }
    case "aplicar_retencion_almacen": {
      const ra = e.resumen?.guarda_copias?.retenciones?.find((r) => r.usuario === c.usuario && r.repo === c.repo);
      if (!ra) return resultado(e, o, "fallida", `«${String(c.usuario)}/${String(c.repo)}» no tiene retención en este almacén: configúrala antes.`);
      resultado(e, o, "en_marcha", "Aplicando la retención en el almacén…");
      setTimeout(() => {
        ra.ultima = ahora();
        if (ra.clave !== "ok") {
          ra.resultado = "fallo";
          ra.mensaje = "La clave del almacén aún no abre este repositorio: falta que el equipo dueño la añada.";
          return resultado(e, o, "fallida", ra.mensaje);
        }
        ra.resultado = "ok";
        ra.versiones = 31;
        ra.mensaje = `Retención aplicada: 46 versiones quitadas, quedan 31.${ra.verificar ? " Comprobado sin errores." : ""}`;
        resultado(e, o, "hecha", ra.mensaje);
      }, 4000);
      return;
    }
    case "desbloquear":
      await espera(800);
      return resultado(e, o, "hecha", "Bloqueos antiguos quitados.");
    case "cambiar_espera": {
      const h = Number(c.horas);
      return resultado(e, o, "hecha", `Espera: ${h} h.`, JSON.stringify({ espera_min_horas: h }));
    }
    case "restaurar": {
      await espera(2500);
      // Como el agente: «junto» crea «Restaurado AAAA-MM-DD HHMM» en la carpeta de cada elemento.
      const rutas = (Array.isArray(c.rutas) ? c.rutas : []).map(String);
      // v1.10: «en su sitio» solo dentro de las carpetas que copia el equipo (como el agente).
      if (c.destino === "original") {
        const copiadas = configInicial(e).copias.flatMap((k) => k.carpetas).map((x) => x.toLowerCase().replace(/\\/g, "/").replace(/^([a-z]):/, "/$1"));
        const fuera = rutas.find((r) => !copiadas.some((k) => r.toLowerCase() === k || r.toLowerCase().startsWith(`${k}/`)));
        if (fuera) return resultado(e, o, "fallida", `«${fuera}» no está en las carpetas que copia este equipo: restáuralo junto al original.`);
      }
      const d = new Date();
      const p2 = (n: number) => String(n).padStart(2, "0");
      const sello = `Restaurado ${d.getFullYear()}-${p2(d.getMonth() + 1)}-${p2(d.getDate())} ${p2(d.getHours())}${p2(d.getMinutes())}`;
      const local = (r: string) => r.replace(/^\/([A-Za-z])\//, "$1:/").replaceAll("/", "\\");
      const destinos = [...new Set(rutas.map((r) => local(r.slice(0, r.lastIndexOf("/"))) + (c.destino === "original" ? "" : `\\${sello}`)))];
      return resultado(e, o, "hecha", `Restaurado (${rutas.length} elementos) en ${destinos.join(", ")}.`);
    }
    case "descargar":
      await prepararDescarga(e, o, plana);
      return resultado(e, o, "hecha", "Descarga lista en el relé.");
    case "abrir_sesion":
    case "explorar":
    case "elegir_carpetas": {
      // Como el agente (servidor_v2.rs): «hecha» y firmada en cuanto la sesión está abierta. Cerrarla
      // después no toca la orden: el servidor no puede cambiar un resultado firmado por el equipo.
      abrirSesion(e, o, plana);
      return resultado(e, o, "hecha", "Sesión abierta.");
    }
    case "cambiar_retencion": {
      // Como el agente: valida y la guarda (v1.28: horarias, plazos y «siempre» solo si los entiende).
      const repo = e.resumen?.repositorios?.find((x) => x.id === c.repo);
      if (!repo) return resultado(e, o, "fallida", "Ese repositorio no lo gestiona este servidor.");
      const { repo: _r, ...regla } = c as unknown as T.Regla & { repo: string };
      const admite = !!e.resumen?.admite?.includes("retencion_plazos");
      const err = errorRegla(regla, admite);
      if (err) return resultado(e, o, "fallida", admite ? err : "La retención tiene que guardar al menos una versión (y como mucho 1000 de cada tipo).");
      repo.retencion = textoRegla(regla);
      if (admite) repo.retencion_regla = regla;
      return resultado(e, o, "hecha", `Retención guardada: ${textoRegla(regla)}.`);
    }
    case "quitar_repositorio": {
      // Como el agente: lo olvida (y deja de copiar en él); lo guardado sigue en su destino.
      const r = e.resumen?.repositorios?.find((x) => x.id === c.repo);
      if (!r || !e.resumen) return resultado(e, o, "fallida", "Ese repositorio no existe en este equipo.");
      e.resumen.repositorios = (e.resumen.repositorios ?? []).filter((x) => x.id !== r.id);
      e.resumen.copias = (e.resumen.copias ?? []).filter((k) => k.repo !== r.id);
      guardarConfig(e, configInicial(e), plana.seq);
      // v1.56: `quitar_destino: true`, también su destino si se queda sin uso.
      const d = e.resumen.destinos?.find((x) => x.id === r.destino || x.nombre === r.destino);
      const enUso = (e.resumen.repositorios ?? []).some((x) => x.destino === d?.id || x.destino === d?.nombre || x.externa?.destino_id === d?.id || (x.derivadas ?? []).some((y) => y.destino_id === d?.id));
      if (c.quitar_destino === true && d && !enUso && e.resumen.admite?.includes("quitar_destino")) {
        e.resumen.destinos = e.resumen.destinos!.filter((x) => x !== d);
        return resultado(e, o, "hecha", `Repositorio «${r.nombre}» quitado de este equipo. Lo guardado sigue en su destino. Destino «${d.nombre}» quitado del equipo.`);
      }
      return resultado(e, o, "hecha", `Repositorio «${r.nombre}» quitado de este equipo. Lo guardado sigue en su destino.`);
    }
    default:
      await espera(800);
      return resultado(e, o, "hecha", "Hecho.");
  }
}

/** Ejecuta las órdenes destructivas cuyo not_before ya pasó (se llama de vez en cuando). */
export async function revisarEsperas() {
  for (const o of estado.ordenes) {
    if (o.estado !== "pendiente" || !o.not_before || Date.parse(o.not_before) > Date.now() || !o.sellado) continue;
    const e = estado.equipos.find((x) => x.id === o.equipo);
    if (!e) continue;
    const plana: OrdenPlana = JSON.parse(deUtf8(abrir(e.secretaBox, deB64(o.sellado))));
    await ejecutar(e, o, plana);
  }
}

// ---------------------------------------------------------------------------
// Sesiones interactivas
// ---------------------------------------------------------------------------

function abrirSesion(e: EquipoMock, o: OrdenMock, plana: OrdenPlana) {
  const id = o.sesion;
  const c = plana.cuerpo as { clave_sesion?: string; sesion?: string };
  // El id de dentro del sobre tiene que coincidir con el que dice el servidor.
  if (!id || !c.clave_sesion || c.sesion !== id) return;
  const clave = c.clave_sesion;
  const s: SesionMock = {
    id,
    cliente: o.cliente,
    equipo: e.id,
    mensajes: [],
    claveSesion: deB64(clave),
    tipo: plana.tipo,
    repo: (plana.autorizacion?.clave_repo?.repo as string) ?? null,
    recibidosConsola: 0,
    creada: Date.now(),
    ultimo: Date.now(),
  };
  estado.sesionesInteractivas.set(id, s);
  // v1.15: los agentes nuevos dicen qué admiten (los anteriores, no).
  const ops = !versionAlMenos(e.version_agente, VERSION_CREAR_CARPETA)
    ? undefined
    : plana.tipo === "elegir_carpetas"
      ? ["carpetas", "sugerencias", "crear_carpeta"]
      : plana.tipo === "explorar"
        ? ["versiones", "listar", "buscar", ...(versionAlMenos(e.version_agente, VERSION_DETALLE) ? OPS_DETALLE : []), ...(versionAlMenos(e.version_agente, VERSION_BUSCAR) ? [OP_BUSCAR] : [])]
        : [];
  enviarDesdeEquipo(s, { i: 0, op: "lista", tipo: plana.tipo, ...(ops ? { ops } : {}) });
}

/** La consola cierra la sesión (DELETE …/sesiones/{s}): se olvida, sin tocar la orden que la abrió. */
export function cerrarSesion(id: string) {
  estado.sesionesInteractivas.delete(id);
  despertar(id);
}

/** Desde qué versión el agente anuncia `ops` y crea carpetas (v1.15). */
const VERSION_CREAR_CARPETA = "0.7.7";
/** Carpetas creadas desde «Nueva carpeta» (por carpeta de arriba, en minúsculas). */
const creadas = new Map<string, string[]>();

let contadorEquipo = 1;
function enviarDesdeEquipo(s: SesionMock, datos: Record<string, unknown>) {
  const k = claveDireccion(s.claveSesion!, s.id, "equipo");
  const n = s.mensajes.length + 1;
  s.mensajes.push({ n, de: "equipo", cifrado: aB64(cifrarMensaje(k, s.id, { ...datos, i: datos.i ?? contadorEquipo++ })) });
  s.ultimo = Date.now();
  despertar(s.id);
}

/** Lo que la consola manda a la sesión: el «agente» lo descifra y contesta. */
export async function mensajeDeConsola(s: SesionMock, cifrado: string) {
  const k = claveDireccion(s.claveSesion!, s.id, "consola");
  let m: { i: number; op: string; [k: string]: unknown };
  try {
    m = descifrarMensaje(k, s.id, deB64(cifrado));
  } catch {
    return;
  }
  await espera(150 + Math.random() * 250);
  const re = m.i;
  const enviar = (s2: SesionMock, datos: Record<string, unknown>) => enviarDesdeEquipo(s2, { ...datos, re });
  switch (m.op) {
    case "versiones":
      return enviar(s, { op: "versiones", repo: m.repo, versiones: versiones(String(m.repo ?? s.repo), s.equipo) });
    case "listar":
      return enviar(s, { op: "listar", version: m.version, ruta: m.ruta, entradas: listarVersion(String(m.ruta ?? "/")) });
    case "carpetas": {
      const r = String(m.ruta ?? "").replace(/[\\/]+$/, "");
      const nuevas = (creadas.get(r.toLowerCase()) ?? []).map((nombre) => ({ nombre, tipo: "dir" as const }));
      return enviar(s, { op: "carpetas", ruta: m.ruta, entradas: [...nuevas, ...arbol(String(m.ruta ?? ""), false)] });
    }
    case "crear_carpeta": {
      // Como el agente (sesiones_v2.rs, crear_carpeta): solo si lo admite, con las mismas reglas.
      const eq = estado.equipos.find((x) => x.id === s.equipo);
      if (!eq || !versionAlMenos(eq.version_agente, VERSION_CREAR_CARPETA)) return enviar(s, { op: "crear_carpeta", error: "Operación no disponible en esta sesión: «crear_carpeta»." });
      const padre = String(m.ruta ?? "").replace(/[\\/]+$/, "");
      const nombre = String(m.nombre ?? "");
      const ruta = `${padre}\\${nombre}`;
      const mal = errorNombreCarpeta(nombre) ?? (/^[A-Za-z]:$/.test(padre) ? null : errorCarpetaLocal(padre, true)) ?? errorCarpetaLocal(ruta, true) ?? (enCarpetaDelSistema(ruta, true) ? "Aquí no se pueden crear carpetas (es del sistema, de los programas o de Resguardo): elige otro sitio." : null);
      if (mal) return enviar(s, { op: "crear_carpeta", error: mal });
      const l = creadas.get(padre.toLowerCase()) ?? [];
      const ya = l.some((x) => x.toLowerCase() === nombre.toLowerCase()) || arbol(padre, false).some((x) => x.nombre.toLowerCase() === nombre.toLowerCase());
      if (!ya) creadas.set(padre.toLowerCase(), [...l, nombre]);
      return enviar(s, { op: "crear_carpeta", ruta, ya_existia: ya });
    }
    case "sugerencias":
      return enviar(s, {
        op: "sugerencias",
        sugerencias: [
          { id: "usuarios", nombre: "Carpetas de usuarios", detalle: "3 usuarios: recepcion, contador, gerente", rutas: ["C:\\Users\\recepcion", "C:\\Users\\contador", "C:\\Users\\gerente"] },
          { id: "siigo", nombre: "Siigo", detalle: "Datos en C:\\SIIWI01", rutas: ["C:\\SIIWI01"], gancho: "siigo" },
          { id: "sqlserver", nombre: "SQL Server · 2 bases", detalle: "Se vuelcan con COPY_ONLY antes de copiar", rutas: [], gancho: "sqlserver" },
        ],
      });
    case "buscar":
      return enviar(s, {
        op: "buscar",
        texto: m.texto,
        resultados: arbol("C:\\Users\\recepcion\\Documents", true)
          .filter((x) => x.nombre.toLowerCase().includes(String(m.texto ?? "").toLowerCase()))
          .map((x) => ({ ...x, ruta: `/C/Users/recepcion/Documents/${x.nombre}` })),
      });
    case "cerrar":
      estado.sesionesInteractivas.delete(s.id);
      return;
    case "buscar_todas": {
      // «Buscar archivos» en todas las versiones (mock/buscar.ts): solo en `explorar` y si el agente lo anuncia.
      const eq = estado.equipos.find((x) => x.id === s.equipo);
      if (s.tipo !== "explorar" || !eq || !versionAlMenos(eq.version_agente, VERSION_BUSCAR)) return enviar(s, { op: m.op, error: `Operación no disponible en esta sesión: «${m.op}».` });
      return enviar(s, { op: m.op, ...(await buscarTodas(m, s, () => enviarDesdeEquipo(s, { op: "trabajando", sobre: re }))) });
    }
    case "diferencias":
    case "ocupa":
    case "historial_archivo": {
      // v1.33: solo en `explorar` y si el agente lo anuncia (mock/detalle.ts).
      const eq = estado.equipos.find((x) => x.id === s.equipo);
      if (s.tipo !== "explorar" || !eq || !versionAlMenos(eq.version_agente, VERSION_DETALLE)) return enviar(s, { op: m.op, error: `Operación no disponible en esta sesión: «${m.op}».` });
      const r = await operarDetalle(m.op, m, s, () => enviarDesdeEquipo(s, { op: "trabajando", sobre: re }));
      return enviar(s, { op: m.op, ...(r ?? {}) });
    }
  }
}

/** Dentro de una versión las rutas son las de restic («/C/Users/…»). */
function listarVersion(ruta: string): Entrada[] {
  const partes = ruta.split("/").filter(Boolean);
  if (!partes.length) return [{ nombre: "C", tipo: "dir" }, { nombre: "D", tipo: "dir" }];
  return arbol(`${partes[0]}:${partes.length > 1 ? "\\" + partes.slice(1).join("\\") : ""}`, true);
}

function versiones(repo: string, equipo?: string) {
  // Las mismas del informe del equipo (v1.7), si las tiene: así coinciden los ids.
  const inf = estado.equipos.find((x) => x.id === equipo)?.informes[0]?.datos.repos?.find((x) => x.id === repo);
  if (inf?.versiones.length)
    return inf.versiones.map((v) => ({ id: v.id, cuando: v.hora, archivos: (v.archivos_nuevos ?? 0) + (v.archivos_cambiados ?? 0) + (v.archivos_sin_cambios ?? 0), bytes: v.total_bytes ?? 0, etiquetas: v.etiquetas, repo }));
  const out = [];
  for (let i = 0; i < 40; i++) {
    const d = new Date(Date.now() - i * 12 * 3600_000 - 2 * 3600_000);
    out.push({ id: `${(0x1a2b3c4d + i * 7919).toString(16).slice(0, 8)}`, cuando: d.toISOString(), archivos: 14_200 - i * 13, bytes: 38_400_000_000 - i * 52_000_000, etiquetas: i % 14 === 0 ? ["mensual"] : [], repo });
  }
  return out;
}

type Entrada = { nombre: string; tipo: "dir" | "archivo"; bytes?: number; modificado?: string; sistema?: boolean; repositorio?: boolean };

/** Un árbol de mentira, igual para todas las versiones (basta para la interfaz). */
function arbol(ruta: string, version: boolean): Entrada[] {
  const r = ruta.replace(/[\\/]+$/, "");
  const fecha = (n: number) => new Date(Date.now() - n * 86_400_000).toISOString();
  if (r === "") return [{ nombre: "C:", tipo: "dir" }, { nombre: "D:", tipo: "dir" }, ...(version ? [] : [{ nombre: "E:", tipo: "dir" as const }])];
  if (r === "C:")
    return [
      { nombre: "Users", tipo: "dir" },
      { nombre: "SIIWI01", tipo: "dir" },
      { nombre: "Program Files", tipo: "dir", sistema: true },
      { nombre: "Windows", tipo: "dir", sistema: true },
    ];
  if (r === "C:\\Users") return ["recepcion", "contador", "gerente", "Public"].map((n) => ({ nombre: n, tipo: "dir" as const }));
  if (/^C:\\Users\\[^\\]+$/.test(r))
    return [
      { nombre: "Documents", tipo: "dir" },
      { nombre: "Desktop", tipo: "dir" },
      { nombre: "Pictures", tipo: "dir" },
      { nombre: "Downloads", tipo: "dir" },
      { nombre: "AppData", tipo: "dir", sistema: true },
    ];
  if (r === "D:") return [{ nombre: "Escaneos", tipo: "dir" }, { nombre: "Contratos", tipo: "dir" }, ...(version ? [] : [{ nombre: "Copias", tipo: "dir" as const }])];
  // Repositorios de restic (p. ej. de la app de escritorio): el agente los marca al listar (`repositorio`).
  if (!version && r === "D:\\Copias")
    return [
      { nombre: "Contabilidad", tipo: "dir", repositorio: true },
      { nombre: "Nomina", tipo: "dir", repositorio: true },
      { nombre: "Viejas", tipo: "dir" },
    ];
  if (!version && /^D:\\Copias\\(Contabilidad|Nomina)$/i.test(r))
    return [
      { nombre: "data", tipo: "dir" },
      { nombre: "index", tipo: "dir" },
      { nombre: "keys", tipo: "dir" },
      { nombre: "locks", tipo: "dir" },
      { nombre: "snapshots", tipo: "dir" },
      { nombre: "config", tipo: "archivo", bytes: 155, modificado: fecha(400) },
    ];
  return [
    { nombre: "Clientes 2026", tipo: "dir" },
    { nombre: "Facturas", tipo: "dir" },
    { nombre: "Informe mensual septiembre.xlsx", tipo: "archivo", bytes: 284_311, modificado: fecha(2) },
    { nombre: "Contrato arriendo oficina.pdf", tipo: "archivo", bytes: 1_942_007, modificado: fecha(40) },
    { nombre: "Lista de precios.docx", tipo: "archivo", bytes: 88_420, modificado: fecha(5) },
    { nombre: "Logo empresa.png", tipo: "archivo", bytes: 512_993, modificado: fecha(120) },
    { nombre: "Notas reunión.txt", tipo: "archivo", bytes: 3_120, modificado: fecha(1) },
  ];
}

// ---------------------------------------------------------------------------
// Relé de descargas
// ---------------------------------------------------------------------------

async function prepararDescarga(e: EquipoMock, o: OrdenMock, plana: OrdenPlana) {
  const c = plana.cuerpo as { rutas?: string[]; relevo?: { id: string; clave: string } };
  if (!o.relevo || !c.relevo) return;
  const clave = deB64(c.relevo.clave);
  const rutas = c.rutas?.length ? c.rutas : ["Notas reunión.txt"];
  const contenido = (r: string) => utf8(`Archivo restaurado por Resguardo (simulado)\nRuta: ${r}\nEquipo: ${e.nombre}\nVersión: ${String((plana.cuerpo as { version?: string }).version ?? "")}\n\n${"Lorem ipsum dolor sit amet. ".repeat(400)}`);
  const datos = rutas.length === 1 ? contenido(rutas[0]) : zipSinComprimir(rutas.map((r) => ({ nombre: r.split(/[\\/]/).pop() || "archivo", datos: contenido(r) })));
  const rel = estado.relevos.get(o.relevo.id)!;
  // Trozos pequeños en el simulador, para ver la barra de progreso.
  const tam = Math.min(TROZO, Math.max(1024, Math.ceil(datos.length / 5)));
  const total = Math.ceil(datos.length / tam);
  for (let n = 0; n < total; n++) {
    await espera(300);
    rel.trozosDatos.push(cifrarTrozo(clave, o.relevo.id, n, n === total - 1, datos.subarray(n * tam, (n + 1) * tam)));
    rel.trozos = n + 1;
    rel.bytes += Math.min(tam, datos.length - n * tam);
  }
  rel.estado = "listo";
}
