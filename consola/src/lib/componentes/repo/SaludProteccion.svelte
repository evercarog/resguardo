<script lang="ts">
  import { tip } from "$lib/tooltip";
  // «Salud de la protección» de un repositorio, como la calcula el agente
  // (protection.rs): el anillo «5 de 7» y la lista de comprobaciones, cada una
  // con icono y palabra (nunca solo color).
  import { CircleAlert, CircleCheck, CircleDashed, TriangleAlert } from "@lucide/svelte";
  import type { Proteccion } from "$lib/repo";
  import { GLOSARIO } from "$lib/glosario";
  import Ayuda from "../Ayuda.svelte";
  import AnilloProteccion from "./AnilloProteccion.svelte";

  let { proteccion, nombre }: { proteccion: Proteccion; nombre: string } = $props();
  const PALABRA = { ok: "Bien", aviso: "Por revisar", fallo: "Falta", desconocido: "Sin comprobar" };
</script>

<section class="card salud" id="proteccion" aria-labelledby="t-proteccion">
  <div class="resumen">
    <AnilloProteccion {proteccion} tamano={64} cifra />
    <div>
      <h2 class="section-title" id="t-proteccion">Salud de la protección</h2>
      <p class="faint">
        <span class="sr-only">{proteccion.puntuacion} de {proteccion.total}.</span>
        {#if !proteccion.pendientes}
          Todo en orden: las copias de «{nombre}» están protegidas por todos los frentes.
        {:else}
          {proteccion.pendientes === 1 ? "Falta una cosa" : `Faltan ${proteccion.pendientes} cosas`} para que las copias de «{nombre}» estén protegidas del todo.
        {/if}
      </p>
    </div>
  </div>
  <ul class="items">
    {#each proteccion.items as i (i.id)}
      <li class="item st-{i.estado}">
        <span class="ic" use:tip={PALABRA[i.estado]}>
          {#if i.estado === "ok"}<CircleCheck size={16} />{:else if i.estado === "aviso"}<TriangleAlert size={16} />{:else if i.estado === "fallo"}<CircleAlert size={16} />{:else}<CircleDashed size={16} />{/if}
        </span>
        <span class="txt">
          <strong>{i.etiqueta}<span class="sr-only">: {PALABRA[i.estado]}</span>{#if GLOSARIO[`prot-${i.id}`]}<Ayuda id="prot-{i.id}" />{/if}</strong>
          {#if i.detalle}<span class="faint">{i.detalle}</span>{/if}
        </span>
      </li>
    {/each}
  </ul>
</section>

<style>
  .salud {
    display: flex;
    flex-direction: column;
    gap: var(--sp-4);
    padding: var(--sp-5);
    scroll-margin-top: 24px;
  }
  .resumen {
    display: flex;
    align-items: center;
    gap: var(--sp-4);
  }
  .resumen p {
    margin: 2px 0 0;
    font-size: var(--fs-sm);
    line-height: var(--lh-sm);
  }
  .items {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(min(100%, 280px), 1fr));
    gap: var(--sp-2);
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .item {
    display: flex;
    align-items: flex-start;
    gap: var(--sp-3);
    min-width: 0;
    padding: var(--sp-3);
    background: var(--surface-2);
    border-radius: var(--radius);
  }
  .ic {
    display: grid;
    flex: none;
    padding-top: 2px;
  }
  .st-ok .ic {
    color: var(--ok);
  }
  .st-aviso .ic {
    color: var(--warn);
  }
  .st-fallo .ic {
    color: var(--bad);
  }
  .st-desconocido .ic {
    color: var(--neutral);
  }
  .txt {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }
  .txt strong {
    display: inline-flex;
    align-items: center;
    font-weight: 500;
  }
  .txt .faint {
    font-size: var(--fs-sm);
    line-height: var(--lh-sm);
    overflow-wrap: anywhere;
  }
</style>
