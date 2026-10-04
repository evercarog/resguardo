<script lang="ts" module>
  /** Lo que pide un ajuste para cambiarse. */
  export type Need = "admin" | "password" | "hello";
</script>

<script lang="ts">
  // Una fila de Ajustes: etiqueta, una línea de explicación y el control a la
  // derecha. Si el ajuste pide algo para cambiarse (administrador, la
  // contraseña de un destino, Windows Hello), lo dice con la misma insignia en
  // todas partes. Debajo puede ir más (un formulario, un aviso).
  import type { Snippet } from "svelte";
  import { Fingerprint, Lock, ShieldCheck } from "@lucide/svelte";

  interface Props {
    label: string;
    /** Una línea (o dos) de explicación. */
    description?: string;
    needs?: Need[];
    /** Id del control (la etiqueta lo señala). */
    for?: string;
    /** El control, a la derecha. */
    children?: Snippet;
    /** Lo que va debajo, a todo el ancho. */
    below?: Snippet;
    /** Texto de estado bajo la explicación (en lugar de, o además de, `description`). */
    status?: Snippet;
  }
  let { label, description, needs = [], for: forId, children, below, status }: Props = $props();

  const NEED = {
    admin: { icon: ShieldCheck, text: "Administrador", title: "Para cambiarlo hay que abrir Resguardo como administrador" },
    password: { icon: Lock, text: "Contraseña", title: "Para cambiarlo se pide la contraseña de un repositorio" },
    hello: { icon: Fingerprint, text: "Windows Hello", title: "Para cambiarlo se pide confirmar que eres tú con Windows Hello" },
  } as const;
</script>

<div class="srow">
  <div class="main">
    <div class="text">
      <div class="label-line">
        {#if forId}<label class="label" for={forId}>{label}</label>{:else}<span class="label">{label}</span>{/if}
        {#each needs as n (n)}
          {@const b = NEED[n]}
          <span class="badge badge-sm tone-neutral need" title={b.title}><b.icon size={11} /> {b.text}</span>
        {/each}
      </div>
      {#if description}<p class="desc">{description}</p>{/if}
      {#if status}<div class="status">{@render status()}</div>{/if}
    </div>
    {#if children}<div class="control">{@render children()}</div>{/if}
  </div>
  {#if below}<div class="below">{@render below()}</div>{/if}
</div>

<style>
  .srow {
    padding: 14px 0;
  }
  .srow + :global(.srow) {
    border-top: 1px solid var(--border);
  }
  .main {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 16px;
  }
  .text {
    display: flex;
    flex-direction: column;
    gap: 3px;
    min-width: 0;
    flex: 1;
  }
  .label-line {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 6px 8px;
  }
  .label {
    font-size: var(--fs-body);
    font-weight: 500;
    color: var(--text-1);
  }
  .need {
    gap: 3px;
  }
  .desc {
    margin: 0;
    font-size: var(--fs-sm);
    line-height: 1.45;
    color: var(--text-3);
  }
  .status {
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  .control {
    display: flex;
    align-items: center;
    gap: 6px;
    flex: none;
    padding-top: 1px;
  }
  /* Lo de debajo separa solo si hay algo (los bloques vacíos dejan marcadores). */
  .below > :global(*:first-child) {
    margin-top: 12px;
  }
  .status:empty {
    display: none;
  }
</style>
