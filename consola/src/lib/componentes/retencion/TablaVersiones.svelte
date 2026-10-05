<script lang="ts">
  // Una lista de versiones de «Retención en detalle» (quitadas o que se
  // quitarán): versión, fecha, copia, lo que ocupaban sus archivos y por qué.
  // En el móvil, cada versión en un bloque (sin desplazamiento lateral).
  import { bytes, fechaLarga } from "$lib/formato";
  import type { FilaVersion } from "$lib/retencionDetalle";

  let { filas, titulo, vacio = "Ninguna." }: { filas: FilaVersion[]; titulo: string; vacio?: string } = $props();
  const fmt = new Intl.DateTimeFormat("es", { weekday: "short", day: "numeric", month: "short", hour: "2-digit", minute: "2-digit" });
</script>

{#if filas.length}
  <table class="tabla">
    <caption class="sr-only">{titulo}</caption>
    <thead>
      <tr><th scope="col">Versión</th><th scope="col">Fecha</th><th scope="col">Copia</th><th scope="col" class="der">Tamaño</th><th scope="col">Por qué</th></tr>
    </thead>
    <tbody>
      {#each filas as f (f.id)}
        <tr class:queda={f.tono === "queda"}>
          <td data-k="Versión"><code>{f.id}</code></td>
          <td data-k="Fecha">{#if f.hora}<span title={fechaLarga(f.hora)}>{fmt.format(new Date(f.hora))}</span>{:else}<span class="faint">—</span>{/if}</td>
          <td data-k="Copia">{f.copia ?? "—"}</td>
          <td data-k="Tamaño" class="der num">{f.bytes != null ? bytes(f.bytes) : "—"}</td>
          <td data-k="Por qué" class="porque">{f.porque}</td>
        </tr>
      {/each}
    </tbody>
  </table>
{:else}
  <p class="faint vacio">{vacio}</p>
{/if}

<style>
  .tabla {
    width: 100%;
    border-collapse: collapse;
    font-size: var(--fs-sm);
  }
  th {
    text-align: left;
    font-size: var(--fs-xs);
    font-weight: 500;
    color: var(--text-3);
    padding: 6px 8px;
    border-bottom: 1px solid var(--border);
  }
  td {
    padding: 6px 8px;
    border-bottom: 1px solid var(--border);
    vertical-align: top;
  }
  tr:last-child td {
    border-bottom: none;
  }
  code {
    font-family: var(--mono);
    font-size: var(--fs-xs);
  }
  .der {
    text-align: right;
    white-space: nowrap;
  }
  .porque {
    color: var(--text-2);
    min-width: 16ch;
  }
  tr.queda .porque {
    color: var(--text-3);
  }
  .vacio {
    margin: 0;
    font-size: var(--fs-sm);
  }
  .sr-only {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip: rect(0 0 0 0);
    white-space: nowrap;
  }
  @media (max-width: 640px) {
    thead {
      display: none;
    }
    .tabla,
    tbody,
    tr,
    td {
      display: block;
    }
    tr {
      padding: 8px 0;
      border-bottom: 1px solid var(--border);
    }
    tr:last-child {
      border-bottom: none;
    }
    td {
      display: grid;
      grid-template-columns: 9ch minmax(0, 1fr);
      gap: 8px;
      padding: 2px 0;
      border: none;
      text-align: left;
    }
    td::before {
      content: attr(data-k);
      font-size: var(--fs-xs);
      color: var(--text-3);
    }
    .der {
      text-align: left;
    }
  }
</style>
