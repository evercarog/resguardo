<script lang="ts">
  // «Ir a…» (Ctrl+K): encuentra todo. Destinos, copias y secciones; acciones
  // (copiar ahora, pausar, kit, restaurar, buscar un archivo, verificar…);
  // ajustes (por su nombre y por sinónimos) y apartados de la ayuda. Agrupado
  // y con el teclado: ↑ ↓ e Intro.
  import type { Component } from "svelte";
  import {
    Activity,
    ArchiveRestore,
    BookOpen,
    CirclePause,
    CirclePlay,
    CloudUpload,
    FileSearch,
    FolderPlus,
    FolderSync,
    Package,
    Share2,
    HardDrive,
    History,
    KeyRound,
    LayoutDashboard,
    MonitorCog,
    MonitorSmartphone,
    Play,
    Plus,
    Search,
    Settings,
    ShieldCheck,
    Sparkles,
  } from "@lucide/svelte";
  import * as api from "$lib/api";
  import type { Repo } from "$lib/api";
  import type { Selection } from "$lib/nav";
  import { agent } from "$lib/agent.svelte";
  import { startBackup } from "$lib/backups.svelte";
  import { allCopies } from "$lib/copies.svelte";
  import { openHelp } from "$lib/help.svelte";
  import { HELP, normalize } from "$lib/helpContent";
  import { pendingEditor } from "$lib/intent.svelte";
  import { openKit } from "$lib/kit.svelte";
  import { openNews } from "$lib/news.svelte";
  import { pauseOf } from "$lib/pause.svelte";
  import { searchUi } from "$lib/search.svelte";
  import { openSettings, type SettingsSection } from "$lib/settingsNav.svelte";
  import { toast } from "$lib/toast.svelte";
  import Modal from "./Modal.svelte";

  interface Props {
    repos: Repo[];
    onpick: (sel: Selection) => void;
    onclose: () => void;
    /** «Nueva copia» y «Añadir destino» (los abre la página). */
    onnewcopy?: () => void;
    onadddestination?: () => void;
    /** «Usar un destino compartido» por otro equipo. */
    onshared?: () => void;
  }
  let { repos, onpick, onclose, onnewcopy, onadddestination, onshared }: Props = $props();

  type Group = "Ir a" | "Copias" | "Repositorios" | "Destinos" | "Acciones" | "Ajustes" | "Ayuda";
  const GROUP_ORDER: Group[] = ["Ir a", "Copias", "Repositorios", "Destinos", "Acciones", "Ajustes", "Ayuda"];
  /** Con algo escrito, como mucho tantos por grupo (rápido y legible). */
  const PER_GROUP = 7;

  interface Item {
    key: string;
    group: Group;
    label: string;
    detail: string;
    icon: Component<{ size?: number }>;
    /** Sinónimos y palabras con las que también se encuentra. */
    keywords?: string;
    /** Texto largo en el que buscar (la ayuda). */
    body?: string;
    run: () => void;
  }

  const go = (sel: Selection) => () => onpick(sel);
  const dest = (r: Repo) => ({ kind: "destination", repoId: r.id }) as Selection;

  // ---------- Lo que se puede encontrar ----------

  const SETTINGS: { section: SettingsSection; label: string; keywords: string }[] = [
    { section: "general", label: "Apariencia: modo claro u oscuro y color", keywords: "tema oscuro claro color acento apariencia aspecto" },
    { section: "general", label: "Atajos de teclado", keywords: "teclado atajos combinaciones teclas" },
    { section: "general", label: "Acerca de y novedades", keywords: "version novedades cambios acerca" },
    { section: "equipo", label: "Modo discreto", keywords: "prioridad baja lento discreto trabajar horario oficina limitar subida ancho de banda" },
    { section: "equipo", label: "Copias a distancia", keywords: "remoto distancia web copiar desde otro equipo permitir" },
    { section: "equipo", label: "Resguardo Web", keywords: "web panel vincular desvincular nube informe" },
    { section: "equipo", label: "Agente de copias y su registro", keywords: "agente tarea programada registro log reparar servicio" },
    { section: "bandeja", label: "Bandeja, inicio con Windows y avisos", keywords: "bandeja notificaciones avisos iniciar con windows arranque cerrar minimizar" },
    { section: "explorador", label: "«Ver versiones» en el Explorador", keywords: "clic derecho menu contextual explorador archivos versiones" },
    { section: "seguridad", label: "Bloqueo con Windows Hello", keywords: "bloqueo pin huella hello contraseña bloquear app seguridad" },
    { section: "seguridad", label: "Kit de recuperación", keywords: "kit recuperacion contraseña perder equipo imprimir" },
    { section: "seguridad", label: "Cuenta de «Todos mis equipos»", keywords: "cuenta sesion iniciar cerrar equipos login" },
  ];

  function runTask(r: Repo, kind: "verify" | "offsite") {
    return async () => {
      onpick(dest(r));
      try {
        await api.agentTaskNow(r.id, kind);
        toast(kind === "verify" ? `El agente empezará a verificar «${r.name}» en unos minutos.` : `El agente empezará a subir «${r.name}» en unos minutos.`, "info");
      } catch (e) {
        toast(String(e), "error");
      }
    };
  }

  const items = $derived.by<Item[]>(() => {
    const out: Item[] = [
      { key: "status", group: "Ir a", label: "Estado", detail: "Resumen de todos los repositorios", icon: LayoutDashboard, keywords: "inicio resumen panel", run: go({ kind: "status" }) },
      { key: "activity", group: "Ir a", label: "Actividad", detail: "Historial de copias y tareas", icon: Activity, keywords: "historial registro", run: go({ kind: "activity" }) },
      { key: "equipos", group: "Ir a", label: "Todos mis equipos", detail: "Los demás equipos de tu cuenta", icon: MonitorSmartphone, run: go({ kind: "equipos" }) },
      { key: "gestionados", group: "Ir a", label: "Equipos gestionados", detail: "Los equipos con Resguardo Agente que administras", icon: MonitorCog, run: go({ kind: "gestionados" }) },
      { key: "ajustes", group: "Ir a", label: "Ajustes", detail: "Apariencia, este equipo, bandeja, seguridad…", icon: Settings, keywords: "configuracion preferencias opciones", run: go({ kind: "ajustes" }) },
    ];
    for (const { repo, plan } of allCopies(repos)) {
      out.push({ key: `copy:${repo.id}:${plan.id}`, group: "Copias", label: plan.name, detail: `Copia · en «${repo.name}»`, icon: FolderSync, run: go({ kind: "copy", repoId: repo.id, planId: plan.id }) });
    }
    for (const r of repos) out.push({ key: `dest:${r.id}`, group: "Repositorios", label: r.name, detail: r.place_name ? `Repositorio · en «${r.place_name}»` : "Repositorio", icon: Package, run: go(dest(r)) });

    // Destinos (lugares).
    for (const r of repos) {
      if (!r.place_id || out.some((it) => it.key === `place:${r.place_id}`)) continue;
      const n = repos.filter((x) => x.place_id === r.place_id).length;
      const placeId = r.place_id;
      out.push({ key: `place:${placeId}`, group: "Destinos", label: r.place_name ?? "Destino", detail: `Destino · ${n} ${n === 1 ? "repositorio" : "repositorios"}`, icon: HardDrive, keywords: "destino lugar", run: go({ kind: "place", placeId }) });
    }

    // Acciones.
    if (onnewcopy) out.push({ key: "new-copy", group: "Acciones", label: "Nueva copia", detail: "Ctrl+N", icon: Plus, keywords: "crear copia plan", run: onnewcopy });
    if (onadddestination)
      out.push({ key: "add-dest", group: "Acciones", label: "Añadir repositorio", detail: "Ctrl+Mayús+N", icon: FolderPlus, keywords: "nuevo repositorio disco servidor nube conectar", run: onadddestination });
    if (onshared)
      out.push({ key: "shared", group: "Acciones", label: "Usar un destino compartido", detail: "Por otro equipo tuyo", icon: Share2, keywords: "compartir compartido nube cuenta otro equipo", run: onshared });
    out.push({ key: "kit-all", group: "Acciones", label: "Preparar el kit de recuperación", detail: "Todos los repositorios", icon: KeyRound, keywords: "kit recuperacion imprimir", run: () => openKit(null) });
    out.push({ key: "news", group: "Acciones", label: "Ver las novedades", detail: "Qué hay de nuevo en Resguardo", icon: Sparkles, keywords: "novedades version cambios", run: openNews });
    for (const { repo, plan } of allCopies(repos)) {
      out.push({
        key: `run:${repo.id}:${plan.id}`,
        group: "Acciones",
        label: `Copiar ahora «${plan.name}»`,
        detail: `En «${repo.name}»`,
        icon: Play,
        keywords: "copiar ahora hacer copia backup ya",
        run: () => {
          onpick({ kind: "copy", repoId: repo.id, planId: plan.id });
          void startBackup(repo.id, repo.name, plan.id, plan.name);
        },
      });
    }
    for (const r of repos) {
      const a = agent.info?.repos.find((x) => x.id === r.id);
      out.push({
        key: `restore:${r.id}`,
        group: "Acciones",
        label: `Restaurar archivos de «${r.name}»`,
        detail: "Recuperar de una versión",
        icon: ArchiveRestore,
        keywords: "restaurar recuperar volver version anterior",
        run: () => ((pendingEditor.restore = r.id), onpick(dest(r))),
      });
      out.push({
        key: `search:${r.id}`,
        group: "Acciones",
        label: `Buscar un archivo en «${r.name}»…`,
        detail: "Cuándo existió y dónde está",
        icon: FileSearch,
        keywords: "buscar encontrar archivo fichero",
        run: () => (onpick(dest(r)), (searchUi.open = r.id)),
      });
      out.push({
        key: `kit:${r.id}`,
        group: "Acciones",
        label: `Kit de recuperación de «${r.name}»`,
        detail: "Imprimir o guardar",
        icon: KeyRound,
        keywords: "kit recuperacion contraseña",
        run: () => openKit([r.id]),
      });
      out.push({
        key: `improve:${r.id}`,
        group: "Acciones",
        label: `Mejorar la protección de «${r.name}»`,
        detail: "Asistente paso a paso",
        icon: ShieldCheck,
        keywords: "proteccion salud mejorar asistente seguridad",
        run: () => ((pendingEditor.improve = r.id), onpick(dest(r))),
      });
      out.push({
        key: `history:${r.id}`,
        group: "Acciones",
        label: `Historia de «${r.name}»`,
        detail: "Todo lo que le ha pasado",
        icon: History,
        keywords: "historia historial cambios registro quien cambio",
        run: () => ((pendingEditor.historyTab = r.id), onpick(dest(r))),
      });
      if (a && a.schedule.kind !== "monitor") {
        const paused = !!pauseOf(r.id);
        out.push(
          paused
            ? {
                key: `resume:${r.id}`,
                group: "Acciones",
                label: `Reanudar las copias automáticas de «${r.name}»`,
                detail: "Están en pausa",
                icon: CirclePlay,
                keywords: "reanudar continuar pausa",
                run: () => ((pendingEditor.resume = r.id), onpick(dest(r))),
              }
            : {
                key: `pause:${r.id}`,
                group: "Acciones",
                label: `Pausar las copias automáticas de «${r.name}»`,
                detail: "Por un tiempo",
                icon: CirclePause,
                keywords: "pausar parar detener suspender mantenimiento",
                run: () => ((pendingEditor.pause = r.id), onpick(dest(r))),
              },
        );
      }
      if (a?.verify)
        out.push({ key: `verify:${r.id}`, group: "Acciones", label: `Verificar ahora «${r.name}»`, detail: "Comprobar que está sano", icon: ShieldCheck, keywords: "verificar comprobar check", run: runTask(r, "verify") });
      if (a?.offsite)
        out.push({ key: `offsite:${r.id}`, group: "Acciones", label: `Subir ahora la copia externa de «${r.name}»`, detail: "A la nube u otro disco", icon: CloudUpload, keywords: "subir nube copia externa offsite", run: runTask(r, "offsite") });
    }

    // Ajustes.
    for (const s of SETTINGS)
      out.push({ key: `set:${s.label}`, group: "Ajustes", label: s.label, detail: "Ajustes", icon: Settings, keywords: s.keywords, run: () => openSettings(s.section) });

    // Ayuda.
    for (const section of HELP)
      for (const it of section.items)
        out.push({ key: `help:${it.id}`, group: "Ayuda", label: it.title, detail: section.title, icon: BookOpen, body: it.html, run: () => openHelp(it.id) });
    return out;
  });

  /** Sin tildes ni mayúsculas: «verificacion» encuentra «Verificación». */
  const norm = (s: string) => normalize(s);
  /** Palabras que no ayudan a buscar («qué es la retención» → «retención»). */
  const STOP = new Set(["que", "es", "la", "el", "los", "las", "de", "del", "un", "una", "como", "se", "y", "o", "a", "en", "por", "para", "mi", "mis", "cual", "hay", "esta", "esto", "lo"]);
  // Índice de búsqueda (una vez por apertura).
  const index = $derived(items.map((it) => ({ it, head: norm(`${it.label} ${it.detail} ${it.keywords ?? ""}`), body: it.body ? norm(it.body) : "" })));

  let query = $state("");
  let active = $state(0);

  const groups = $derived.by(() => {
    const all = norm(query).split(/\s+/).filter(Boolean);
    const words = all.filter((w) => !STOP.has(w));
    let list: Item[];
    if (!all.length) {
      // Sin escribir: ir a, copias y destinos (como siempre).
      list = items.filter((it) => it.group === "Ir a" || it.group === "Copias" || it.group === "Repositorios" || it.group === "Destinos");
    } else {
      const ws = words.length ? words : all;
      const scored: { it: Item; r: number; i: number }[] = [];
      index.forEach(({ it, head, body }, i) => {
        if (ws.every((w) => head.includes(w))) {
          const label = norm(it.label);
          scored.push({ it, i, r: label.startsWith(ws[0]) ? 0 : ws.every((w) => label.includes(w)) ? 1 : 2 });
        } else if (body && ws.every((w) => head.includes(w) || body.includes(w))) scored.push({ it, i, r: 3 });
      });
      list = scored.sort((a, b) => a.r - b.r || a.i - b.i).map((x) => x.it);
    }
    const out: { group: Group; items: Item[] }[] = [];
    for (const g of GROUP_ORDER) {
      const inGroup = list.filter((it) => it.group === g);
      if (inGroup.length) out.push({ group: g, items: all.length ? inGroup.slice(0, PER_GROUP) : inGroup });
    }
    return out;
  });
  const flat = $derived(groups.flatMap((g) => g.items));
  $effect(() => {
    void query;
    active = 0;
  });

  function pick(it: Item | undefined) {
    if (!it) return;
    onclose();
    it.run();
  }

  function onkeydown(e: KeyboardEvent) {
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      if (!flat.length) return;
      active = (active + (e.key === "ArrowDown" ? 1 : flat.length - 1)) % flat.length;
      document.getElementById(`qs-${active}`)?.scrollIntoView({ block: "nearest" });
    } else if (e.key === "Enter") {
      e.preventDefault();
      pick(flat[active]);
    }
  }
</script>

<Modal {onclose} labelledby="qs-title" width={580}>
  <h2 id="qs-title" class="sr-only">Buscar en Resguardo</h2>
  <div class="search">
    <Search size={16} />
    <input
      class="input"
      type="text"
      placeholder="Buscar un repositorio, una copia, una acción, un ajuste o en la ayuda…"
      aria-label="Buscar"
      role="combobox"
      aria-expanded="true"
      aria-controls="qs-list"
      aria-activedescendant={flat.length ? `qs-${active}` : undefined}
      autocomplete="off"
      spellcheck="false"
      bind:value={query}
      {onkeydown}
    />
  </div>
  {#if flat.length}
    <div id="qs-list" class="list" role="listbox" aria-label="Resultados">
      {#each groups as g (g.group)}
        <div class="group" role="group" aria-label={g.group}>
          <div class="gtitle" aria-hidden="true">{g.group}</div>
          {#each g.items as it (it.key)}
            {@const i = flat.indexOf(it)}
            <!-- El teclado se maneja desde el campo de búsqueda (aria-activedescendant). -->
            <!-- svelte-ignore a11y_click_events_have_key_events -->
            <div id="qs-{i}" role="option" tabindex="-1" aria-selected={i === active} class="opt" class:on={i === active} onclick={() => pick(it)} onmousemove={() => (active = i)}>
              <span class="ic" aria-hidden="true"><it.icon size={15} /></span>
              <span class="label">{it.label}</span>
              <span class="detail faint">{it.detail}</span>
            </div>
          {/each}
        </div>
      {/each}
    </div>
  {:else}
    <p class="none faint">Nada coincide con «{query}». Prueba con otra palabra o pulsa F1 para abrir la ayuda.</p>
  {/if}
  <p class="keys faint"><kbd>↑</kbd> <kbd>↓</kbd> para moverte · <kbd>Intro</kbd> para abrir · <kbd>Esc</kbd> para cerrar</p>
</Modal>

<style>
  .search {
    position: relative;
    display: flex;
    align-items: center;
    color: var(--text-3);
  }
  .search :global(svg) {
    position: absolute;
    left: 11px;
    pointer-events: none;
  }
  .search .input {
    padding-left: 34px;
    height: 40px;
    font-size: var(--fs-body);
  }
  .list {
    margin: 10px 0 0;
    padding: 0;
    max-height: min(420px, 60vh);
    overflow-y: auto;
  }
  .group + .group {
    margin-top: 6px;
  }
  .gtitle {
    position: sticky;
    top: 0;
    z-index: 1;
    padding: 4px 10px;
    font-size: var(--fs-xs);
    font-weight: 650;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--text-3);
    background: var(--surface);
  }
  .opt {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 10px;
    border-radius: var(--radius-sm);
    cursor: pointer;
    min-width: 0;
  }
  .opt.on {
    background: var(--accent-soft);
    color: var(--accent-text);
  }
  .ic {
    display: grid;
    flex: none;
    color: var(--text-3);
  }
  .opt.on .ic {
    color: inherit;
  }
  .label {
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    min-width: 0;
  }
  .detail {
    margin-left: auto;
    flex: none;
    font-size: var(--fs-sm);
    max-width: 45%;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .none {
    margin: 14px 2px 4px;
    font-size: var(--fs-sm);
  }
  .keys {
    margin: 10px 2px 0;
    font-size: var(--fs-xs);
  }
</style>
