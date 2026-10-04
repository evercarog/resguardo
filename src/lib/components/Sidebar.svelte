<script lang="ts">
  import { Activity, ChevronRight, CircleHelp, FolderSync, History, MonitorCog, MonitorSmartphone, Package, Plus, Settings } from "@lucide/svelte";
  import { setUi, ui } from "$lib/ui.svelte";
  import { health, statusOf } from "$lib/status.svelte";
  import type { Repo } from "$lib/api";
  import { runs } from "$lib/backups.svelte";
  import { COPY_LEVEL_LABEL, allCopies, copyStatus } from "$lib/copies.svelte";
  import type { Selection } from "$lib/nav";
  import { repoKind } from "$lib/repoKind";
  import Logo from "./Logo.svelte";
  import InboxBell from "./InboxBell.svelte";
  import InfoTip from "./InfoTip.svelte";
  import { agent, liveTask, offsiteSources, refreshAgent } from "$lib/agent.svelte";

  interface Props {
    repos: Repo[];
    selection: Selection;
    /** Versión de restic ("restic 0.19.1 compiled…"); vacía si no se pudo ejecutar. */
    version: string;
    /** Versión de Resguardo. */
    appVersion: string;
    onstatus: () => void;
    /** Abrir el historial de actividad. */
    onactivity: () => void;
    /** Todos los equipos de la cuenta de Resguardo Web. */
    onequipos: () => void;
    /** Equipos gestionados: solo si este equipo es Servidor de copias. */
    managed?: boolean;
    ongestionados?: () => void;
    onselectcopy: (repoId: string, planId: string) => void;
    onselectdestination: (repoId: string) => void;
    /** Abrir un destino (lugar). */
    onselectplace: (placeId: string) => void;
    onnewcopy: () => void;
    onadddestination: () => void;
    onappearance: () => void;
    /** Abrir el centro de ayuda (F1). */
    onhelp: () => void;
    /** Ir a donde lleva un aviso del buzón. */
    onnavigate: (sel: Selection) => void;
  }
  let {
    repos,
    selection,
    version,
    appVersion,
    onstatus,
    onactivity,
    onequipos,
    managed = false,
    ongestionados,
    onselectcopy,
    onselectdestination,
    onselectplace,
    onnewcopy,
    onadddestination,
    onappearance,
    onhelp,
    onnavigate,
  }: Props = $props();

  // "Ahora" avanza cada minuto para que los retrasos aparezcan solos.
  let now = $state(Date.now());
  // Mientras el agente sube, verifica o copia, se consulta a menudo para ver el avance.
  $effect(() => {
    const t = setInterval(() => {
      const busy = repos.some((r) => liveTask(r.id)) || !!agent.info?.state.running;
      if (busy && document.visibilityState === "visible") refreshAgent();
    }, 10_000);
    return () => clearInterval(t);
  });
  $effect(() => {
    const t = setInterval(() => (now = Date.now()), 60_000);
    return () => clearInterval(t);
  });

  const levels = $derived(Object.fromEntries(repos.map((r) => [r.id, statusOf(r, health[r.id], now).level])));
  const copies = $derived(allCopies(repos).map((c) => ({ ...c, status: copyStatus(c.repo, c.plan, now) })));
  const attention = $derived(
    Object.values(levels).filter((l) => l === "late" || l === "overdue" || l === "error").length +
      copies.filter((c) => c.status.level === "error" || c.status.level === "late").length +
      Object.keys(agent.info?.offsite_holds ?? {}).filter((id) => repos.some((r) => r.id === id)).length,
  );

  const statusActive = $derived(selection.kind === "status");
  const activityActive = $derived(selection.kind === "activity");
  const equiposActive = $derived(selection.kind === "equipos");
  const gestionadosActive = $derived(selection.kind === "gestionados");
  const isCopy = (repoId: string, planId: string) => selection.kind === "copy" && selection.repoId === repoId && selection.planId === planId;
  const isDest = (repoId: string) => selection.kind === "destination" && selection.repoId === repoId;

  const shortVersion = $derived(version.split(" ").slice(0, 2).join(" "));

  /** Destinos (lugares) con sus repositorios, en el orden en que aparecen. */
  const places = $derived.by(() => {
    const out: { id: string; name: string; repos: Repo[]; attention: boolean }[] = [];
    for (const r of repos) {
      const id = r.place_id ?? `repo:${r.id}`;
      let p = out.find((x) => x.id === id);
      if (!p) out.push((p = { id, name: r.place_name ?? repoKind(r.location).label, repos: [], attention: false }));
      p.repos.push(r);
      if (levels[r.id] === "overdue" || levels[r.id] === "error" || levels[r.id] === "late") p.attention = true;
    }
    return out;
  });

  function togglePlace(id: string) {
    setUi("foldedPlaces", ui.foldedPlaces.includes(id) ? ui.foldedPlaces.filter((x) => x !== id) : [...ui.foldedPlaces, id]);
  }
</script>

<aside>
  <div class="brand">
    <Logo size={26} />
    <div class="brand-name">Resguardo</div>
  </div>

  <nav aria-label="Secciones">
    <div class="top-nav">
      {#if repos.length}
        <button class="item" class:active={statusActive} aria-current={statusActive ? "page" : undefined} onclick={onstatus} title="Estado (Ctrl+1)">
          <span class="item-icon"><Activity size={16} /></span>
          <span class="item-name">Estado</span>
          {#if attention}<span class="count-badge" title="Copias o repositorios que necesitan atención"
              ><span class="sr-only">Necesitan atención:</span> {attention}</span
            >{/if}
        </button>
        <button class="item" class:active={activityActive} aria-current={activityActive ? "page" : undefined} onclick={onactivity} title="Actividad (Ctrl+2)">
          <span class="item-icon"><History size={16} /></span>
          <span class="item-name">Actividad</span>
        </button>
      {/if}
        <button class="item" class:active={equiposActive} aria-current={equiposActive ? "page" : undefined} onclick={onequipos} title="Todos los equipos de tu cuenta de Resguardo Web">
          <span class="item-icon"><MonitorSmartphone size={16} /></span>
          <span class="item-name">Todos mis equipos</span>
        </button>
        <button
          class="item"
          class:active={gestionadosActive}
          aria-current={gestionadosActive ? "page" : undefined}
          onclick={ongestionados}
          title={managed ? "Equipos con Resguardo Agente que administras desde aquí" : "Administra desde aquí los equipos con Resguardo Agente (requiere activar el Servidor de copias)"}
        >
          <span class="item-icon"><MonitorCog size={16} /></span>
          <span class="item-name">Equipos gestionados</span>
        </button>
    </div>

    <div class="section-label">
      <span id="nav-copies">Copias{#if copies.length}<span class="label-count num">{copies.length}</span>{/if}</span>
      <button class="icon-btn" title="Nueva copia (Ctrl+N)" aria-label="Nueva copia" onclick={onnewcopy}><Plus size={16} /></button>
    </div>
    <div class="list" role="group" aria-labelledby="nav-copies">
      {#each copies as { repo, plan, status } (`${repo.id}#${plan.id}`)}
        {@const run = runs[repo.id]}
        {@const active = isCopy(repo.id, plan.id)}
        <button
          class="item"
          class:active
          aria-current={active ? "page" : undefined}
          title="{plan.name} · se guarda en «{repo.name}»{status.level !== 'loading' ? ` · ${COPY_LEVEL_LABEL[status.level]}` : ''}"
          onclick={() => onselectcopy(repo.id, plan.id)}
        >
          <span class="item-icon"><FolderSync size={16} /></span>
          <span class="item-name">{plan.name}</span>
          {#if status.manualRunning && run}
            <span class="live num">{Math.round((run.status?.percent ?? 0) * 100)} %</span>
          {:else if status.agentRunning?.percent != null}
            <span class="live num">{Math.round(status.agentRunning.percent * 100)} %</span>
          {/if}
          {#if status.level === "running"}
            <span class="pulse" role="img" aria-label="Copia en curso"></span>
          {:else if status.level !== "loading"}
            <span class="dot-state lvl-{status.level}" role="img" aria-label={COPY_LEVEL_LABEL[status.level]} title={COPY_LEVEL_LABEL[status.level]}></span>
          {/if}
        </button>
      {:else}
        {#if repos.length}
          <button class="empty-hint" onclick={onnewcopy}>
            <Plus size={15} />
            <span>Crea tu primera copia</span>
          </button>
        {:else}
          <!-- Sin destinos no hay copias posibles: basta con una pista (el botón está en «Destinos»). -->
          <p class="empty-note">Aquí aparecerán tus copias.</p>
        {/if}
      {/each}
    </div>

    <div class="section-label">
      <span id="nav-dests">Destinos{#if places.length}<span class="label-count num">{places.length}</span>{/if} <InfoTip id="destino" /></span>
      <button class="icon-btn" title="Añadir repositorio (Ctrl+Mayús+N)" aria-label="Añadir repositorio" onclick={onadddestination}><Plus size={16} /></button>
    </div>
    <div class="list" role="group" aria-labelledby="nav-dests">
      {#each places as place (place.id)}
        {@const folded = ui.foldedPlaces.includes(place.id)}
        {@const PlaceIcon = repoKind(place.repos[0].location).icon}
        {@const placeActive = selection.kind === "place" && selection.placeId === place.id}
        <div class="place-row">
          <button
            class="chev-btn"
            aria-expanded={!folded}
            aria-label="{folded ? 'Desplegar' : 'Plegar'} «{place.name}»"
            title={folded ? "Ver sus repositorios" : "Plegar"}
            onclick={() => togglePlace(place.id)}
          >
            <span class="chev" class:open={!folded}><ChevronRight size={13} /></span>
          </button>
          <button
            class="item place"
            class:active={placeActive}
            aria-current={placeActive ? "page" : undefined}
            title="Destino «{place.name}» · {place.repos.length} {place.repos.length === 1 ? 'repositorio' : 'repositorios'}"
            onclick={() => onselectplace(place.id)}
          >
            <span class="item-icon"><PlaceIcon size={16} /></span>
            <span class="item-name">{place.name}</span>
            {#if folded && place.attention}<span class="dot-state lvl-error" role="img" aria-label="Algún repositorio necesita atención"></span>{/if}
          </button>
        </div>
        {#if !folded}
          <div class="nested" role="group" aria-label="Repositorios de «{place.name}»">
            {#each place.repos as repo (repo.id)}
            {@const kind = repoKind(repo.location)}
            {@const run = runs[repo.id]}
            {@const active = isDest(repo.id)}
            <button
              class="item"
              class:active
              aria-current={active ? "page" : undefined}
              title="Repositorio «{repo.name}» · {kind.label} · {levels[repo.id] === 'paused'
                ? 'en pausa'
                : !repo.plans.length && offsiteSources(repo.id).length
                  ? `recibe de ${offsiteSources(repo.id).map((s) => s.name).join(', ')}`
                  : `${repo.plans.length} ${repo.plans.length === 1 ? 'copia' : 'copias'}`}"
              onclick={() => onselectdestination(repo.id)}
            >
              <span class="item-icon"><Package size={15} /></span>
              <span class="item-name">{repo.name}</span>
              {#if run?.running}
                <span class="live num" title="Copiando">{Math.round((run.status?.percent ?? 0) * 100)} %</span>
              {:else if liveTask(repo.id)}
                {@const task = liveTask(repo.id)!}
                <span class="live num" title={task.kind === "offsite" ? "Subiendo a la nube" : "Verificando"}>
                  {task.percent != null ? `${Math.floor(task.percent * 100)} %` : task.kind === "offsite" ? "Subiendo" : "Verificando"}
                </span>
              {:else if repo.plans.length}
                <span class="count num" aria-label="{repo.plans.length} {repo.plans.length === 1 ? 'copia' : 'copias'}">{repo.plans.length}</span>
              {/if}
              {#if run?.running}
                <span class="pulse" role="img" aria-label="Copia en curso"></span>
              {:else if liveTask(repo.id)}
                <span class="pulse" role="img" aria-label={liveTask(repo.id)?.kind === "offsite" ? "Subida a la nube en curso" : "Verificación en curso"}></span>
              {:else if levels[repo.id] === "overdue" || levels[repo.id] === "error"}
                <span class="dot-state lvl-error" role="img" aria-label="Atrasado o sin conexión" title="Atrasado o sin conexión"></span>
              {:else if levels[repo.id] === "late"}
                <span class="dot-state lvl-late" role="img" aria-label="Con retraso" title="Con retraso"></span>
              {:else if levels[repo.id] === "paused"}
                <span class="dot-state lvl-paused" role="img" aria-label="Copias automáticas en pausa" title="Copias automáticas en pausa"></span>
              {/if}
            </button>
            {/each}
          </div>
        {/if}
      {:else}
        <button class="empty-hint" onclick={onadddestination}>
          <Plus size={15} />
          <span>Añade tu primer repositorio</span>
        </button>
      {/each}
    </div>
  </nav>

  <div class="foot">
    <span class="dot" class:ok={!!version}></span>
    <span class="version">
      {#if appVersion}<span class="app-version">Resguardo v{appVersion}</span>{/if}
      <span title={version || "No se pudo ejecutar el restic incluido. Reinstala Resguardo."}>{version ? shortVersion : "restic no disponible"}</span>
    </span>
    <span class="foot-actions">
      <InboxBell {repos} {onnavigate} />
      <button class="icon-btn" title="Ayuda (F1)" aria-label="Ayuda" onclick={onhelp}><CircleHelp size={16} /></button>
      <button class="icon-btn" class:on={selection.kind === "ajustes"} title="Ajustes (Ctrl+,)" aria-label="Ajustes" aria-current={selection.kind === "ajustes" ? "page" : undefined} onclick={onappearance}><Settings size={16} /></button>
    </span>
  </div>
</aside>

<style>
  .place-row {
    display: flex;
    align-items: center;
  }
  .place-row .place {
    flex: 1;
    min-width: 0;
  }
  .chev-btn {
    display: grid;
    place-items: center;
    flex: none;
    width: 18px;
    height: 28px;
    margin-right: -2px;
    padding: 0;
    color: var(--text-3);
    background: none;
    border: none;
    border-radius: 4px;
    cursor: pointer;
  }
  .chev-btn:hover {
    color: var(--text-1);
  }
  .chev {
    display: grid;
    transition: transform 0.15s;
  }
  .chev.open {
    transform: rotate(90deg);
  }
  .place .item-name {
    font-weight: 550;
  }
  .nested {
    display: flex;
    flex-direction: column;
    gap: 1px;
    margin-left: 14px;
    padding-left: 6px;
    border-left: 1px solid var(--border);
  }
  .icon-btn.on {
    color: var(--text-1);
    background: var(--surface-2);
  }
  /* Barra lateral al estilo de Linear: fondo hundido, filas de una línea,
   * estado activo sutil (sin barras de color), puntos de estado y contadores. */
  aside {
    display: flex;
    flex-direction: column;
    min-height: 0;
    background: var(--bg-subtle);
    border-right: 1px solid var(--border);
  }
  .brand {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 18px 18px 14px;
  }
  .brand-name {
    font-weight: 650;
    font-size: var(--fs-h2);
    letter-spacing: -0.01em;
  }
  nav {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding-bottom: var(--sp-3);
  }
  .section-label {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 14px 12px 4px 20px;
    font-size: var(--fs-overline);
    font-weight: 600;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--text-3);
  }
  .section-label .icon-btn {
    width: 24px;
    height: 24px;
  }
  .label-count {
    margin-left: 6px;
    font-weight: 500;
    letter-spacing: 0;
    opacity: 0.8;
  }
  .list,
  .top-nav {
    display: flex;
    flex-direction: column;
    gap: 1px;
    padding: 0 10px;
  }
  .top-nav {
    padding-bottom: 4px;
  }
  .item {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    height: 32px;
    padding: 0 10px;
    font: inherit;
    font-size: var(--fs-sm);
    color: var(--text-2);
    text-align: left;
    background: transparent;
    border: none;
    border-radius: var(--radius-sm);
    cursor: pointer;
    transition:
      background var(--dur-fast) var(--ease),
      color var(--dur-fast) var(--ease);
  }
  .item:hover {
    color: var(--text-1);
    background: color-mix(in srgb, var(--text-1) 4%, transparent);
  }
  .item.active {
    color: var(--text-1);
    font-weight: 500;
    background: color-mix(in srgb, var(--text-1) 7%, transparent);
  }
  .item-icon {
    display: grid;
    place-items: center;
    flex: none;
    color: var(--text-3);
  }
  .item.active .item-icon {
    color: var(--text-1);
  }
  .item-name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .live {
    flex: none;
    font-size: var(--fs-xs);
    font-weight: 500;
    color: var(--accent-text);
  }
  .count {
    flex: none;
    font-size: var(--fs-xs);
    color: var(--text-3);
  }
  .pulse {
    flex: none;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--accent);
    box-shadow: 0 0 0 0 color-mix(in srgb, var(--accent) 60%, transparent);
    animation: pulse 1.6s ease-out infinite;
  }
  @keyframes pulse {
    to {
      box-shadow: 0 0 0 6px transparent;
    }
  }
  .count-badge {
    min-width: 18px;
    padding: 0 5px;
    font-size: var(--fs-overline);
    font-weight: 600;
    line-height: 18px;
    text-align: center;
    border-radius: 999px;
    color: var(--bad);
    background: var(--bad-soft);
    font-variant-numeric: tabular-nums;
  }
  .dot-state {
    flex: none;
    width: 7px;
    height: 7px;
    border-radius: 50%;
  }
  .lvl-ok {
    background: var(--ok);
  }
  .lvl-late,
  .lvl-warning {
    background: var(--warn);
  }
  .lvl-error {
    background: var(--bad);
  }
  .lvl-never {
    box-shadow: inset 0 0 0 1.5px var(--border-strong);
  }
  /* En pausa: dos barras (neutro, no es un aviso). */
  .lvl-paused {
    border-radius: 1px;
    background: linear-gradient(to right, var(--paused) 0 35%, transparent 35% 65%, var(--paused) 65%);
  }
  .empty-hint {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 4px 0;
    padding: 9px 10px;
    font: inherit;
    font-size: var(--fs-sm);
    color: var(--text-3);
    background: transparent;
    border: 1px dashed var(--border-strong);
    border-radius: var(--radius);
    cursor: pointer;
    transition:
      color var(--dur-fast) var(--ease),
      border-color var(--dur-fast) var(--ease);
  }
  .empty-hint:hover {
    color: var(--accent-text);
    border-color: var(--accent);
  }
  .empty-note {
    margin: 2px 0 4px;
    padding: 0 10px;
    font-size: var(--fs-xs);
    color: var(--text-3);
  }
  .foot {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 12px 18px;
    border-top: 1px solid var(--border);
    font-size: var(--fs-xs);
    color: var(--text-3);
  }
  .version {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-width: 0;
    line-height: 1.35;
  }
  .version > span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .app-version {
    color: var(--text-2);
    font-weight: 500;
  }
  .foot-actions {
    display: flex;
    gap: 2px;
    margin: -6px -8px -6px 0;
  }
  .dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--bad);
  }
  .dot.ok {
    background: var(--ok);
  }
</style>
