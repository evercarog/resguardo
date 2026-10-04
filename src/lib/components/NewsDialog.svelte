<script lang="ts">
  // «Novedades»: qué cambió desde la versión anterior (o las últimas versiones).
  import { Sparkles, X } from "@lucide/svelte";
  import { CHANGELOG, compareVersions } from "$lib/changelog";
  import { news } from "$lib/news.svelte";
  import Modal from "./Modal.svelte";

  let { onclose }: { onclose: () => void } = $props();

  const dateFmt = new Intl.DateTimeFormat("es", { day: "numeric", month: "long", year: "numeric" });
  const when = (d: string) => dateFmt.format(new Date(`${d}T12:00:00`));

  /** Ver también las anteriores (todas). */
  let all = $state(false);
  // «Todas»: desde la más reciente mostrada hacia atrás (nunca las aún no publicadas).
  const list = $derived(all ? CHANGELOG.filter((r) => !news.releases[0] || compareVersions(r.version, news.releases[0].version) <= 0) : news.releases);
</script>

<Modal {onclose} labelledby="news-title" width={560}>
  <header class="dlg-head">
    <div class="dlg-title">
    <span class="ticon"><Sparkles size={19} /></span>
    <div>
      <h2 id="news-title">Novedades</h2>
      <p class="faint">
        {#if news.since && !all}Lo nuevo desde la versión {news.since}.{:else}Lo último en Resguardo.{/if}
      </p>
    </div>
    </div>
    <button class="icon-btn" title="Cerrar" aria-label="Cerrar" onclick={onclose}><X size={17} /></button>
  </header>

  <div class="list">
    {#each list as r (r.version)}
      <section>
        <h3>
          <span>{r.title}</span>
          <span class="badge badge-sm tone-accent">v{r.version}</span>
        </h3>
        <p class="date faint">{when(r.date)}</p>
        <ul>
          {#each r.items as item, i (i)}<li>{item}</li>{/each}
        </ul>
      </section>
    {/each}
  </div>

  <footer>
    {#if !all}
      <button class="btn btn-ghost" onclick={() => (all = true)}>Ver versiones anteriores</button>
    {/if}
    <span class="spacer"></span>
    <button class="btn btn-primary" onclick={onclose}>Entendido</button>
  </footer>
</Modal>

<style>
  .list {
    display: flex;
    flex-direction: column;
    gap: 18px;
    max-height: min(56vh, 520px);
    overflow-y: auto;
    padding-right: 4px;
  }
  h3 {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 0;
    font-size: var(--fs-body);
  }
  .date {
    margin: 2px 0 6px;
    font-size: var(--fs-xs);
  }
  ul {
    margin: 0;
    padding-left: 20px;
    display: flex;
    flex-direction: column;
    gap: 5px;
    font-size: var(--fs-sm);
    line-height: 1.5;
  }
  footer {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 18px;
  }
</style>
