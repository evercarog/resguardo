<script lang="ts">
  // Prueba de restauración desde un espejo (plan 0.8.0, E4, 14j): para cada espejo
  // del repositorio, su última prueba, «Probar ahora» y «Programar…» (p. ej. cada
  // mes). La hace el equipo dueño del repositorio, que tiene su contraseña: restaura
  // unos archivos desde la copia del espejo (la suya, una nube conectada en él o el
  // disco del espejo conectado ahí) a una carpeta temporal y los compara. El almacén
  // nunca recibe la contraseña. Cuenta en la regla 3-2-1-1-0 para ese destino.
  import { tip } from "$lib/tooltip";
  import { CalendarClock, FlaskConical, HardDrive } from "@lucide/svelte";
  import OrdenDialog from "../OrdenDialog.svelte";
  import Chip from "../Chip.svelte";
  import { opcionesPruebaEspejo, ordenPruebaEspejo, textoPruebaEspejo, type OpcionPruebaEspejo } from "$lib/pruebasFuncionales";
  import { cargarCliente, reloj } from "$lib/estado.svelte";
  import { fechaLarga, relativo } from "$lib/formato";
  import type { ReglaCopia } from "$lib/regla321";
  import type { Cliente, Equipo, EquipoDetalle } from "$lib/tipos";

  let {
    cliente,
    equipo,
    rc,
    equipos,
    ordena,
    titulo = "Pruebas desde los espejos",
  }: {
    cliente: Cliente;
    /** El equipo dueño del repositorio (el que hace la prueba). */
    equipo: EquipoDetalle | Equipo;
    /** La regla de una copia del repositorio (sus pasos «espejo», con su última prueba). */
    rc: ReglaCopia | null;
    equipos: Equipo[];
    ordena: boolean;
    /** El título de la sección (en la página de un espejo, el repositorio). */
    titulo?: string;
  } = $props();

  const admite = $derived(!!equipo.resumen?.admite?.includes("prueba_espejo"));
  const opciones = $derived(rc ? opcionesPruebaEspejo(rc.pasos, equipo, equipos) : []);
  const nubes = $derived(equipo.resumen?.nubes ?? []);

  /** Lo que se está mandando: probar ahora, programar o quitar la programada. */
  let dialogo = $state<{ que: "ahora" | "programar" | "quitar"; o: OpcionPruebaEspejo } | null>(null);
  let carpeta = $state("");
  let nube = $state("");
  let cada = $state(30);
  function abrir(que: "ahora" | "programar" | "quitar", o: OpcionPruebaEspejo) {
    carpeta = o.sugerida;
    nube = nubes.find((n) => n.nombre === o.paso.espejo?.nube)?.nombre ?? nubes[0]?.nombre ?? "";
    cada = o.paso.prueba?.cada_dias ?? 30;
    dialogo = { que, o };
  }
  const orden = $derived(dialogo && dialogo.que !== "quitar" ? ordenPruebaEspejo(dialogo.o, { carpeta, nube }, dialogo.que === "programar" ? cada : null) : null);
  const cuerpo = $derived.by((): Record<string, unknown> => {
    if (!dialogo || !rc) return {};
    if (dialogo.que === "quitar") return { repo: rc.repo.id, quitar_espejo: dialogo.o.id };
    return { repo: rc.repo.id, [dialogo.que === "ahora" ? "espejo" : "programar_espejo"]: orden };
  });
  const descripcion = $derived(
    !dialogo || !rc
      ? ""
      : dialogo.que === "quitar"
        ? `Ya no se probará sola la restauración de «${rc.repo.nombre}» desde «${dialogo.o.nombre}». Sus resultados se quedan.`
        : `«${equipo.nombre}» restaurará unos archivos de «${rc.repo.nombre}» desde la copia en «${dialogo.o.nombre}» a una carpeta temporal y los comparará${dialogo.que === "programar" ? `, cada ${cada} días` : ""}. Usa la contraseña que ya tiene: no sale del equipo. No toca tus archivos.`,
  );
</script>

{#if rc && opciones.length}
  <section class="card p pruebas-espejo" aria-labelledby="t-pe-{rc.repo.id}">
    <h3 class="section-title" id="t-pe-{rc.repo.id}"><FlaskConical size={15} />{titulo}</h3>
    {#if !admite}
      <p class="faint">Actualiza Resguardo en «{equipo.nombre}» para probar a restaurar desde sus espejos.</p>
    {/if}
    <ul>
      {#each opciones as o (o.id)}
        {@const p = o.paso.prueba}
        <li>
          <span class="nom"><HardDrive size={14} />{o.nombre}</span>
          <span class="est">
            {#if p?.resultado}
              <Chip pequeno tono={p.resultado === "ok" ? "ok" : "bad"} texto={p.resultado === "ok" ? "Probado" : "Falló"} />
              <span class="faint" use:tip={p.ultima ? fechaLarga(p.ultima) : ""}>{p.ultima ? relativo(p.ultima, reloj.ahora) : ""} · {textoPruebaEspejo(p)}</span>
            {:else}
              <span class="faint">{textoPruebaEspejo(null)}</span>
            {/if}
            {#if p?.cada_dias}<span class="faint prog"><CalendarClock size={12} />Cada {p.cada_dias} días{p.proxima ? ` · la próxima ${relativo(p.proxima, reloj.ahora)}` : ""}</span>{/if}
          </span>
          {#if o.modo === "no"}
            <span class="faint nota">{o.nota}</span>
          {:else if ordena && admite}
            {#if o.modo === "carpeta"}<span class="faint nota">{o.nota}</span>{/if}
            <span class="botones">
              <button type="button" class="btn btn-sm" onclick={() => abrir("ahora", o)}><FlaskConical size={14} />Probar ahora</button>
              <button type="button" class="btn btn-sm btn-ghost" onclick={() => abrir("programar", o)}><CalendarClock size={14} />{p?.cada_dias ? "Cambiar" : "Programar…"}</button>
              {#if p?.cada_dias}<button type="button" class="btn btn-sm btn-ghost" onclick={() => abrir("quitar", o)}>Quitar</button>{/if}
            </span>
          {/if}
        </li>
      {/each}
    </ul>
  </section>
{/if}

{#if dialogo && rc}
  <OrdenDialog
    {cliente}
    equipo={equipo as EquipoDetalle}
    tipo="probar_restauracion"
    titulo={dialogo.que === "ahora" ? `Probar desde «${dialogo.o.nombre}»` : dialogo.que === "programar" ? `Programar la prueba desde «${dialogo.o.nombre}»` : "Quitar la prueba programada"}
    {cuerpo}
    {descripcion}
    accion={dialogo.que === "ahora" ? "Probar ahora" : dialogo.que === "programar" ? "Programar" : "Quitarla"}
    valido={dialogo.que === "quitar" || !!orden}
    onclose={() => {
      dialogo = null;
      void cargarCliente(cliente.id, { silencioso: true });
    }}
  >
    {#snippet campos()}
      {#if dialogo && dialogo.que !== "quitar"}
        {#if dialogo.o.modo !== "listo"}<p class="faint">{dialogo.o.nota}</p>{/if}
        {#if dialogo.o.modo === "nube"}
          <div class="field">
            <label class="field-label" for="pe-nube">Nube de «{equipo.nombre}»</label>
            <select id="pe-nube" class="input" bind:value={nube}>
              {#each nubes as n (n.nombre)}<option value={n.nombre}>{n.nombre}</option>{/each}
            </select>
          </div>
        {/if}
        {#if dialogo.o.modo === "nube" || dialogo.o.modo === "carpeta"}
          <div class="field">
            <label class="field-label" for="pe-carpeta">{dialogo.o.modo === "nube" ? "Carpeta del repositorio en esa nube" : `Carpeta del repositorio en «${equipo.nombre}»`}</label>
            <input id="pe-carpeta" class="input" bind:value={carpeta} spellcheck="false" />
            <span class="field-hint">Dentro tiene que estar su archivo «config». El equipo solo la lee.</span>
          </div>
        {/if}
        {#if dialogo.que === "programar"}
          <div class="field">
            <label class="field-label" for="pe-cada">Cada cuánto</label>
            <select id="pe-cada" class="input" bind:value={cada}>
              <option value={7}>Cada semana</option>
              <option value={14}>Cada 2 semanas</option>
              <option value={30}>Cada mes</option>
              <option value={60}>Cada 2 meses</option>
              <option value={90}>Cada 3 meses</option>
            </select>
            <span class="field-hint">La primera, de madrugada (a las 5:00).</span>
          </div>
        {/if}
      {/if}
    {/snippet}
  </OrdenDialog>
{/if}

<style>
  .pruebas-espejo h3 {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 0 0 var(--sp-3);
  }
  .pruebas-espejo ul {
    margin: 0;
    padding: 0;
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
  }
  .pruebas-espejo li {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: 4px var(--sp-3);
    align-items: center;
  }
  .nom {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-weight: 500;
  }
  .est {
    grid-column: 1;
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
    font-size: var(--fs-sm);
  }
  .prog {
    display: inline-flex;
    align-items: center;
    gap: 4px;
  }
  .botones {
    grid-column: 2;
    grid-row: 1 / span 2;
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    justify-content: flex-end;
  }
  .nota {
    grid-column: 1 / -1;
    font-size: var(--fs-sm);
  }
  @media (max-width: 640px) {
    .pruebas-espejo li {
      grid-template-columns: 1fr;
    }
    .botones {
      grid-column: 1;
      grid-row: auto;
      justify-content: flex-start;
    }
  }
</style>
