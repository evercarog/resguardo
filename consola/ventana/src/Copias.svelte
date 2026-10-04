<script lang="ts">
  // Las copias de este equipo: el último resultado, cuándo y la próxima; «Copiar ahora».
  import { CircleAlert, CircleCheck, CircleDashed, CirclePause, LoaderCircle, Play, TriangleAlert } from "@lucide/svelte";
  import { cuandoFrase, relativo } from "$lib/formato";
  import { pedir, vivo } from "./puente.svelte";
  import type { CopiaBandeja } from "./tipos";

  const b = $derived(vivo.datos?.bandeja ?? null);
  const copias = $derived(b?.copias ?? []);
  const enCurso = (c: CopiaBandeja) => b?.actividades?.find((a) => a.clave === `copia:${c.clave}`);
  let aviso = $state<{ clave: string; texto: string; error: boolean } | null>(null);

  async function copiar(c: CopiaBandeja) {
    try {
      const r = await pedir<{ mensaje: string }>("copiar", { claves: [c.clave] });
      aviso = { clave: c.clave, texto: r.mensaje, error: false };
    } catch (e) {
      aviso = { clave: c.clave, texto: (e as Error).message, error: true };
    }
  }
  const estado = (c: CopiaBandeja) =>
    ({ ok: "Correcta", warning: "Con avisos", error: "Falló", "": "Aún sin copias" })[c.resultado] ?? c.resultado;
</script>

{#if !copias.length}
  <div class="v-tarjeta vacio">
    <CircleDashed size={24} aria-hidden="true" />
    <p class="v-sub">{b?.local ? "Aún no hay copias: créalas en Ajustes." : b?.vinculado ? "La consola aún no ha asignado copias a este equipo." : "Sin copias todavía."}</p>
  </div>
{:else}
  <ul class="lista">
    {#each copias as c (c.clave)}
      {@const a = enCurso(c)}
      <li class="v-tarjeta fila">
        <span class="ico" data-r={a ? "curso" : c.pausada ? "pausa" : c.resultado}>
          {#if a}<LoaderCircle size={18} class="spin" aria-hidden="true" />
          {:else if c.pausada}<CirclePause size={18} aria-hidden="true" />
          {:else if c.resultado === "ok"}<CircleCheck size={18} aria-hidden="true" />
          {:else if c.resultado === "warning"}<TriangleAlert size={18} aria-hidden="true" />
          {:else if c.resultado === "error"}<CircleAlert size={18} aria-hidden="true" />
          {:else}<CircleDashed size={18} aria-hidden="true" />{/if}
        </span>
        <div class="texto">
          <p class="nombre v-cortar">{c.nombre}</p>
          <p class="v-mini">
            {#if a}Copiando ahora{a.porcentaje != null ? ` · ${Math.floor(a.porcentaje * 100)} %` : "…"}
            {:else}{estado(c)}{c.cuando ? ` ${relativo(c.cuando)}` : ""}{c.pausada ? " · en pausa" : ""}{/if}
          </p>
          {#if c.proxima && !a}<p class="v-mini">Próxima: {cuandoFrase(c.proxima)}</p>{/if}
          {#if aviso?.clave === c.clave}<p class={aviso.error ? "v-error" : "v-ok"} role="status">{aviso.texto}</p>{/if}
        </div>
        {#if b?.pedir && !c.pausada && !a}
          <button class="btn btn-sm" onclick={() => copiar(c)} aria-label={`Copiar ahora «${c.nombre}»`}><Play size={14} aria-hidden="true" />Copiar ahora</button>
        {/if}
      </li>
    {/each}
  </ul>
{/if}

<style>
  .lista {
    display: grid;
    gap: var(--sp-2);
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .fila {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    padding: var(--sp-3);
  }
  .texto {
    flex: 1;
    min-width: 0;
  }
  .nombre {
    margin: 0;
    font-weight: 600;
  }
  .ico {
    flex: none;
    display: grid;
    place-items: center;
    color: var(--neutral);
  }
  .ico[data-r="ok"] {
    color: var(--ok);
  }
  .ico[data-r="warning"] {
    color: var(--warn);
  }
  .ico[data-r="error"] {
    color: var(--bad);
  }
  .ico[data-r="curso"] {
    color: var(--info);
  }
  .ico[data-r="pausa"] {
    color: var(--paused);
  }
  .vacio {
    display: grid;
    justify-items: center;
    gap: var(--sp-2);
    text-align: center;
    color: var(--text-3);
  }
</style>
