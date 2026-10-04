<script lang="ts">
  // Esqueleto de lo que va a aparecer, con la misma forma (filas de una lista,
  // tarjetas, una tabla, cifras o la cabecera de una ficha). Se enseña hasta
  // la primera carga: nunca un «no hay…» antes de saberlo. Con
  // «reducir movimiento», sin brillo.
  let { forma = "filas", n = 4, etiqueta = "Cargando…" }: { forma?: "filas" | "tarjetas" | "tabla" | "cifras" | "ficha" | "lineas"; n?: number; etiqueta?: string } = $props();
  const ancho = (i: number, base = 70, var_ = 30) => `${base - ((i * 17) % var_)}%`;
</script>

<div class="esq {forma}" aria-busy="true" role="status" aria-label={etiqueta}>
  {#if forma === "filas"}
    <div class="card p-0">
      {#each Array(n) as _, i (i)}
        <div class="fila-e">
          <span class="b cuadro"></span>
          <span class="col"><span class="b l" style:width={ancho(i, 55, 25)}></span><span class="b s" style:width={ancho(i + 2, 38, 20)}></span></span>
          <span class="b pill"></span>
        </div>
      {/each}
    </div>
  {:else if forma === "tarjetas"}
    <div class="rejilla-e">
      {#each Array(n) as _, i (i)}
        <div class="card tarjeta-e">
          <span class="cab-e"><span class="b cuadro"></span><span class="b l" style:width={ancho(i, 60, 25)}></span></span>
          <span class="b s" style:width={ancho(i + 1, 90, 30)}></span>
          <span class="b s" style:width={ancho(i + 3, 70, 30)}></span>
          <span class="pie-e"><span class="b pill"></span><span class="b pill corta"></span></span>
        </div>
      {/each}
    </div>
  {:else if forma === "tabla"}
    <div class="card p-0">
      <div class="fila-e cabecera">{#each Array(5) as _, j (j)}<span class="b s celda"></span>{/each}</div>
      {#each Array(n) as _, i (i)}
        <div class="fila-e">{#each Array(5) as _, j (j)}<span class="b s celda" style:max-width={ancho(i + j, 100, 40)}></span>{/each}</div>
      {/each}
    </div>
  {:else if forma === "cifras"}
    <div class="cifras-e">
      {#each Array(n) as _, i (i)}
        <div class="card cifra-e"><span class="b s" style:width="55%"></span><span class="b grande" style:width={ancho(i, 50, 20)}></span></div>
      {/each}
    </div>
  {:else if forma === "ficha"}
    <div class="ficha-e">
      <span class="cab-e"><span class="b cuadro grande-c"></span><span class="col"><span class="b titulo" style:width="40%"></span><span class="b s" style:width="60%"></span></span></span>
      <span class="pestanas-e">{#each Array(4) as _, j (j)}<span class="b s" style:width="70px"></span>{/each}</span>
    </div>
    <div class="card p-0">
      {#each Array(n) as _, i (i)}
        <div class="fila-e">
          <span class="col"><span class="b l" style:width={ancho(i, 50, 25)}></span><span class="b s" style:width={ancho(i + 2, 70, 30)}></span></span>
          <span class="b pill"></span>
        </div>
      {/each}
    </div>
  {:else}
    <div class="lineas-e">
      {#each Array(n) as _, i (i)}<span class="b l" style:width={ancho(i, 90, 35)}></span>{/each}
    </div>
  {/if}
</div>

<style>
  .esq {
    display: flex;
    flex-direction: column;
    gap: var(--sp-4);
  }
  .b {
    display: block;
    border-radius: 6px;
    background: linear-gradient(90deg, var(--surface-3) 0%, color-mix(in srgb, var(--surface-3) 55%, var(--surface)) 50%, var(--surface-3) 100%);
    background-size: 200% 100%;
    animation: brillo 1.4s ease-in-out infinite;
  }
  @keyframes brillo {
    from {
      background-position: 100% 0;
    }
    to {
      background-position: -100% 0;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .b {
      animation: none;
      background: var(--surface-3);
    }
  }
  .l {
    height: 14px;
  }
  .s {
    height: 10px;
  }
  .titulo {
    height: 22px;
  }
  .grande {
    height: 24px;
  }
  .cuadro {
    flex: none;
    width: 32px;
    height: 32px;
    border-radius: var(--radius);
  }
  .grande-c {
    width: 44px;
    height: 44px;
  }
  .pill {
    flex: none;
    width: 76px;
    height: 20px;
    border-radius: 999px;
  }
  .pill.corta {
    width: 52px;
  }
  .col {
    display: flex;
    flex-direction: column;
    gap: 7px;
    flex: 1;
    min-width: 0;
  }
  .fila-e {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    min-height: 56px;
    padding: 10px var(--sp-4);
    border-top: 1px solid var(--border);
  }
  .fila-e:first-child {
    border-top: none;
  }
  .fila-e.cabecera {
    min-height: 40px;
    background: var(--surface-2);
  }
  .celda {
    flex: 1;
  }
  .rejilla-e {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(min(100%, 260px), 1fr));
    gap: var(--sp-4);
  }
  .tarjeta-e {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: var(--sp-4);
  }
  .cab-e {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
  }
  .pie-e {
    display: flex;
    gap: 6px;
    margin-top: 4px;
  }
  .cifras-e {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(min(100%, 150px), 1fr));
    gap: var(--sp-3);
  }
  .cifra-e {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: var(--sp-4);
  }
  .ficha-e {
    display: flex;
    flex-direction: column;
    gap: var(--sp-4);
  }
  .pestanas-e {
    display: flex;
    gap: var(--sp-4);
  }
  .lineas-e {
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 8px 0;
  }
</style>
