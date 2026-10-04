<script lang="ts">
  import { tick } from "svelte";
  import {
    CalendarClock,
    CircleHelp,
    FolderSync,
    HardDrive,
    Keyboard,
    BookOpen,
    Layers,
    Rocket,
    RotateCcw,
    Search,
    ShieldCheck,
    Wrench,
    X,
    Sparkles,
  } from "@lucide/svelte";
  import { openNews } from "$lib/news.svelte";
  import { HELP, normalize, sectionOf, type HelpItem, type HelpSection } from "$lib/helpContent";
  import Modal from "./Modal.svelte";

  interface Props {
    /** Sección o apartado que se abre (null: el primero). */
    topic: string | null;
    onclose: () => void;
  }
  let { topic, onclose }: Props = $props();

  const ICONS = {
    rocket: Rocket,
    harddrive: HardDrive,
    foldersync: FolderSync,
    calendarclock: CalendarClock,
    wrench: Wrench,
    rotateccw: RotateCcw,
    layers: Layers,
    shieldcheck: ShieldCheck,
    keyboard: Keyboard,
    book: BookOpen,
  } as Record<string, typeof Rocket>;

  // svelte-ignore state_referenced_locally
  let current = $state<HelpSection>(sectionOf(topic ?? "") ?? HELP[0]);
  let query = $state("");
  let highlighted = $state<string | null>(null);
  let content = $state<HTMLElement>();

  // Índice de búsqueda: texto sin acentos de cada apartado (con el título de su sección).
  const index = HELP.flatMap((s) => s.items.map((item) => ({ section: s, item, text: normalize(`${s.title} ${item.title} ${item.html}`) })));

  const words = $derived(normalize(query).split(/\s+/).filter(Boolean));
  const results = $derived(words.length ? index.filter((e) => words.every((w) => e.text.includes(w))) : []);
  /** Secciones con algún resultado (para marcar el índice lateral). */
  const matching = $derived(new Set(results.map((r) => r.section.id)));

  /** Va a una sección o apartado (desde fuera o desde un enlace interno). */
  async function go(id: string) {
    const section = sectionOf(id);
    if (!section) return;
    query = "";
    current = section;
    await tick();
    const item = section.items.find((i) => i.id === id);
    if (item) {
      document.getElementById(`help-${item.id}`)?.scrollIntoView({ block: "start", behavior: "smooth" });
      highlighted = item.id;
      setTimeout(() => highlighted === item.id && (highlighted = null), 1600);
    } else {
      content?.scrollTo({ top: 0 });
    }
  }

  // Al abrir con un apartado concreto, se baja hasta él.
  $effect(() => {
    if (topic) go(topic);
  });

  function onclick(e: MouseEvent) {
    const link = (e.target as HTMLElement).closest<HTMLElement>("a[data-topic]");
    if (link) {
      e.preventDefault();
      go(link.dataset.topic!);
    }
  }

  function pick(section: HelpSection) {
    query = "";
    current = section;
    content?.scrollTo({ top: 0 });
  }
</script>

{#snippet article(item: HelpItem)}
  <article id="help-{item.id}" class:flash={highlighted === item.id}>
    <h4>{item.title}</h4>
    <!-- Contenido estático escrito por nosotros (src/lib/helpContent.ts). -->
    <div class="prose">{@html item.html}</div>
  </article>
{/snippet}

<Modal {onclose} labelledby="help-title" width={860}>
  <header>
    <div class="title">
      <span class="ticon"><CircleHelp size={19} /></span>
      <div>
        <h2 id="help-title">Ayuda</h2>
        <p class="faint">Guías rápidas para sacar partido a Resguardo</p>
      </div>
    </div>
    <label class="search">
      <Search size={14} />
      <input class="input" type="search" placeholder="Buscar en la ayuda" bind:value={query} spellcheck="false" aria-label="Buscar en la ayuda" />
      {#if query}<button class="icon-btn clear" onclick={() => (query = "")} title="Limpiar" aria-label="Limpiar búsqueda"><X size={13} /></button>{/if}
    </label>
    <button class="btn btn-ghost btn-sm" onclick={() => (onclose(), openNews())} title="Qué cambió en las últimas versiones"><Sparkles size={14} /> Novedades</button>
    <button class="icon-btn" title="Cerrar (Esc)" aria-label="Cerrar la ayuda" onclick={onclose}><X size={17} /></button>
  </header>

  <div class="layout">
    <nav aria-label="Secciones de la ayuda">
      {#each HELP as s (s.id)}
        {@const Icon = ICONS[s.icon] ?? CircleHelp}
        <button
          class="nav-item"
          class:on={!words.length && current.id === s.id}
          class:dim={words.length > 0 && !matching.has(s.id)}
          aria-current={!words.length && current.id === s.id ? "true" : undefined}
          onclick={() => pick(s)}
        >
          <Icon size={15} />
          <span>{s.title}</span>
        </button>
      {/each}
    </nav>

    <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
    <div class="content" bind:this={content} {onclick}>
      {#if words.length}
        {#if results.length}
          <p class="faint count">{results.length} {results.length === 1 ? "resultado" : "resultados"}</p>
          {#each results as r (r.item.id)}
            <span class="crumb">{r.section.title}</span>
            {@render article(r.item)}
          {/each}
        {:else}
          <div class="empty">
            <Search size={22} />
            <strong>Nada coincide con «{query}»</strong>
            <span class="faint">Prueba con otra palabra, como «contraseña», «horario» o «restaurar».</span>
          </div>
        {/if}
      {:else}
        <h3>{current.title}</h3>
        {#each current.items as item (item.id)}
          {@render article(item)}
        {/each}
      {/if}
    </div>
  </div>
</Modal>

<style>
  header {
    display: flex;
    align-items: center;
    gap: 14px;
    margin-bottom: 16px;
  }
  .title {
    display: flex;
    gap: 12px;
    align-items: center;
    flex: 1;
    min-width: 0;
  }
  h2 {
    font-size: 17px;
    font-weight: 650;
  }
  .title p {
    margin: 1px 0 0;
    font-size: var(--fs-sm);
  }
  .search {
    position: relative;
    display: flex;
    align-items: center;
    width: 260px;
    flex: none;
  }
  .search > :global(svg) {
    position: absolute;
    left: 10px;
    color: var(--text-3);
    pointer-events: none;
  }
  .search .input {
    height: 34px;
    padding-left: 30px;
    padding-right: 28px;
  }
  .search .input::-webkit-search-cancel-button {
    display: none;
  }
  .search .clear {
    position: absolute;
    right: 3px;
    width: 26px;
    height: 26px;
  }

  .layout {
    display: grid;
    grid-template-columns: 200px minmax(0, 1fr);
    gap: 4px;
    height: min(560px, calc(100dvh - 190px));
  }
  nav {
    display: flex;
    flex-direction: column;
    gap: 2px;
    overflow: auto;
  }
  .nav-item {
    display: flex;
    align-items: center;
    gap: 9px;
    padding: 8px 10px;
    font: inherit;
    font-size: var(--fs-sm);
    font-weight: 550;
    text-align: left;
    color: var(--text-2);
    background: none;
    border: none;
    border-radius: var(--radius);
    cursor: pointer;
    transition:
      background 0.12s,
      color 0.12s,
      opacity 0.12s;
  }
  .nav-item:hover {
    color: var(--text-1);
    background: var(--surface-3);
  }
  .nav-item.on {
    color: var(--accent-text);
    background: var(--accent-soft);
  }
  .nav-item.dim {
    opacity: 0.45;
  }
  .content {
    overflow: hidden auto;
    padding: 0 0 12px;
    scroll-padding-top: 4px;
  }
  h3 {
    margin: 0 0 6px;
    padding: 0 14px;
    font-size: 17px;
    font-weight: 650;
  }
  article {
    padding: 12px 14px;
    margin: 0 0 4px;
    border-radius: var(--radius);
    transition: background 0.4s;
  }
  article.flash {
    background: var(--accent-soft);
  }
  h4 {
    margin: 0 0 6px;
    font-size: var(--fs-body);
    font-weight: 650;
  }
  .prose {
    font-size: var(--fs-sm);
    line-height: 1.6;
    color: var(--text-2);
  }
  .prose :global(p) {
    margin: 0 0 8px;
  }
  .prose :global(ul),
  .prose :global(ol) {
    margin: 0 0 8px;
    padding-left: 20px;
  }
  .prose :global(li) {
    margin-bottom: 4px;
  }
  .prose :global(strong) {
    color: var(--text-1);
    font-weight: 620;
  }
  .prose :global(code) {
    padding: 1px 5px;
    font-family: var(--mono);
    font-size: var(--fs-xs);
    border-radius: var(--radius-sm);
    background: var(--surface-3);
    color: var(--text-1);
  }
  .prose :global(kbd) {
    display: inline-block;
    min-width: 20px;
    padding: 0 6px;
    font-family: var(--font);
    font-size: var(--fs-xs);
    font-weight: 600;
    line-height: 20px;
    text-align: center;
    color: var(--text-1);
    background: var(--surface-2);
    border: 1px solid var(--border-strong);
    border-bottom-width: 2px;
    border-radius: var(--radius-sm);
  }
  .prose :global(a[data-topic]) {
    color: var(--accent-text);
    text-decoration: underline;
    text-underline-offset: 2px;
    cursor: pointer;
  }
  .prose :global(ul.keys) {
    list-style: none;
    padding: 0;
  }
  .prose :global(ul.keys li) {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 7px 0;
    margin: 0;
    border-bottom: 1px solid var(--border);
  }
  .prose :global(ul.keys li > span) {
    flex: none;
    width: 170px;
  }
  .count {
    margin: 0 0 6px;
    padding: 0 14px;
    font-size: var(--fs-sm);
  }
  .crumb {
    display: block;
    margin-top: 6px;
    padding: 0 14px;
    font-size: var(--fs-xs);
    font-weight: 650;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--text-3);
  }
  .crumb + article {
    padding-top: 2px;
  }
  .empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 8px;
    height: 100%;
    text-align: center;
    color: var(--text-3);
  }
  .empty strong {
    color: var(--text-1);
  }

  @media (max-width: 760px) {
    .layout {
      grid-template-columns: minmax(0, 1fr);
    }
    nav {
      flex-direction: row;
      flex-wrap: wrap;
    }
    .search {
      width: 180px;
    }
  }
</style>
