<script lang="ts">
  // La barra de fuerza de una clave nueva, con su texto (G5 y G7, plan 0.8.1):
  // una sola pieza para «Cambiar la clave de administración» y la clave de
  // respaldo de la consola (antes, copiada en los dos y sin `aria-valuetext`).
  import type { Fuerza } from "$lib/fuerza";

  let { f, extra = "" }: { f: Fuerza; extra?: string } = $props();
  const texto = $derived(`${f.texto}${extra ? ` · ${extra}` : f.consejo ? ` · ${f.consejo}` : ""}`);
</script>

<div class="fuerza" aria-live="polite">
  <div class="barra nivel-{f.nivel}" role="meter" aria-valuemin="0" aria-valuemax="4" aria-valuenow={f.nivel} aria-valuetext={f.texto} aria-label="Fuerza de la clave"><span></span></div>
  <span class="pequeno">{texto}</span>
</div>

<style>
  .fuerza {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    margin-top: -6px;
  }
  .barra {
    flex: 0 0 140px;
    height: 6px;
    border-radius: 3px;
    background: var(--border);
    overflow: hidden;
  }
  .barra span {
    display: block;
    height: 100%;
    width: 8%;
    background: var(--bad);
  }
  .nivel-1 span {
    width: 30%;
  }
  .nivel-2 span {
    width: 55%;
    background: var(--warn);
  }
  .nivel-3 span {
    width: 80%;
    background: var(--ok);
  }
  .nivel-4 span {
    width: 100%;
    background: var(--ok);
  }
  .pequeno {
    font-size: var(--fs-xs);
    color: var(--text-2);
  }
</style>
