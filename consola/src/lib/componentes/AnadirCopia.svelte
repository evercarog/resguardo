<script lang="ts">
  // «Añadir una copia» (tarea 7f, docs/copias-en-cadena.md): primero qué se
  // copia y, si es el repositorio de otra copia, qué tipo de paso. Después
  // sigue el diálogo de cada uno (cuándo, destino, repositorio, qué versiones
  // traer y el resumen):
  // - carpetas del equipo → «Cambiar las copias» con una copia nueva (con la
  //   clave de administración), si se quiere «después de» otra;
  // - espejo → el almacén donde está el repositorio (PasoEspejo);
  // - repositorio nuevo a partir de él → copia derivada (CopiaDerivada).
  import { goto } from "$app/navigation";
  import { Copy, FolderOpen, HardDrive, Link2 } from "@lucide/svelte";
  import Modal from "$ui/componentes/Modal.svelte";
  import { ADMITE, admite, repoEnAlmacen } from "$lib/cadenas";
  import type { CopiaResumen, Equipo, RepositorioResumen } from "$lib/tipos";

  interface Props {
    cliente: string;
    equipo: Equipo;
    equipos: Equipo[];
    onclose: () => void;
    /** Sigue con un paso «espejo» o una copia derivada de ese repositorio. */
    alElegir: (que: "espejo" | "derivada", repo: RepositorioResumen) => void;
  }
  let { cliente, equipo, equipos, onclose, alElegir }: Props = $props();

  const copias = $derived<CopiaResumen[]>(equipo.resumen?.copias ?? []);
  let que = $state<"carpetas" | "paso">("carpetas");
  // svelte-ignore state_referenced_locally
  let copiaId = $state(copias[0]?.id ?? "");
  let despues = $state(false);
  const repo = $derived(equipo.resumen?.repositorios?.find((r) => r.id === copias.find((k) => k.id === copiaId)?.repo) ?? null);
  const conEspejo = $derived(!!repo && !!repoEnAlmacen(equipo, repo, equipos) && admite(repoEnAlmacen(equipo, repo, equipos)!.almacen, ADMITE.espejoZonas));
  const conDerivadas = $derived(admite(equipo, ADMITE.derivadas));
  const conCadenas = $derived(admite(equipo, ADMITE.cadenas));

  function seguir(tipo?: "espejo" | "derivada") {
    if (que === "carpetas") {
      const q = new URLSearchParams({ nueva: "1", ...(despues && copiaId && conCadenas ? { tras: copiaId } : {}) });
      void goto(`/c/${cliente}/equipos/${equipo.id}/copias?${q}`);
      onclose();
    } else if (repo && tipo) alElegir(tipo, repo);
  }
</script>

<Modal labelledby="t-anadir-copia" {onclose} width={540}>
  <div class="dlg-title">
    <span class="ticon"><Copy size={18} /></span>
    <div>
      <h2 id="t-anadir-copia">Añadir una copia</h2>
      <p>Qué copiar primero; después, cuándo, a qué destino y el resumen del camino completo.</p>
    </div>
  </div>
  <div class="form">
    <div class="segmented" role="radiogroup" aria-label="Qué se copia">
      <button type="button" role="radio" aria-checked={que === "carpetas"} class:on={que === "carpetas"} onclick={() => (que = "carpetas")}><FolderOpen size={14} />Carpetas de {equipo.nombre}</button>
      <button type="button" role="radio" aria-checked={que === "paso"} class:on={que === "paso"} disabled={!copias.length} onclick={() => (que = "paso")}><Link2 size={14} />El repositorio de otra copia</button>
    </div>
    {#if copias.length && (que === "paso" || (conCadenas && despues))}
      <div class="field">
        <label class="field-label" for="ac-copia">{que === "paso" ? "De la copia" : "Después de"}</label>
        <select id="ac-copia" class="input" bind:value={copiaId}>
          {#each copias as k (k.id)}<option value={k.id}>{k.nombre}</option>{/each}
        </select>
      </div>
    {/if}
    {#if que === "carpetas"}
      {#if conCadenas && copias.length}<label class="switch-row"><input type="checkbox" bind:checked={despues} /><span>Empieza después de otra copia<span class="faint">Cuando termine bien; si falla, esta no se hace y se avisa.</span></span></label>{/if}
      <p class="faint nota">Se abre «Cambiar las copias» con una copia nueva: carpetas, repositorio (uno que ya existe en su destino o «Nuevo repositorio») y horario. Se envía con la clave de administración.</p>
      <footer>
        <button type="button" class="btn btn-ghost" onclick={onclose}>Cancelar</button>
        <button type="button" class="btn btn-primary" onclick={() => seguir()}>Seguir</button>
      </footer>
    {:else}
      <div class="tipos">
        <button type="button" class="tipo" disabled={!conEspejo} onclick={() => seguir("espejo")}>
          <HardDrive size={18} /><strong>Espejo</strong>
          <span class="faint">El mismo repositorio en otro destino (otra zona o una nube del almacén). Lo hace el almacén, sin contraseñas. {#if !conEspejo}Solo si el repositorio está en un almacén actualizado.{/if}</span>
        </button>
        <button type="button" class="tipo" disabled={!conDerivadas} onclick={() => seguir("derivada")}>
          <Copy size={18} /><strong>Repositorio nuevo a partir de este</strong>
          <span class="faint">Otro repositorio (la misma contraseña u otra) al que se traen las versiones, todas o filtradas, y después las nuevas. Lo hace {equipo.nombre}. {#if !conDerivadas}Actualiza su agente para poder hacerlo.{/if}</span>
        </button>
      </div>
      <footer><button type="button" class="btn btn-ghost" onclick={onclose}>Cancelar</button></footer>
    {/if}
  </div>
</Modal>

<style>
  .tipos {
    display: grid;
    gap: var(--sp-3);
  }
  .tipo {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 2px 10px;
    padding: var(--sp-3);
    text-align: left;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--surface, transparent);
    color: inherit;
    cursor: pointer;
  }
  .tipo:disabled {
    opacity: 0.6;
    cursor: default;
  }
  .tipo:not(:disabled):hover {
    border-color: var(--accent, var(--border-strong));
  }
  .tipo .faint {
    grid-column: 2;
    font-size: var(--fs-sm);
  }
  .segmented {
    flex-wrap: wrap;
  }
  .nota {
    margin: 0;
    font-size: var(--fs-sm);
  }
</style>
