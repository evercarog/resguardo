<script lang="ts">
  // «Mover a otro sitio…» (v1.41): llevar un repositorio, con todo su
  // historial, a otro destino (lo recomendado: el almacén de otro equipo del
  // cliente). Sin órdenes nuevas, en pasos que se ven:
  //  1. (si hace falta) el almacén da acceso al equipo (guarda_copias { anadir },
  //     respuesta sellada solo para este navegador, como «Copiar en …»);
  //  2. se guarda el kit del repositorio nuevo (su contraseña se genera aquí);
  //  3. el equipo crea el repositorio nuevo con el mismo troceado que el actual
  //     (crear_repositorio con parametros_de { repo }), y su misma retención;
  //  4. trae el historial (copiar_historial { origen: { repo } }), con progreso;
  //  5. las copias que guardaban en el anterior pasan al nuevo (config firmada);
  //  6. se trae lo que se copió mientras tanto (otra vez copiar_historial: no
  //     repite nada).
  // Al final, «Dejar de usar el repositorio anterior» (quitar_repositorio, con
  // su espera) y que la carpeta antigua se borra a mano después de comprobar.
  // Lo hecho se recuerda en este navegador: si se cierra o el equipo está
  // apagado, al volver sigue donde se quedó (pidiendo otra vez la clave).
  import { onDestroy, untrack } from "svelte";
  import { ArrowRightLeft, Check, CircleAlert, CircleCheck, CircleDashed, Clock, KeyRound, LoaderCircle, Printer, TriangleAlert } from "@lucide/svelte";
  import Modal from "$ui/componentes/Modal.svelte";
  import * as api from "$lib/api";
  import { argon2Navegador } from "$lib/cripto/argon2";
  import { aB64, aleatorio, borrar, deB64, deUtf8 } from "$lib/cripto/bytes";
  import { pruebaAdmin } from "$lib/cripto/claves";
  import { abrir as abrirSobre, parEfimero } from "$lib/cripto/sobre";
  import { descifrarConfig } from "$lib/cripto/simetrico";
  import { ErrorEtiqueta, ErrorLlavesCambiadas, kcfgComprobada, mandarOrden } from "$lib/ordenar";
  import { actual, cargarCliente } from "$lib/estado.svelte";
  import { avisar } from "$lib/avisos.svelte";
  import { fechaLarga, lista } from "$lib/formato";
  import { destinoDe } from "$lib/repo";
  import { esDeAlmacen, reglaParaOrden } from "$lib/retencion";
  import { lineaLugar, lugarDe, type Lugar } from "$lib/dondeGuarda";
  import type { Cliente, Configuracion, DestinoResumen, Equipo, Orden, RepositorioResumen } from "$lib/tipos";
  import AlertaLlaves from "./AlertaLlaves.svelte";
  import Ayuda from "./Ayuda.svelte";
  import CampoClave from "./CampoClave.svelte";
  import OrdenDialog from "./OrdenDialog.svelte";
  import SeGuardaEn from "./SeGuardaEn.svelte";

  let { cliente, equipo, repo, equipos, onclose }: { cliente: Cliente; equipo: Equipo; repo: RepositorioResumen; equipos: Equipo[]; onclose: () => void } = $props();

  // ---------------------------------------------------------------------------
  // A dónde
  // ---------------------------------------------------------------------------

  interface Opcion {
    clave: string;
    lugar: Lugar;
    /** Un almacén del cliente (recomendado). */
    almacen?: Equipo;
    /** El destino que ya tiene el equipo (si no, hay que pedir acceso al almacén). */
    destino?: DestinoResumen;
    recomendado: boolean;
  }
  const destinos = $derived(equipo.resumen?.destinos ?? []);
  const actualD = $derived(destinoDe(destinos, repo));
  const lugarActual = $derived(lugarDe(actualD, equipo, equipos));
  const opciones = $derived.by((): Opcion[] => {
    const almacenes = equipos.filter((a) => a.id !== equipo.id && a.confirmado && a.modo !== "trasladado" && a.resumen?.guarda_copias?.activo);
    const deAlmacen = almacenes.map((a): Opcion => {
      const d = destinos.find((x) => esDeAlmacen(x, a));
      return { clave: `al:${a.id}`, almacen: a, destino: d, recomendado: true, lugar: lugarDe(d ?? { id: "", nombre: a.nombre, tipo: "rest", equipo_almacen: a.id }, equipo, equipos) };
    });
    const otros = destinos
      .filter((d) => d.id !== actualD?.id && !almacenes.some((a) => esDeAlmacen(d, a)))
      .map((d): Opcion => ({ clave: `de:${d.id}`, destino: d, recomendado: false, lugar: lugarDe(d, equipo, equipos) }));
    return [...deAlmacen.filter((o) => o.destino?.id !== actualD?.id), ...otros];
  });
  let elegida = $state("");
  $effect(() => {
    if (!elegida && opciones.length) elegida = untrack(() => opciones[0].clave);
  });
  const opcion = $derived(opciones.find((o) => o.clave === elegida));
  let nombre = $state(untrack(() => repo.nombre));
  const copiasSuyas = $derived((equipo.resumen?.copias ?? []).filter((k) => k.repo === repo.id));

  // ---------------------------------------------------------------------------
  // Lo recordado (para seguir donde se quedó)
  // ---------------------------------------------------------------------------

  type Fase = "creando" | "creado" | "historial" | "copias" | "ultimo" | "listo";
  interface Plan {
    nuevo: string;
    nombre: string;
    /** La línea «Se guarda en…» del destino nuevo (para enseñarla al volver). */
    donde: string;
    almacen?: string;
    fase: Fase;
    ordenes: Partial<Record<"crear" | "historial" | "copias" | "ultimo", string>>;
  }
  const CLAVE = $derived(`resguardo.mover.${cliente.id}.${equipo.id}.${repo.id}`);
  function leerPlan(): Plan | null {
    try {
      return JSON.parse(localStorage.getItem(CLAVE) ?? "null") as Plan | null;
    } catch {
      return null;
    }
  }
  function guardarPlan(p: Plan | null) {
    plan = p;
    try {
      if (p) localStorage.setItem(CLAVE, JSON.stringify(p));
      else localStorage.removeItem(CLAVE);
    } catch {
      /* sin almacenamiento: solo mientras siga abierta */
    }
  }
  let plan = $state<Plan | null>(untrack(leerPlan));
  // Un plan ya creado cuyo repositorio nuevo ya no está (se quitó): se olvida.
  if (untrack(() => plan && plan.fase !== "creando" && plan.fase !== "listo" && !(equipo.resumen?.repositorios ?? []).some((r) => r.id === plan!.nuevo))) untrack(() => guardarPlan(null));

  // ---------------------------------------------------------------------------
  // Pasos
  // ---------------------------------------------------------------------------

  type IdPaso = "acceso" | "kit" | "crear" | "historial" | "copias" | "ultimo";
  type EstadoPaso = "espera" | "en_marcha" | "esperando_equipo" | "hecho" | "fallo" | "omitido";
  interface Paso {
    id: IdPaso;
    texto: string;
    estado: EstadoPaso;
    detalle?: string;
  }
  const textoCopias = $derived(copiasSuyas.length ? lista(copiasSuyas.map((k) => `«${k.nombre}»`)) : "ninguna copia");
  function pasosIniciales(conAcceso: boolean, alm?: string): Paso[] {
    return [
      ...(conAcceso ? [{ id: "acceso" as const, texto: `${alm ?? "El almacén"} da acceso a ${equipo.nombre}`, estado: "espera" as const }] : []),
      { id: "kit", texto: "Guardar el kit de recuperación del repositorio nuevo", estado: "espera" },
      { id: "crear", texto: `${equipo.nombre} crea el repositorio nuevo (con el mismo troceado)`, estado: "espera" },
      { id: "historial", texto: `Traer todo el historial de «${repo.nombre}»`, estado: "espera" },
      { id: "copias", texto: `Cambiar ${textoCopias} para que guarden en el nuevo`, estado: "espera" },
      { id: "ultimo", texto: "Traer lo copiado mientras tanto", estado: "espera" },
    ];
  }
  let pasos = $state<Paso[]>([]);
  const ponerPaso = (id: IdPaso, estado: EstadoPaso, detalle?: string) => (pasos = pasos.map((p) => (p.id === id ? { ...p, estado, detalle } : p)));

  /** «elegir» → «pasos» (corriendo, o parado esperando la clave / un error) → «listo». */
  let vista = $state<"elegir" | "pasos" | "listo">(untrack(() => (plan ? (plan.fase === "listo" ? "listo" : "pasos") : "elegir")));
  if (untrack(() => plan && plan.fase !== "listo")) {
    const p = untrack(() => plan!);
    const orden: Fase[] = ["creando", "creado", "historial", "copias", "ultimo", "listo"];
    const hecho = (f: Fase) => orden.indexOf(p.fase) >= orden.indexOf(f);
    const de: Partial<Record<IdPaso, Fase>> = { crear: "creado", historial: "historial", copias: "copias", ultimo: "ultimo" };
    pasos = untrack(() => pasosIniciales(false)).map((x) => ({ ...x, estado: x.id === "kit" || hecho(de[x.id] ?? "listo") ? "hecho" : "espera" }));
  }

  // ---------------------------------------------------------------------------
  // Secretos (solo mientras la ventana está abierta)
  // ---------------------------------------------------------------------------

  let claveAdmin = $state("");
  let error = $state("");
  let cambiadas = $state<Equipo | null>(null);
  let corriendo = $state(false);
  let pasoTxt = $state("");
  let kcfg: Uint8Array | null = null;
  let prueba: Uint8Array | null = null;
  let contrasena = $state("");
  let impreso = $state(false);
  let kitListo: (() => void) | null = null;
  let acceso: { donde: string; usuario: string; secreto: string; ca_pem: string } | null = null;
  let vivo = true;
  onDestroy(() => {
    vivo = false;
    borrar(kcfg, prueba);
    kcfg = prueba = null;
    claveAdmin = contrasena = "";
    acceso = null;
    kitListo = null;
  });

  class Cerrada extends Error {}
  const FINALES = ["hecha", "fallida", "rechazada", "cancelada", "caducada"];
  const dormir = (ms: number) => new Promise((r) => setTimeout(r, ms));
  /** El equipo como está ahora (conectado o no), de la lista del cliente. */
  const ahoraDe = (e: Equipo) => actual.equipos.find((x) => x.id === e.id) ?? e;

  /** Espera la respuesta de una orden mientras la ventana siga abierta (sin límite: el equipo puede estar apagado). */
  async function esperar(eq: Equipo, id: string, paso: IdPaso, enMarcha: string): Promise<Orden> {
    while (vivo) {
      try {
        const o = (await api.ordenesEquipo(cliente.id, eq.id, 30)).find((y) => y.id === id);
        if (o && FINALES.includes(o.estado)) return o;
        if (o?.estado === "pendiente" && !ahoraDe(eq).conectado) ponerPaso(paso, "esperando_equipo", `${eq.nombre} no está conectado: la orden le llegará en cuanto se conecte. Puedes cerrar esta ventana y volver luego.`);
        else ponerPaso(paso, "en_marcha", o?.estado === "en_marcha" && o.mensaje ? o.mensaje : enMarcha);
      } catch {
        // Sin red un momento: se vuelve a preguntar.
      }
      await dormir(2000);
    }
    throw new Cerrada();
  }

  /** La clave de administración: K_cfg (para leer las copias) y la prueba del equipo, una sola vez. */
  async function prepararClave() {
    if (kcfg && prueba) return;
    pasoTxt = "Comprobando la clave y las llaves del equipo…";
    kcfg = await kcfgComprobada(cliente, equipo, claveAdmin);
    pasoTxt = "Preparando la autorización…";
    prueba = await pruebaAdmin(argon2Navegador, claveAdmin, equipo.sal_equipo);
    pasoTxt = "";
  }

  function nuevaContrasena() {
    const b = aleatorio(32);
    contrasena = btoa(String.fromCharCode(...b)).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
    b.fill(0);
  }
  const idNuevo = () =>
    `${nombre
      .trim()
      .toLowerCase()
      .normalize("NFD")
      .replace(/[̀-ͯ]/g, "")
      .replace(/[^a-z0-9]+/g, "-")
      .replace(/^-|-$/g, "")
      .slice(0, 36) || "repositorio"}-${crypto.randomUUID().slice(0, 4)}`;

  // Lo que sale en el kit.
  let kitDestino = $state("");
  let kitId = $state("");

  /** Empezar: con la opción elegida. */
  async function empezar(e: SubmitEvent) {
    e.preventDefault();
    const o = opcion;
    if (!o || !nombre.trim()) return;
    const conAcceso = !!o.almacen && !o.destino;
    pasos = pasosIniciales(conAcceso, o.almacen?.nombre);
    vista = "pasos";
    await correr(async () => {
      await prepararClave();
      const id = idNuevo();
      kitId = id;
      // 1. Acceso al almacén (si el equipo aún no copia en él).
      let destinoCuerpo: Record<string, unknown> = { id: o.destino?.id ?? "" };
      if (conAcceso && o.almacen) {
        const alm = o.almacen;
        ponerPaso("acceso", "en_marcha", `Pidiendo a ${alm.nombre} un usuario para ${equipo.nombre}…`);
        const eph = parEfimero();
        try {
          const ord = await mandarOrden({ cliente, equipo: alm, tipo: "guarda_copias", cuerpo: { anadir: equipo.id }, secretos: { claveAdmin }, responderA: aB64(eph.publica), alPaso: (t) => ponerPaso("acceso", "en_marcha", t) });
          const r = await esperar(alm, ord.id, "acceso", `Esperando a ${alm.nombre}…`);
          if (r.estado !== "hecha" || !r.detalle) throw new Error(r.mensaje ?? `${alm.nombre} no pudo dar acceso.`);
          const sellado = (JSON.parse(r.detalle) as { sellado?: string }).sellado;
          if (!sellado) throw new Error("La respuesta no trae el acceso sellado.");
          const a = JSON.parse(deUtf8(abrirSobre(eph.secreta, deB64(sellado)))) as { destino: { donde: string; usuario: string; secreto: string; ca_pem: string } };
          acceso = a.destino;
        } catch (err) {
          ponerPaso("acceso", "fallo", (err as Error).message);
          throw err;
        } finally {
          borrar(eph.secreta);
        }
        ponerPaso("acceso", "hecho", `${alm.nombre} guardará las copias de ${equipo.nombre} con su propio usuario, sin poder borrar lo ya copiado.`);
        destinoCuerpo = {
          id: `almacen-${alm.id.slice(0, 8)}`,
          nombre: alm.nombre,
          tipo: "rest",
          donde: acceso.donde,
          usuario: acceso.usuario,
          secreto: acceso.secreto,
          ca_pem: acceso.ca_pem,
          equipo_almacen: alm.id,
        };
        kitDestino = `${lineaLugar(o.lugar)} (${acceso.donde})`;
      } else {
        kitDestino = `${lineaLugar(o.lugar)}${o.destino?.donde ? ` (${o.destino.donde})` : ""}`;
      }

      // 2. El kit (lo confirma la persona).
      nuevaContrasena();
      impreso = false;
      ponerPaso("kit", "en_marcha", "Imprímelo o guárdalo en PDF: sin esta contraseña nadie podrá leer el repositorio nuevo.");
      await new Promise<void>((res) => (kitListo = res));
      kitListo = null;
      ponerPaso("kit", "hecho");

      // 3. Crear el repositorio nuevo con los parámetros de troceado del actual.
      ponerPaso("crear", "en_marcha", `Enviando a ${equipo.nombre}…`);
      const regla = repo.retencion_regla;
      const crear = await mandarOrden({
        cliente,
        equipo,
        tipo: "crear_repositorio",
        cuerpo: {
          id,
          nombre: nombre.trim(),
          destino: destinoCuerpo,
          contrasena,
          parametros_de: { repo: repo.id },
          ...(regla ? { retencion: reglaParaOrden(regla) } : {}),
        },
        secretos: { prueba: prueba! },
        alPaso: (t) => ponerPaso("crear", "en_marcha", t),
      });
      contrasena = "";
      acceso = null;
      // Desde aquí se recuerda: si se cierra la ventana, al volver sigue.
      guardarPlan({ nuevo: id, nombre: nombre.trim(), donde: lineaLugar(o.lugar), almacen: o.almacen?.id, fase: "creando", ordenes: { crear: crear.id } });
      await restoDePasos();
    });
  }

  /** Seguir donde se quedó (también tras volver a abrir). */
  async function seguir(e?: SubmitEvent) {
    e?.preventDefault();
    await correr(async () => {
      await prepararClave();
      await restoDePasos();
    });
  }

  /** Historial, copias y lo último, según lo que ya esté hecho. */
  async function restoDePasos() {
    if (!plan) return;
    // 3. (Al volver) el repositorio nuevo, si aún se estaba creando.
    if (plan.fase === "creando") {
      const r = await esperar(equipo, plan.ordenes.crear ?? "", "crear", "Creando el repositorio (restic init)…");
      if (r.estado !== "hecha") {
        const m = r.mensaje ?? "El equipo no pudo crear el repositorio.";
        guardarPlan(null);
        ponerPaso("crear", "fallo", m);
        throw new Error(m);
      }
      ponerPaso("crear", "hecho", r.mensaje ?? undefined);
      guardarPlan({ ...plan, fase: "creado" });
      await cargarCliente(cliente.id, { silencioso: true });
    }
    // 4. Traer el historial (si ya se mandó, se espera a esa orden).
    if (plan!.fase === "creado") {
      await traer(plan!, "historial", "Trayendo el historial…");
      guardarPlan({ ...plan!, fase: "historial" });
    }
    // 5. Las copias, al repositorio nuevo.
    if (plan!.fase === "historial") {
      await cambiarCopias(plan!);
      guardarPlan({ ...plan!, fase: "copias" });
    }
    // 6. Lo copiado entretanto (no repite lo ya traído).
    if (plan!.fase === "copias") {
      await traer(plan!, "ultimo", "Trayendo lo copiado mientras tanto…");
      guardarPlan({ ...plan!, fase: "listo" });
    }
    vista = "listo";
    avisar(`«${repo.nombre}» ya guarda en su sitio nuevo.`);
    void cargarCliente(cliente.id, { silencioso: true });
  }

  async function traer(p: Plan, paso: "historial" | "ultimo", enMarcha: string) {
    let id = p.ordenes[paso];
    // Una orden de antes (se cerró la ventana): si sigue o ya terminó, esa; si falló, otra.
    if (id) {
      const o = (await api.ordenesEquipo(cliente.id, equipo.id, 50)).find((x) => x.id === id);
      if (!o || ["fallida", "rechazada", "cancelada", "caducada"].includes(o.estado)) id = undefined;
    }
    if (!id) {
      ponerPaso(paso, "en_marcha", `Enviando a ${equipo.nombre}…`);
      const o = await mandarOrden({
        cliente,
        equipo,
        tipo: "copiar_historial",
        cuerpo: { repo: p.nuevo, origen: { repo: repo.id } },
        secretos: { prueba: prueba! },
        alPaso: (t) => ponerPaso(paso, "en_marcha", t),
      });
      id = o.id;
      guardarPlan({ ...p, ordenes: { ...p.ordenes, [paso]: id } });
    }
    const r = await esperar(equipo, id, paso, enMarcha);
    if (r.estado !== "hecha") {
      ponerPaso(paso, "fallo", r.mensaje ?? "No se pudo traer el historial.");
      throw new Error(r.mensaje ?? "No se pudo traer el historial.");
    }
    ponerPaso(paso, "hecho", r.mensaje ?? undefined);
  }

  async function cambiarCopias(p: Plan) {
    let id = p.ordenes.copias;
    if (id) {
      const o = (await api.ordenesEquipo(cliente.id, equipo.id, 50)).find((x) => x.id === id);
      if (!o || ["fallida", "rechazada", "cancelada", "caducada"].includes(o.estado)) id = undefined;
    }
    if (!id) {
      ponerPaso("copias", "en_marcha", "Leyendo las copias del equipo…");
      let cfg: Configuracion;
      try {
        const cifrada = await api.configEquipo(cliente.id, equipo.id);
        cfg = descifrarConfig<Configuracion>(kcfg!, equipo.id, cifrada.seq, deB64(cifrada.cifrado));
      } catch {
        throw new Error("No se pudo leer la configuración del equipo con esta clave.");
      }
      const cambia = cfg.copias.filter((k) => k.repo === repo.id);
      if (!cambia.length) {
        ponerPaso("copias", "omitido", `Ninguna copia de ${equipo.nombre} guardaba en «${repo.nombre}».`);
        return;
      }
      for (const k of cambia) k.repo = p.nuevo;
      // La verificación automática del anterior, también en el nuevo.
      if (cfg.verificaciones?.[repo.id] && !cfg.verificaciones[p.nuevo]) cfg.verificaciones[p.nuevo] = cfg.verificaciones[repo.id];
      const o = await mandarOrden({ cliente, equipo, tipo: "config", cuerpo: { config: cfg }, secretos: { prueba: prueba! }, alPaso: (t) => ponerPaso("copias", "en_marcha", t) });
      id = o.id;
      guardarPlan({ ...p, ordenes: { ...p.ordenes, copias: id } });
    }
    const r = await esperar(equipo, id, "copias", "Aplicando el cambio en el equipo…");
    if (r.estado !== "hecha") {
      ponerPaso("copias", "fallo", r.mensaje ?? "El equipo no aplicó el cambio.");
      throw new Error(r.mensaje ?? "El equipo no aplicó el cambio.");
    }
    ponerPaso("copias", "hecho", r.mensaje ?? undefined);
  }

  /** Corre un tramo: errores a la vista (y el paso en marcha, en fallo); cerrar la ventana no es un error. */
  async function correr(f: () => Promise<void>) {
    error = "";
    corriendo = true;
    try {
      await f();
    } catch (err) {
      if (err instanceof Cerrada) return;
      if (err instanceof ErrorLlavesCambiadas) cambiadas = equipo;
      error = err instanceof ErrorEtiqueta || err instanceof ErrorLlavesCambiadas ? err.message : (err as Error).message;
      // La clave mal: se vuelve a pedir.
      if (/clave/i.test(error)) {
        borrar(kcfg, prueba);
        kcfg = prueba = null;
      }
      const enMarcha = pasos.find((p) => p.estado === "en_marcha" || p.estado === "esperando_equipo");
      if (enMarcha) ponerPaso(enMarcha.id, "fallo", error);
    } finally {
      corriendo = false;
      pasoTxt = "";
    }
  }

  function empezarDeNuevo() {
    guardarPlan(null);
    pasos = [];
    error = "";
    vista = "elegir";
  }

  // «Dejar de usar el repositorio anterior»: la orden de siempre (contraseña del repositorio, clave y espera).
  let quitar = $state(false);

  const ICONO = { espera: CircleDashed, en_marcha: LoaderCircle, esperando_equipo: Clock, hecho: CircleCheck, fallo: CircleAlert, omitido: CircleCheck };
  const TONO = { espera: "neutral", en_marcha: "info", esperando_equipo: "paused", hecho: "ok", fallo: "bad", omitido: "neutral" };
  const TEXTO_ESTADO = { espera: "Pendiente", en_marcha: "En marcha", esperando_equipo: "Esperando al equipo", hecho: "Hecho", fallo: "Falló", omitido: "No hacía falta" };
  const desconectado = $derived(!ahoraDe(equipo).conectado);
  const pasoKit = $derived(pasos.find((p) => p.id === "kit")?.estado === "en_marcha");
  const necesitaClave = $derived(vista === "pasos" && !corriendo && !pasoKit);
</script>

{#if quitar}
  <OrdenDialog
    {cliente}
    {equipo}
    tipo="quitar_repositorio"
    cuerpo={{ repo: repo.id }}
    titulo="Dejar de usar el repositorio anterior"
    descripcion={`${equipo.nombre} olvidará «${repo.nombre}» (${lugarActual.texto.toLowerCase()}). Lo guardado sigue en su carpeta: bórrala a mano cuando hayas comprobado que el nuevo tiene todo.`}
    repo={{ id: repo.id, nombre: repo.nombre }}
    onclose={() => ((quitar = false), onclose())}
  />
{:else}
<Modal labelledby="t-mover" {onclose} width={620} dismissible={false}>
  <div class="dlg-title">
    <span class="ticon"><ArrowRightLeft size={18} /></span>
    <div>
      <h2 id="t-mover">Mover «{repo.nombre}» a otro sitio</h2>
      <p>Se crea un repositorio nuevo en el sitio que elijas, con todo el historial de este, y las copias pasan a guardar allí. El repositorio actual no se toca hasta que tú decidas dejar de usarlo.</p>
    </div>
  </div>

  {#if cambiadas}
    <AlertaLlaves equipo={cambiadas} cliente={cliente.id} />
    <footer><button class="btn btn-primary" onclick={onclose}>Entendido</button></footer>
  {:else if vista === "elegir"}
    <form class="form" onsubmit={empezar}>
      <div class="ahora">
        <span class="k">Ahora</span>
        <SeGuardaEn lugar={lugarActual} riesgo={lugarActual.clase === "carpeta" || lugarActual.clase === "almacen_propio"} />
      </div>
      {#if !opciones.length}
        <div class="notice notice-info"><TriangleAlert size={16} /><p>{equipo.nombre} no tiene otro destino y {cliente.nombre} no tiene ningún almacén en otro equipo. Activa «Este equipo guarda copias» en otro equipo (p. ej. un PC dedicado a copias) y vuelve aquí.</p></div>
        <footer><button type="button" class="btn btn-primary" onclick={onclose}>Entendido</button></footer>
      {:else}
        <fieldset class="opciones">
          <legend class="field-label">¿A dónde?</legend>
          {#each opciones as o (o.clave)}
            <label class="opcion" class:on={elegida === o.clave}>
              <input type="radio" name="mover-a" value={o.clave} bind:group={elegida} />
              <span class="o-txt">
                <SeGuardaEn lugar={o.lugar} etiqueta="" riesgo={o.lugar.mismoEquipo && o.lugar.clase !== "usb"} />
                {#if o.recomendado}<span class="badge badge-sm tone-ok">Recomendado</span>{/if}
                {#if o.almacen && !o.destino}<span class="faint pequeno">{o.almacen.nombre} dará a {equipo.nombre} su propio usuario, sin poder borrar lo ya copiado.</span>{/if}
                {#if o.almacen && !ahoraDe(o.almacen).conectado}<span class="faint pequeno aviso-txt">{o.almacen.nombre} no está conectado ahora: tendrá que encenderse para dar el acceso.</span>{/if}
                {#if o.lugar.mismoEquipo && o.lugar.clase !== "usb"}<span class="faint pequeno aviso-txt">Sigue en {equipo.nombre}: no protege si se daña ese equipo.</span>{/if}
              </span>
            </label>
          {/each}
        </fieldset>
        <div class="field">
          <label class="field-label" for="m-nombre">Nombre del repositorio nuevo</label>
          <input id="m-nombre" class="input" bind:value={nombre} />
        </div>
        {#if desconectado}
          <div class="notice notice-warn"><Clock size={16} /><p>{equipo.nombre} no está conectado ahora. Las órdenes le esperarán: puedes empezar, cerrar esta ventana y volver cuando se conecte.</p></div>
        {/if}
        {#if repo.externa}<p class="faint pequeno">La copia externa de «{repo.nombre}» no se mueve: configúrala después en el nuevo si la quieres.</p>{/if}
        <CampoClave requerido id="m-admin" etiqueta="Clave de administración" bind:value={claveAdmin} error={error && /clave/i.test(error) ? error : ""}>
          {#snippet extra()}<Ayuda id="clave-admin" />{/snippet}
        </CampoClave>
        <footer>
          <button type="button" class="btn btn-ghost" onclick={onclose}>Cancelar</button>
          <button class="btn btn-primary" disabled={!opcion || !nombre.trim() || !claveAdmin}><KeyRound size={15} />Empezar</button>
        </footer>
      {/if}
    </form>
  {:else}
    <div class="form">
      {#if plan}
        <div class="ahora">
          <span class="k">Del</span><SeGuardaEn lugar={lugarActual} etiqueta="" />
          <span class="k">Al nuevo</span><span class="nuevo-donde"><strong>«{plan.nombre}»</strong> · {plan.donde}</span>
        </div>
      {/if}
      <ol class="pasos" aria-label="Pasos">
        {#each pasos as p (p.id)}
          {@const Ic = ICONO[p.estado]}
          <li class="paso tone-{TONO[p.estado]}" aria-current={p.estado === "en_marcha" ? "step" : undefined}>
            <span class="p-ic" aria-hidden="true"><Ic size={16} class={p.estado === "en_marcha" ? "spin" : ""} /></span>
            <div class="p-txt">
              <span class="p-t">{p.texto} <span class="sr-only">({TEXTO_ESTADO[p.estado]})</span><span class="p-est">{TEXTO_ESTADO[p.estado]}</span></span>
              {#if p.detalle}<span class="p-d">{p.detalle}</span>{/if}
              {#if p.id === "kit" && p.estado === "en_marcha"}
                <article class="kit" id="kit-imprimible">
                  <h3>Kit de recuperación · Resguardo</h3>
                  <dl>
                    <dt>Cliente</dt><dd>{cliente.nombre}</dd>
                    <dt>Equipo</dt><dd>{equipo.nombre}</dd>
                    <dt>Repositorio</dt><dd>{nombre} · <span class="pastilla mono selectable">{kitId}</span></dd>
                    <dt>Destino</dt><dd>{kitDestino}</dd>
                    <dt>Contraseña</dt><dd><code class="selectable pw">{contrasena}</code></dd>
                    <dt>Creado</dt><dd>{fechaLarga(new Date().toISOString())}</dd>
                  </dl>
                  <p class="faint">Guárdalo fuera del equipo. Con esta contraseña y el destino se pueden restaurar las copias con restic, incluso sin Resguardo.</p>
                </article>
                <div class="kit-acc">
                  <button type="button" class="btn btn-sm" onclick={() => window.print()}><Printer size={14} />Imprimir o guardar en PDF</button>
                  <label class="switch-row"><input type="checkbox" bind:checked={impreso} /><span>He guardado el kit en un sitio seguro</span></label>
                  <button type="button" class="btn btn-sm btn-primary" disabled={!impreso} onclick={() => kitListo?.()}><Check size={14} />Seguir</button>
                </div>
              {/if}
            </div>
          </li>
        {/each}
      </ol>

      {#if vista === "listo"}
        <div class="notice notice-success" role="status">
          <CircleCheck size={16} />
          <div>
            <p><strong>Hecho.</strong> Las copias de {equipo.nombre} ya guardan en «{plan?.nombre ?? nombre}» ({plan?.donde}), con todo el historial de antes.</p>
            <p class="pequeno">Cuando compruebes que el nuevo tiene todas las versiones (en su página), deja de usar el anterior. Su carpeta ({lugarActual.texto.toLowerCase()}{lugarActual.detalle ? `, «${lugarActual.detalle}»` : ""}) no se borra sola: bórrala a mano después, si quieres recuperar el espacio.</p>
          </div>
        </div>
        <footer>
          <button type="button" class="btn btn-ghost" onclick={() => (guardarPlan(null), onclose())}>Cerrar</button>
          {#if plan?.nuevo}<a class="btn" href="/c/{cliente.id}/equipos/{equipo.id}/repositorios/{encodeURIComponent(plan.nuevo)}" onclick={() => guardarPlan(null)}>Ver el repositorio nuevo</a>{/if}
          <button type="button" class="btn btn-danger" onclick={() => (guardarPlan(null), (quitar = true))}>Dejar de usar el repositorio anterior…</button>
        </footer>
      {:else}
        {#if error && !/clave/i.test(error)}<div class="notice notice-danger" role="alert"><TriangleAlert size={16} /><p>{error}</p></div>{/if}
        {#if necesitaClave}
          <form class="form" onsubmit={seguir}>
            <p class="faint pequeno">{plan ? "Para seguir donde se quedó, escribe otra vez la clave de administración." : "Escribe la clave de administración para volver a intentarlo."}</p>
            {#if plan}
              <CampoClave requerido id="m-admin2" etiqueta="Clave de administración" bind:value={claveAdmin} error={error && /clave/i.test(error) ? error : ""}>
                {#snippet extra()}<Ayuda id="clave-admin" />{/snippet}
              </CampoClave>
            {/if}
            <footer>
              <button type="button" class="btn btn-ghost" onclick={onclose}>Cerrar</button>
              {#if plan}
                <button class="btn btn-primary" disabled={!claveAdmin}><KeyRound size={15} />Seguir</button>
              {:else}
                <button type="button" class="btn btn-primary" onclick={empezarDeNuevo}>Volver a empezar</button>
              {/if}
            </footer>
          </form>
        {:else if !pasoKit}
          <footer>
            {#if pasoTxt}<span class="espera" role="status"><LoaderCircle size={15} class="spin" />{pasoTxt}</span>{/if}
            <span class="faint pequeno hueco">Puedes cerrar esta ventana: lo que esté en marcha sigue en el equipo, y al volver aquí sigue donde se quedó.</span>
            <button type="button" class="btn btn-ghost" onclick={onclose}>Cerrar</button>
          </footer>
        {/if}
      {/if}
    </div>
  {/if}
</Modal>
{/if}

<style>
  .ahora {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr);
    align-items: baseline;
    gap: 6px 12px;
    padding: var(--sp-3) var(--sp-4);
    background: var(--surface-2);
    border-radius: var(--radius);
  }
  .k {
    font-size: var(--fs-xs);
    font-weight: 500;
    color: var(--text-3);
  }
  .nuevo-donde {
    font-size: var(--fs-sm);
    color: var(--text-2);
    overflow-wrap: anywhere;
  }
  .opciones {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin: 0;
    padding: 0;
    border: none;
  }
  .opcion {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    padding: 10px 12px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    cursor: pointer;
  }
  .opcion.on {
    border-color: var(--accent);
    background: var(--accent-soft);
  }
  .opcion input {
    margin-top: 4px;
  }
  .o-txt {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 4px 8px;
    min-width: 0;
  }
  .pequeno {
    margin: 0;
    font-size: var(--fs-xs);
  }
  .o-txt .pequeno {
    flex-basis: 100%;
  }
  .aviso-txt {
    color: var(--warn);
  }
  .pasos {
    display: flex;
    flex-direction: column;
    gap: 0;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .paso {
    display: flex;
    gap: 10px;
    padding: 10px 0;
    border-top: 1px solid var(--border);
  }
  .paso:first-child {
    border-top: none;
  }
  .p-ic {
    display: inline-flex;
    flex: none;
    margin-top: 1px;
    color: var(--tone, var(--text-3));
  }
  .tone-neutral .p-ic {
    color: var(--text-3);
  }
  .p-txt {
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-width: 0;
    flex: 1;
  }
  .p-t {
    display: flex;
    flex-wrap: wrap;
    justify-content: space-between;
    gap: 2px 10px;
    font-size: var(--fs-sm);
    font-weight: 550;
  }
  .p-est {
    font-size: var(--fs-xs);
    font-weight: 500;
    color: var(--tone, var(--text-3));
  }
  .tone-neutral .p-est {
    color: var(--text-3);
  }
  .p-d {
    font-size: var(--fs-xs);
    color: var(--text-2);
    overflow-wrap: anywhere;
  }
  .kit {
    margin-top: 6px;
    padding: var(--sp-4);
    border: 1px dashed var(--border-strong);
    border-radius: var(--radius);
  }
  .kit h3 {
    margin: 0 0 var(--sp-3);
    font-size: var(--fs-body);
  }
  .kit dl {
    display: grid;
    grid-template-columns: 96px minmax(0, 1fr);
    gap: 4px 12px;
    margin: 0 0 var(--sp-3);
    font-size: var(--fs-sm);
  }
  .kit dt {
    color: var(--text-3);
  }
  .kit dd {
    margin: 0;
    overflow-wrap: anywhere;
  }
  .kit p {
    margin: 0;
    font-size: var(--fs-xs);
  }
  .pw {
    font-size: 13px;
    word-break: break-all;
  }
  .kit-acc {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px 12px;
    margin-top: 6px;
  }
  .espera {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  .hueco {
    margin-right: auto;
  }
  @media print {
    :global(body *) {
      visibility: hidden;
    }
    :global(#kit-imprimible),
    :global(#kit-imprimible *) {
      visibility: visible;
    }
    :global(#kit-imprimible) {
      position: fixed;
      inset: 0 auto auto 0;
      width: 100%;
      border: none;
      color: #000;
    }
  }
</style>
