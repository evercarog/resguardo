<script lang="ts">
  // «Ordenar nombres de carpeta» (plan 0.8.0, B1; docs/nombres-legibles.md).
  // Con la clave de administración, de forma coordinada:
  // 1) el almacén mueve en su disco la carpeta del equipo y las de sus repositorios
  //    (`guarda_copias { mover }`) y pone al día su retención y sus espejos;
  // 2) el equipo dueño (y quien lo tenga importado) pone la ruta nueva
  //    (`cambiar_destino { donde, usuario, carpetas }`), abriendo antes cada repositorio.
  // Si 2 falla, se dice qué falta, con «Reintentar» y «Deshacer en el almacén».
  // En el destino de cada espejo no se mueve nada: la carpeta nueva se sube entera.
  import { onDestroy } from "svelte";
  import { Check, FolderTree, KeyRound, LoaderCircle, RotateCcw, TriangleAlert } from "@lucide/svelte";
  import Modal from "$ui/componentes/Modal.svelte";
  import * as api from "$lib/api";
  import { ErrorLlavesCambiadas, mandarOrden } from "$lib/ordenar";
  import { avisar } from "$lib/avisos.svelte";
  import { cargarCliente } from "$lib/estado.svelte";
  import { bytes } from "$lib/formato";
  import type { Cliente, Equipo, Orden } from "$lib/tipos";
  import {
    admiteNombres,
    cuerpoMover,
    cuerpoRutaNueva,
    equiposEnAlmacen,
    espejosQueLoCopian,
    normalizar,
    MAX_EQUIPO,
    planOrden,
    textoCarpetaVieja,
    type EnElAlmacen,
    type Movido,
  } from "$lib/nombres";
  import CampoClave from "./CampoClave.svelte";
  import AlertaLlaves from "./AlertaLlaves.svelte";
  import BotonFalta from "./BotonFalta.svelte";
  import Ayuda from "./Ayuda.svelte";

  let {
    cliente,
    almacen,
    equipos,
    equipoInicial,
    soloRepo,
    onclose,
  }: {
    cliente: Cliente;
    almacen: Equipo;
    equipos: Equipo[];
    /** El equipo (dueño) que se ordena; sin él, se elige. */
    equipoInicial?: string;
    /** Desde la página de un repositorio: solo ese (y, si se marca, la carpeta del equipo). */
    soloRepo?: string;
    onclose: () => void;
  } = $props();

  // Lo de cada equipo dueño en el almacén (al abrir: después manda lo que se elija).
  const alAbrir = () => equiposEnAlmacen(almacen, equipos);
  const todos = alAbrir();
  const duenos = todos.filter((x) => !x.importado);
  const conAlgo = duenos.filter((x) => {
    const p = planOrden(x);
    return p.usuario || p.repos.length;
  });
  const inicial = () => (equipoInicial && duenos.some((x) => x.equipo.id === equipoInicial) ? equipoInicial : (conAlgo[0]?.equipo.id ?? duenos[0]?.equipo.id ?? ""));
  let elegido = $state(inicial());
  const en = $derived<EnElAlmacen | undefined>(duenos.find((x) => x.equipo.id === elegido));
  const plan = $derived(en ? planOrden(en) : null);
  // Qué se mueve: la carpeta del equipo y cada repositorio, marcables.
  let conUsuario = $state(true);
  let quitados = $state<string[]>([]);
  const repos = $derived((plan?.repos ?? []).filter((r) => (!soloRepo || r.repo.id === soloRepo) && !quitados.includes(r.de)));
  const moverUsuario = $derived(!!plan?.usuario && conUsuario);
  /** Los que lo tienen importado (mismo usuario): también ponen la ruta nueva. */
  const importadores = $derived(en ? todos.filter((x) => x.importado && x.usuario === en.usuario) : []);
  const espejos = $derived(en ? espejosQueLoCopian(almacen, en.usuario, moverUsuario ? en.repos.map((r) => r.carpeta) : repos.map((r) => r.de)) : []);
  /** Lo que se volverá a subir (aprox.: lo protegido de lo que se mueve). */
  const tamano = $derived(en ? (moverUsuario ? en.repos : en.repos.filter((r) => repos.some((x) => x.de === r.carpeta))).reduce((s, r) => s + (r.repo.solo_lectura ? 0 : (r.repo.bytes ?? 0)), 0) : 0);

  let claveAdmin = $state("");
  let fase = $state<"pedir" | "almacen" | "equipo" | "fallo_equipo" | "listo">("pedir");
  let pasoTxt = $state("");
  let error = $state("");
  let cambiadas = $state<Equipo | null>(null);
  let movido = $state<Movido | null>(null);
  /** Equipos a los que aún les falta la ruta nueva (tras un fallo). */
  let faltan = $state<string[]>([]);
  onDestroy(() => (claveAdmin = ""));

  // Lo que hace falta, dicho por el botón (14aa).
  const falta = $derived(
    !en
      ? { texto: "Elige un equipo" }
      : !admiteNombres(almacen)
        ? { texto: `Actualiza antes el agente de ${almacen.nombre}` }
        : !moverUsuario && !repos.length
          ? { texto: "No hay nada que ordenar" }
          : repos.length && !admiteNombres(en.equipo)
            ? { texto: `Actualiza antes el agente de ${en.equipo.nombre} (o mueve solo la carpeta del equipo)` }
            : !en.equipo.conectado || !almacen.conectado
              ? { texto: `${!almacen.conectado ? almacen.nombre : en.equipo.nombre} no está conectado ahora` }
              : !claveAdmin
                ? { texto: "Falta la clave de administración", campo: "o-clave" }
                : null,
  );

  const FINALES = ["hecha", "fallida", "rechazada", "cancelada", "caducada"];
  /** Espera la respuesta firmada de una orden (como mucho unos 3 min: el almacén espera a que su servidor lea el usuario nuevo). */
  async function respuesta(eq: Equipo, o: Orden): Promise<Orden> {
    for (let i = 0; i < 120; i++) {
      const x = (await api.ordenesEquipo(cliente.id, eq.id, 10)).find((y) => y.id === o.id);
      if (x && FINALES.includes(x.estado)) return x;
      await new Promise((r) => setTimeout(r, 1500));
    }
    throw new Error(`${eq.nombre} no ha respondido todavía. Mira sus órdenes en un momento.`);
  }

  async function ordenEsperada(eq: Equipo, tipo: string, cuerpo: Record<string, unknown>): Promise<Orden> {
    const o = await mandarOrden({ cliente, equipo: eq, tipo, cuerpo, secretos: { claveAdmin }, alPaso: (t) => (pasoTxt = t) });
    pasoTxt = `Esperando a ${eq.nombre}…`;
    const r = await respuesta(eq, o);
    if (r.estado !== "hecha") throw new Error(r.mensaje ?? `${eq.nombre} no pudo hacerlo.`);
    return r;
  }

  /** Paso 2: la ruta nueva en el dueño y en quien lo tenga importado. */
  async function rutaNueva(m: Movido, solo?: string[]) {
    fase = "equipo";
    const mal: string[] = [];
    let ultimoError = "";
    for (const x of [en!, ...importadores]) {
      if (solo && !solo.includes(x.equipo.id)) continue;
      const cuerpo = cuerpoRutaNueva(x, m);
      if (!cuerpo) continue;
      try {
        pasoTxt = `${x.equipo.nombre} pone la ruta nueva…`;
        await ordenEsperada(x.equipo, "cambiar_destino", cuerpo);
      } catch (e) {
        if (e instanceof ErrorLlavesCambiadas) cambiadas = x.equipo;
        mal.push(x.equipo.id);
        ultimoError = (e as Error).message;
      }
    }
    faltan = mal;
    if (mal.length) {
      fase = "fallo_equipo";
      error = ultimoError;
      return;
    }
    fase = "listo";
    avisar("Carpetas ordenadas: el almacén y el equipo ya usan los nombres nuevos.");
    void cargarCliente(cliente.id, { silencioso: true });
  }

  async function empezar(e: SubmitEvent) {
    e.preventDefault();
    if (!en || falta) return;
    error = "";
    fase = "almacen";
    try {
      const cuerpo = cuerpoMover(en, { usuario: moverUsuario, repos });
      const r = await ordenEsperada(almacen, "guarda_copias", cuerpo);
      const m = (JSON.parse(r.detalle ?? "{}") as { mover?: Movido }).mover;
      if (!m) throw new Error(`${almacen.nombre} no dijo qué movió (¿un agente anterior?). Revisa sus órdenes.`);
      movido = m;
      await rutaNueva(m);
    } catch (err) {
      if (err instanceof ErrorLlavesCambiadas) cambiadas = almacen;
      error = (err as Error).message;
      if (fase === "almacen") fase = "pedir";
    } finally {
      pasoTxt = "";
    }
  }

  async function reintentar() {
    if (!movido) return;
    error = "";
    try {
      await rutaNueva(movido, faltan);
    } finally {
      pasoTxt = "";
    }
  }

  /** Vuelve a los nombres de antes en el almacén (la misma orden con `deshacer`). */
  async function deshacer() {
    if (!movido || !en) return;
    error = "";
    fase = "almacen";
    try {
      const usuario = movido.usuario?.a ?? en.usuario;
      await ordenEsperada(almacen, "guarda_copias", { mover: { usuario, deshacer: true } });
      // Los que ya pusieron la ruta nueva vuelven a la de antes.
      const vuelta: Movido = { usuario: movido.usuario ? { de: movido.usuario.a, a: movido.usuario.de } : null, repos: movido.repos.map((r) => ({ de: r.a, a: r.de })) };
      for (const x of [en, ...importadores].filter((x) => !faltan.includes(x.equipo.id))) {
        const enAhora = { ...x, usuario: movido.usuario?.a ?? x.usuario, repos: x.repos.map((r) => ({ ...r, carpeta: movido!.repos.find((y) => y.de === r.carpeta)?.a ?? r.carpeta })) };
        const c = cuerpoRutaNueva(enAhora, vuelta);
        if (c) await ordenEsperada(x.equipo, "cambiar_destino", c);
      }
      avisar("Deshecho: las carpetas vuelven a tener los nombres de antes.");
      void cargarCliente(cliente.id, { silencioso: true });
      onclose();
    } catch (err) {
      error = (err as Error).message;
      fase = "fallo_equipo";
    } finally {
      pasoTxt = "";
    }
  }

  const ocupado = $derived(fase === "almacen" || fase === "equipo");
  const nombreEquipo = (id: string) => [en, ...importadores].find((x) => x?.equipo.id === id)?.equipo.nombre ?? "un equipo";
</script>

<Modal labelledby="t-ordenar" {onclose} width={620} dismissible={!ocupado}>
  <div class="dlg-title">
    <span class="ticon"><FolderTree size={18} /></span>
    <div>
      <h2 id="t-ordenar">Ordenar nombres de carpeta en {almacen.nombre}</h2>
      <p>Las carpetas pasan a llevar el nombre del equipo y de cada repositorio. Se mueven en el mismo disco del almacén, sin copiar nada, y el equipo usa la ruta nueva enseguida. <Ayuda id="guarda-copias" /></p>
    </div>
  </div>

  {#if cambiadas}
    <AlertaLlaves equipo={cambiadas} cliente={cliente.id} />
    <footer><button class="btn btn-primary" onclick={onclose}>Entendido</button></footer>
  {:else if fase === "listo"}
    <p><Check size={15} /> Listo: {en?.equipo.nombre} ya guarda en <code>{movido?.usuario?.a ?? en?.usuario}</code>{movido?.repos.length ? `, con ${movido.repos.map((r) => r.a).join(", ")}` : ""}.</p>
    {#if espejos.length}<p class="faint">Los espejos subirán la carpeta nueva la próxima vez que copien.</p>{/if}
    <footer><button class="btn btn-primary" onclick={onclose}>Cerrar</button></footer>
  {:else if fase === "fallo_equipo"}
    <div class="notice notice-danger" role="alert">
      <TriangleAlert size={16} />
      <p>
        {almacen.nombre} ya movió las carpetas, pero falta que {faltan.map(nombreEquipo).join(" y ")} use la ruta nueva: hasta entonces no puede copiar allí.
        {#if error}<br /><span class="faint">{error}</span>{/if}
      </p>
    </div>
    <footer>
      {#if ocupado}<span class="espera" role="status"><LoaderCircle size={15} class="spin" />{pasoTxt}</span>{/if}
      <button type="button" class="btn btn-ghost" disabled={ocupado} onclick={deshacer}><RotateCcw size={15} />Deshacer en el almacén</button>
      <button type="button" class="btn btn-primary" disabled={ocupado} onclick={reintentar}>Reintentar en {faltan.map(nombreEquipo).join(" y ")}</button>
    </footer>
  {:else}
    <form class="form" onsubmit={empezar}>
      {#if !duenos.length}
        <p class="faint">Ningún equipo de {cliente.nombre} guarda todavía en {almacen.nombre}.</p>
      {:else if !soloRepo || !equipoInicial}
        <div class="field">
          <label class="field-label" for="o-equipo">Equipo</label>
          <select id="o-equipo" class="input" bind:value={elegido} disabled={ocupado}>
            {#each duenos as x (x.equipo.id + x.destino.id)}<option value={x.equipo.id}>{x.equipo.nombre}{conAlgo.includes(x) ? "" : " (ya en orden)"}</option>{/each}
          </select>
        </div>
      {/if}
      {#if en && plan}
        <ul class="lista">
          {#if plan.usuario}
            <li>
              <label class="switch-row"><input type="checkbox" bind:checked={conUsuario} disabled={ocupado} /><span>Carpeta del equipo</span></label>
              <span class="ruta"><code>{en.usuario}</code> → <code>{normalizar(en.equipo.nombre, MAX_EQUIPO, "equipo")}-····</code></span>
            </li>
          {/if}
          {#each plan.repos.filter((r) => !soloRepo || r.repo.id === soloRepo) as r (r.de)}
            <li>
              <label class="switch-row"
                ><input type="checkbox" checked={!quitados.includes(r.de)} disabled={ocupado} onchange={(ev) => (quitados = (ev.currentTarget as HTMLInputElement).checked ? quitados.filter((q) => q !== r.de) : [...quitados, r.de])} /><span
                  >{r.nombre}</span
                ></label
              >
              <span class="ruta"><code>{r.de}</code> → <code>{r.propuesta}</code></span>
            </li>
          {/each}
          {#if !plan.usuario && !plan.repos.length}<li class="faint">Las carpetas de {en.equipo.nombre} ya tienen nombres legibles.</li>{/if}
        </ul>
        <p class="faint pequeno">El sufijo (····) lo pone el almacén para que sea único. El nombre que se ve de cada repositorio no cambia.</p>
        {#if importadores.length}<p class="faint pequeno">{importadores.map((x) => x.equipo.nombre).join(", ")} {importadores.length === 1 ? "lo tiene" : "lo tienen"} importado: también usará la ruta nueva.</p>{/if}
        {#if espejos.length && (moverUsuario || repos.length)}
          <div class="notice notice-info">
            <div>
            <p>
              <strong>Espejos.</strong> En su destino no se mueve nada: la carpeta nueva se copia entera la próxima vez que copie{tamano ? ` (unos ${bytes(tamano)})` : ""}.
            </p>
            <ul>
              {#each espejos as t (t.id)}<li>«{t.nombre}»{t.adonde.tipo === "nube" && t.adonde.nube ? ` (a ${t.adonde.nube})` : ""}: {textoCarpetaVieja(t)}.</li>{/each}
            </ul>
            </div>
          </div>
        {/if}
        <p class="faint pequeno">Mejor cuando no esté copiando: si hay una copia en marcha, el almacén no mueve nada y te lo dice.</p>
      {/if}
      <CampoClave requerido id="o-clave" etiqueta="Clave de administración de {cliente.nombre}" bind:value={claveAdmin} ayuda="Para las dos órdenes: mover en el almacén y la ruta nueva en el equipo. El servidor nunca la ve." />
      {#if error}<div class="notice notice-danger" role="alert"><TriangleAlert size={16} /><p>{error}</p></div>{/if}
      <footer>
        {#if ocupado}<span class="espera" role="status"><LoaderCircle size={15} class="spin" />{pasoTxt || (fase === "almacen" ? `${almacen.nombre} mueve las carpetas…` : "")}</span>{/if}
        <button type="button" class="btn btn-ghost" disabled={ocupado} onclick={onclose}>Cancelar</button>
        <BotonFalta disabled={ocupado} {falta}><KeyRound size={15} />Ordenar con la clave de administración</BotonFalta>
      </footer>
    </form>
  {/if}
</Modal>

<style>
  .espera {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    margin-right: auto;
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  .lista {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .lista li {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: 4px var(--sp-3);
    padding: var(--sp-2) var(--sp-3);
    border: 1px solid var(--border);
    border-radius: var(--radius);
  }
  .ruta {
    font-size: var(--fs-sm);
    overflow-wrap: anywhere;
  }
  .pequeno {
    margin: 0;
    font-size: var(--fs-xs);
  }
  .notice ul {
    margin: 4px 0 0;
    padding-left: 1.2em;
    font-size: var(--fs-sm);
  }
</style>
