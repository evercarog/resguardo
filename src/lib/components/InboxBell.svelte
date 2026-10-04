<script lang="ts">
  import { onMount, tick } from "svelte";
  import { fly } from "svelte/transition";
  import { Bell, BellOff, CheckCheck, CircleAlert, CircleCheck, Info, TriangleAlert, X } from "@lucide/svelte";
  import type { Repo } from "$lib/api";
  import type { Selection } from "$lib/nav";
  import { pollActivity } from "$lib/activity.svelte";
  import { formatDate, formatRelative } from "$lib/format";
  import { clearInbox, inbox, inboxItems, inboxUi, markRead, noteSeen, type InboxItem } from "$lib/inbox.svelte";
  import { openKit } from "$lib/kit.svelte";
  import { dur } from "$lib/motion";

  // La campana del buzón de avisos (en la barra lateral) y su panel.
  interface Props {
    repos: Repo[];
    onnavigate: (sel: Selection) => void;
  }
  let { repos, onnavigate }: Props = $props();

  let now = $state(Date.now());
  onMount(() => {
    const stop = pollActivity(60_000);
    const t = setInterval(() => (now = Date.now()), 60_000);
    return () => {
      stop();
      clearInterval(t);
    };
  });

  const items = $derived(inboxItems(repos, now));
  const readSet = $derived(new Set(inbox.read));
  const unread = $derived(items.filter((i) => !readSet.has(i.id)).length);
  $effect(() => noteSeen(items));

  let button = $state<HTMLButtonElement>();
  let panel = $state<HTMLDivElement>();
  let pos = $state({ left: 0, bottom: 0 });

  async function toggle() {
    if (inboxUi.open) return close();
    const r = button!.getBoundingClientRect();
    // Pegado al borde izquierdo de la barra lateral, encima de la campana.
    pos = { left: 12, bottom: window.innerHeight - r.top + 8 };
    inboxUi.open = true;
    await tick();
    panel?.focus();
  }
  function close(focusBack = true) {
    inboxUi.open = false;
    if (focusBack) button?.focus();
  }

  function act(it: InboxItem) {
    markRead([it.id]);
    close(false);
    if (it.action.kit !== undefined) openKit(it.action.kit);
    else if (it.action.sel) onnavigate(it.action.sel);
  }

  function onwindow(e: Event) {
    if (!inboxUi.open) return;
    if (e.type === "keydown") {
      if ((e as KeyboardEvent).key === "Escape") {
        e.stopPropagation();
        close();
      }
      return;
    }
    if (panel?.contains(e.target as Node) || button?.contains(e.target as Node)) return;
    close(false);
  }

  const ICON = { bad: CircleAlert, warn: TriangleAlert, ok: CircleCheck, info: Info };
</script>

<svelte:window onkeydowncapture={onwindow} onpointerdown={onwindow} />

<button
  bind:this={button}
  class="icon-btn bell"
  class:on={inboxUi.open}
  title={unread ? `Avisos: ${unread} sin leer` : "Avisos"}
  aria-label={unread ? `Avisos, ${unread} sin leer` : "Avisos"}
  aria-expanded={inboxUi.open}
  aria-haspopup="dialog"
  onclick={toggle}
>
  <Bell size={16} />
  {#if unread}<span class="count" aria-hidden="true">{unread > 9 ? "9+" : unread}</span>{/if}
</button>

{#if inboxUi.open}
  <div
    bind:this={panel}
    class="panel"
    role="dialog"
    aria-label="Avisos"
    tabindex="-1"
    style:left="{pos.left}px"
    style:bottom="{pos.bottom}px"
    transition:fly={{ y: 8, duration: dur(150) }}
  >
    <header>
      <strong>Avisos</strong>
      {#if unread}<span class="faint">{unread} sin leer</span>{/if}
      <span class="spacer"></span>
      {#if unread}
        <button class="icon-btn" title="Marcar todo como leído" aria-label="Marcar todo como leído" onclick={() => markRead(items.map((i) => i.id))}><CheckCheck size={15} /></button>
      {/if}
      <button class="icon-btn" title="Cerrar (Escape)" aria-label="Cerrar los avisos" onclick={() => close()}><X size={15} /></button>
    </header>
    {#if items.length}
      <ul class="items">
        {#each items as it (it.id)}
          {@const Icon = ICON[it.tone]}
          {@const isNew = !readSet.has(it.id)}
          <li class="item tone-{it.tone}" class:new={isNew}>
            <span class="ic"><Icon size={15} /></span>
            <div class="txt">
              <span class="title">{it.title}</span>
              <span class="text">{it.text}</span>
              <span class="meta">
                <span title={formatDate(it.time)}>{it.ongoing ? `desde ${formatRelative(it.time)}` : formatRelative(it.time)}</span>
                <button class="link" onclick={() => act(it)}>{it.action.label}</button>
                {#if isNew}<button class="link quiet" onclick={() => markRead([it.id])}>Marcar como leído</button>{/if}
              </span>
            </div>
            {#if isNew}<span class="dot" title="Sin leer"></span>{/if}
          </li>
        {/each}
      </ul>
      <footer>
        <button class="link quiet" onclick={() => clearInbox(items)} title="Deja de mostrar lo que ya pasó (lo que sigue pasando se queda)">Vaciar</button>
      </footer>
    {:else}
      <div class="empty">
        <BellOff size={22} />
        <p>Nada nuevo. Aquí verás las copias que fallen, las subidas frenadas y lo que necesite tu atención.</p>
      </div>
    {/if}
  </div>
{/if}

<style>
  .bell {
    position: relative;
  }
  .count {
    position: absolute;
    top: -3px;
    right: -4px;
    min-width: 16px;
    height: 16px;
    padding: 0 4px;
    font-size: 10px;
    font-weight: 700;
    line-height: 16px;
    text-align: center;
    color: var(--bad-contrast);
    background: var(--bad);
    border-radius: 999px;
  }
  .panel {
    position: fixed;
    z-index: 30;
    display: flex;
    flex-direction: column;
    width: min(380px, calc(100vw - 24px));
    max-height: min(560px, calc(100vh - 80px));
    background: var(--surface);
    border: 1px solid var(--border-strong);
    border-radius: calc(var(--radius) + 2px);
    box-shadow: var(--shadow-lg);
  }
  .panel:focus {
    outline: none;
  }
  header,
  footer {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 10px 12px;
  }
  header {
    border-bottom: 1px solid var(--border);
  }
  header .faint {
    font-size: var(--fs-sm);
  }
  footer {
    justify-content: flex-end;
    border-top: 1px solid var(--border);
  }
  .spacer {
    flex: 1;
  }
  .items {
    flex: 1;
    overflow: auto;
    margin: 0;
    padding: 4px 0;
    list-style: none;
  }
  .item {
    position: relative;
    display: flex;
    gap: 10px;
    padding: 10px 14px;
  }
  .item + .item {
    border-top: 1px solid var(--border);
  }
  .item.new {
    background: color-mix(in srgb, var(--accent) 4%, transparent);
  }
  .ic {
    display: grid;
    flex: none;
    margin-top: 2px;
    color: var(--text-3);
  }
  .tone-bad .ic {
    color: var(--bad);
  }
  .tone-warn .ic {
    color: var(--warn);
  }
  .tone-ok .ic {
    color: var(--ok);
  }
  .tone-info .ic {
    color: var(--info);
  }
  .txt {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
    font-size: var(--fs-sm);
  }
  .title {
    font-weight: 600;
  }
  .new .title {
    font-weight: 700;
  }
  .text {
    color: var(--text-2);
    display: -webkit-box;
    -webkit-line-clamp: 3;
    line-clamp: 3;
    -webkit-box-orient: vertical;
    overflow: hidden;
    overflow-wrap: anywhere;
  }
  .meta {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 10px;
    margin-top: 2px;
    color: var(--text-3);
  }
  .quiet {
    color: var(--text-3);
    font-weight: 450;
    font-size: var(--fs-xs);
  }
  .dot {
    position: absolute;
    top: 14px;
    right: 12px;
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--accent);
  }
  .empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    padding: 26px 22px;
    text-align: center;
    color: var(--text-3);
  }
  .empty p {
    margin: 0;
    font-size: var(--fs-sm);
  }
</style>
