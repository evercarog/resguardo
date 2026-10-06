<script lang="ts">
  // «Versiones» del cliente (docs/actualizaciones.md §9): la versión del agente que da
  // este servidor, cómo se actualizan los equipos (automática por anillos, solo cuando
  // apruebes o en pausa, con su ventana), «Actualizar ahora» y el estado de cada equipo.
  // El estado lo cuenta cada agente en su informe; aquí solo se pone en palabras.
  import { untrack } from "svelte";
  import { CircleArrowUp, CirclePause, CirclePlay, PackageCheck, ShieldCheck, TriangleAlert } from "@lucide/svelte";
  import * as api from "$lib/api";
  import { actual, cargarCliente, puede } from "$lib/estado.svelte";
  import { avisar, fallo } from "$lib/avisos.svelte";
  import { cargarInformes, recargarInformes, ultimos } from "$lib/informes.svelte";
  import { fechaLarga, plural } from "$lib/formato";
  import { compararVersiones, frasePolitica, horaValida, seActualizaSolo, vistaActualizacion, type EstadoActualizacion } from "$lib/actualizaciones";
  import type { ActualizacionesCliente, PoliticaActualizaciones } from "$lib/tipos";
  import BotonCargando from "$lib/componentes/BotonCargando.svelte";
  import CabeceraPagina from "$lib/componentes/CabeceraPagina.svelte";
  import Cargando from "$lib/componentes/Cargando.svelte";
  import Chip from "$lib/componentes/Chip.svelte";
  import Tiempo from "$lib/componentes/Tiempo.svelte";

  /** undefined: cargando; null: un servidor anterior (sin esta función). */
  let datos = $state<ActualizacionesCliente | null | undefined>(undefined);
  let modo = $state<PoliticaActualizaciones["modo"]>("auto");
  let dias = $state(2);
  let conVentana = $state(false);
  let desde = $state("22:00");
  let hasta = $state("06:00");
  let guardando = $state(false);
  let aprobando = $state("");
  let cambiandoAnillo = $state("");

  const rol = $derived(actual.cliente?.rol);
  const admin = $derived(puede.administrar(rol));
  const disponible = $derived(datos?.disponible?.version ?? null);
  const equipos = $derived(actual.equipos.filter((e) => e.confirmado && e.modo !== "trasladado"));

  async function cargar() {
    try {
      const r = await api.actualizaciones(actual.id);
      datos = r;
      if (r) {
        modo = r.politica.modo;
        dias = r.politica.dias_general;
        conVentana = !!r.politica.ventana;
        if (r.politica.ventana) [desde, hasta] = [r.politica.ventana.desde, r.politica.ventana.hasta];
      }
    } catch (e) {
      fallo(e);
      datos = null;
    }
  }

  const clienteListo = $derived(actual.cliente ? actual.id : "");
  $effect(() => {
    if (!clienteListo) return;
    untrack(() => {
      void cargar();
      void cargarInformes(actual.id, actual.equipos.map((e) => e.id));
    });
  });

  const informeDe = (id: string) => (ultimos.cliente === actual.id ? ultimos.porEquipo[id] : null);
  const estadoDe = (id: string) => informeDe(id)?.datos.actualizacion as EstadoActualizacion | undefined;
  const filas = $derived(
    equipos
      .map((e) => {
        const v = vistaActualizacion(estadoDe(e.id), informeDe(e.id)?.datos.version ?? e.version_agente, disponible);
        return { e, v, anillo: datos?.equipos[e.id]?.anillo ?? "general", version: informeDe(e.id)?.datos.version ?? e.version_agente };
      })
      .sort((a, b) => a.e.nombre.localeCompare(b.e.nombre)),
  );
  const cuenta = (clave: string) => filas.filter((f) => f.v.clave === clave).length;
  const alDia = $derived(filas.filter((f) => f.v.clave === "al_dia").length);
  const problemas = $derived(filas.filter((f) => f.v.tono === "bad").length);
  const cambiado = $derived(
    !!datos &&
      (modo !== datos.politica.modo ||
        dias !== datos.politica.dias_general ||
        conVentana !== !!datos.politica.ventana ||
        (conVentana && (desde !== datos.politica.ventana?.desde || hasta !== datos.politica.ventana?.hasta))),
  );
  const ventanaMala = $derived(conVentana && (!horaValida(desde) || !horaValida(hasta)));

  async function guardar(nuevo?: PoliticaActualizaciones["modo"]) {
    if (ventanaMala) return;
    guardando = true;
    try {
      const r = await api.cambiarPoliticaActualizaciones(actual.id, { modo: nuevo ?? modo, dias_general: Math.max(0, Math.min(30, Math.round(dias))), ventana: conVentana ? { desde, hasta } : null });
      if (datos) datos.politica = r.politica;
      modo = r.politica.modo;
      avisar(r.politica.modo === "pausada" ? "Actualizaciones en pausa: ningún equipo se actualiza hasta que las reanudes." : "Guardado. Los equipos conectados lo ven ya.");
    } catch (e) {
      fallo(e);
    } finally {
      guardando = false;
    }
  }

  async function ahora(equipo?: string) {
    aprobando = equipo ?? "*";
    try {
      const r = await api.actualizarAhora(actual.id, equipo);
      avisar(
        `Aprobada la ${r.version}. ${r.avisados ? `${plural(r.avisados, "equipo conectado la busca", "equipos conectados la buscan")} ya` : "Los equipos la buscarán al conectarse"}; cada uno espera a no tener nada en marcha.`,
      );
      await cargar();
      setTimeout(() => void recargarInformes(actual.id), 15_000);
    } catch (e) {
      fallo(e);
    } finally {
      aprobando = "";
    }
  }

  async function anillo(id: string, a: "prueba" | "general") {
    cambiandoAnillo = id;
    try {
      await api.ponerAnillo(actual.id, id, a);
      if (datos) datos.equipos[id] = { ...(datos.equipos[id] ?? {}), anillo: a };
    } catch (e) {
      fallo(e);
    } finally {
      cambiandoAnillo = "";
    }
  }

  async function refrescar() {
    await Promise.all([cargar(), recargarInformes(actual.id), cargarCliente(actual.id, { silencioso: true })]);
  }
</script>

<svelte:head><title>Versiones · {actual.cliente?.nombre ?? ""} · Resguardo Server</title></svelte:head>

<div class="page">
  <CabeceraPagina
    titulo="Versiones"
    icono={PackageCheck}
    migas={[{ texto: actual.cliente?.nombre ?? "Cliente", href: `/c/${actual.id}` }, { texto: "Versiones" }]}
    resumen="La versión del agente de cada equipo y cómo se actualizan solos: primero los de prueba y, si van bien, los demás."
  >
    {#snippet acciones()}
      {#if datos && admin && disponible}
        <BotonCargando class="btn btn-primary" cargando={aprobando === "*"} textoCargando="Aprobando…" onclick={() => ahora()} disabled={datos.politica.modo === "pausada"}
          ><CircleArrowUp size={16} />Actualizar ahora</BotonCargando
        >
      {/if}
    {/snippet}
  </CabeceraPagina>

  {#if datos === undefined || !actual.cargado}
    <Cargando />
  {:else if datos === null}
    <div class="notice notice-info"><p>Este servidor es de una versión anterior: aún no da actualizaciones a los equipos. Actualiza Resguardo Server.</p></div>
  {:else}
    <section class="card p" aria-labelledby="t-disponible">
      <div class="cab">
        <span class="card-icon"><ShieldCheck size={18} /></span>
        <div class="cab-texto">
          <h2 id="t-disponible">Versión disponible</h2>
          {#if datos.sin_llave}
            <p class="faint">Este servidor se compiló sin llave de publicación: no da versiones a los equipos. Los que pueden salir a Internet las buscan en GitHub.</p>
          {:else if datos.disponible}
            <p class="faint">
              <strong>Resguardo Agente {datos.disponible.version}</strong>, del {fechaLarga(datos.disponible.fecha)}, firmada con la llave de publicación de Resguardo.{datos.disponible.notas
                ? ` ${datos.disponible.notas}`
                : ""}
            </p>
          {:else}
            <p class="faint">Este servidor aún no tiene ninguna versión del agente para dar. La pone su propietario («Servidor → Actualizaciones de los agentes»); mientras, los equipos que salen a Internet la buscan en GitHub.</p>
          {/if}
        </div>
        {#if filas.length}
          <Chip tono={problemas ? "bad" : alDia === filas.length ? "ok" : "info"} texto={`${alDia} de ${filas.length} al día`} />
        {/if}
      </div>
      {#if datos.politica.retenidas.length}
        <div class="notice notice-warn">
          <TriangleAlert size={16} />
          <p>
            Retenida{datos.politica.retenidas.length > 1 ? "s" : ""}: {datos.politica.retenidas.join(", ")}. Falló en algún equipo del cliente, que volvió solo a la anterior; los demás no la
            instalan solos. Mira qué pasó en «Avisos»; «Actualizar ahora» la libera.
          </p>
        </div>
      {/if}
    </section>

    <section class="card p" aria-labelledby="t-politica">
      <div class="cab">
        <span class="card-icon">{#if datos.politica.modo === "pausada"}<CirclePause size={18} />{:else}<CirclePlay size={18} />{/if}</span>
        <div class="cab-texto">
          <h2 id="t-politica">Cómo se actualizan</h2>
          <p class="faint">{frasePolitica(datos.politica)} Nunca con una copia o una restauración en marcha, y si una versión no está sana en 10 minutos, el equipo vuelve solo a la anterior.</p>
        </div>
        {#if admin}
          {#if datos.politica.modo === "pausada"}
            <BotonCargando class="btn btn-sm" cargando={guardando} onclick={() => guardar(modo === "pausada" ? "auto" : modo)}><CirclePlay size={14} />Reanudar</BotonCargando>
          {:else}
            <BotonCargando class="btn btn-sm" cargando={guardando} onclick={() => guardar("pausada")}><CirclePause size={14} />Pausar</BotonCargando>
          {/if}
        {/if}
      </div>
      {#if admin}
        <form
          class="politica"
          onsubmit={(e) => {
            e.preventDefault();
            void guardar();
          }}
        >
          <fieldset class="modos">
            <legend class="sr-only">Modo</legend>
            <label class="modo" class:on={modo === "auto"}>
              <input type="radio" bind:group={modo} value="auto" />
              <span><strong>Automática</strong><span class="faint">Los de prueba en cuanto sale; los demás, unos días después.</span></span>
            </label>
            <label class="modo" class:on={modo === "manual"}>
              <input type="radio" bind:group={modo} value="manual" />
              <span><strong>Solo cuando apruebe</strong><span class="faint">Nadie se actualiza hasta que pulses «Actualizar ahora».</span></span>
            </label>
            <label class="modo" class:on={modo === "pausada"}>
              <input type="radio" bind:group={modo} value="pausada" />
              <span><strong>En pausa</strong><span class="faint">Nadie se actualiza, ni con «Actualizar ahora».</span></span>
            </label>
          </fieldset>
          <div class="campos">
            <label class="campo" class:apagado={modo !== "auto"}>
              <span>Los equipos del anillo general esperan</span>
              <span class="junto"><input class="input num" type="number" min="0" max="30" bind:value={dias} disabled={modo !== "auto"} /> días</span>
            </label>
            <label class="campo check"><input type="checkbox" bind:checked={conVentana} /> Solo dentro de una ventana de mantenimiento (hora de cada equipo)</label>
            {#if conVentana}
              <div class="junto ventana">
                de <input class="input hora" type="time" bind:value={desde} aria-label="Desde" /> a <input class="input hora" type="time" bind:value={hasta} aria-label="Hasta" />
                {#if ventanaMala}<span class="mal">Escribe las horas como 22:00.</span>{/if}
              </div>
            {/if}
          </div>
          <div class="acciones">
            <BotonCargando class="btn btn-primary btn-sm" cargando={guardando} disabled={!cambiado || ventanaMala} type="submit">Guardar</BotonCargando>
          </div>
        </form>
      {/if}
    </section>

    <section class="card p" aria-labelledby="t-equipos">
      <div class="cab">
        <div class="cab-texto">
          <h2 id="t-equipos">Equipos</h2>
          <p class="faint">
            {plural(cuenta("al_dia"), "al día", "al día")}{cuenta("pendiente") ? ` · ${cuenta("pendiente")} pendiente${cuenta("pendiente") === 1 ? "" : "s"}` : ""}{cuenta("actualizando")
              ? ` · ${cuenta("actualizando")} actualizándose`
              : ""}{problemas ? ` · ${plural(problemas, "con problemas", "con problemas")}` : ""}{cuenta("anterior")
              ? ` · ${cuenta("anterior")} con un agente que aún no se actualiza solo (instálale la versión nueva a mano una vez)`
              : ""}. Anillo <strong>prueba</strong>: los primeros (uno o dos equipos que no sean críticos).
          </p>
        </div>
        <button class="btn btn-sm btn-ghost" onclick={refrescar}>Actualizar la lista</button>
      </div>
      {#if !filas.length}
        <p class="faint">Aún no hay equipos.</p>
      {:else}
        <div class="tabla" role="table" aria-label="Versión de cada equipo">
          {#each filas as f (f.e.id)}
            <div class="fila" role="row">
              <div class="c-nombre" role="cell">
                <a href="/c/{actual.id}/equipos/{f.e.id}">{f.e.nombre}</a>
                <span class="faint mono pequeno">{f.version}{#if disponible && compararVersiones(f.version, disponible) === -1}<span class="flecha"> → {disponible}</span>{/if}</span>
              </div>
              <div class="c-estado" role="cell">
                <Chip pequeno tono={f.v.tono} texto={f.v.texto} />
                {#if f.v.detalle}<span class="faint pequeno">{f.v.detalle}</span>{/if}
                {#if estadoDe(f.e.id)?.ultima_busqueda}<span class="faint pequeno">Buscó <Tiempo iso={estadoDe(f.e.id)?.ultima_busqueda ?? null} /> ({estadoDe(f.e.id)?.origen ?? "?"})</span>{/if}
              </div>
              <div class="c-anillo" role="cell">
                {#if admin}
                  <select class="input" value={f.anillo} disabled={cambiandoAnillo === f.e.id} aria-label="Anillo de {f.e.nombre}" onchange={(ev) => anillo(f.e.id, (ev.currentTarget as HTMLSelectElement).value as "prueba" | "general")}>
                    <option value="prueba">Prueba</option>
                    <option value="general">General</option>
                  </select>
                {:else}
                  <span class="faint">{f.anillo === "prueba" ? "Prueba" : "General"}</span>
                {/if}
              </div>
              <div class="c-accion" role="cell">
                {#if admin && disponible && compararVersiones(f.version, disponible) === -1 && datos.politica.modo !== "pausada" && seActualizaSolo(f.v)}
                  <BotonCargando class="btn btn-sm btn-ghost" cargando={aprobando === f.e.id} onclick={() => ahora(f.e.id)}>Actualizar ahora</BotonCargando>
                {/if}
              </div>
            </div>
          {/each}
        </div>
        <p class="faint pequeno">Cada equipo cuenta su estado en su informe, cada pocos minutos.</p>
      {/if}
    </section>
  {/if}
</div>

<style>
  .cab {
    display: flex;
    align-items: flex-start;
    gap: var(--sp-3);
  }
  .cab-texto {
    flex: 1;
    min-width: 0;
  }
  .cab h2 {
    margin: 0;
    font-size: var(--fs-md, 15px);
  }
  .cab p {
    margin: 2px 0 0;
    font-size: var(--fs-sm);
  }
  section + section {
    margin-top: var(--sp-4);
  }
  .notice {
    margin-top: var(--sp-3);
  }
  .politica {
    margin-top: var(--sp-4);
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
  }
  .modos {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: var(--sp-2, 8px);
    border: 0;
    padding: 0;
    margin: 0;
  }
  .modo {
    display: flex;
    gap: 8px;
    align-items: flex-start;
    padding: 10px 12px;
    border: 1px solid var(--border);
    border-radius: var(--radius, 8px);
    cursor: pointer;
  }
  .modo.on {
    border-color: var(--accent);
    background: var(--accent-soft);
  }
  .modo span {
    display: flex;
    flex-direction: column;
    gap: 2px;
    font-size: var(--fs-sm);
  }
  .campos {
    display: flex;
    flex-direction: column;
    gap: 8px;
    font-size: var(--fs-sm);
  }
  .campo {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
  }
  .campo.check {
    flex-wrap: nowrap;
    align-items: flex-start;
  }
  .campo.apagado {
    opacity: 0.6;
  }
  .junto {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
  .num {
    width: 70px;
  }
  .hora {
    width: 110px;
  }
  .mal {
    color: var(--bad);
  }
  .acciones {
    display: flex;
    gap: 8px;
  }
  .tabla {
    display: flex;
    flex-direction: column;
    margin-top: var(--sp-3);
    border-top: 1px solid var(--border);
  }
  .fila {
    display: grid;
    grid-template-columns: minmax(140px, 1.2fr) 2fr 130px 150px;
    gap: var(--sp-3);
    align-items: start;
    padding: 10px 0;
    border-bottom: 1px solid var(--border);
  }
  .c-nombre,
  .c-estado {
    display: flex;
    flex-direction: column;
    gap: 3px;
    min-width: 0;
  }
  .c-nombre a {
    font-weight: 600;
  }
  .c-accion {
    display: flex;
    justify-content: flex-end;
  }
  .pequeno {
    font-size: var(--fs-xs);
  }
  .flecha {
    color: var(--accent);
  }
  .sr-only {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip: rect(0 0 0 0);
  }
  @media (max-width: 760px) {
    .modos {
      grid-template-columns: 1fr;
    }
    .fila {
      grid-template-columns: 1fr 1fr;
    }
    .c-estado {
      grid-column: 1 / -1;
      order: 3;
    }
    .cab {
      flex-wrap: wrap;
    }
    .cab-texto {
      flex-basis: calc(100% - 52px);
    }
  }
</style>
