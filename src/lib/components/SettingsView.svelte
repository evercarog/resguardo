<script lang="ts">
  // «Ajustes»: todos los ajustes en un solo sitio, por secciones, con un
  // índice a la izquierda. Cada ajuste es una fila: etiqueta, una línea de
  // explicación y el control, con la misma insignia cuando pide administrador,
  // la contraseña de un destino o Windows Hello.
  import { onMount, tick } from "svelte";
  import { fade } from "svelte/transition";
  import { dur } from "$lib/motion";
  import { Check, KeyRound, LogIn, LogOut, ScrollText, ShieldCheck, Sparkles, Wrench } from "@lucide/svelte";
  import * as api from "$lib/api";
  import type { AccountStatus, Repo } from "$lib/api";
  import { agent } from "$lib/agent.svelte";
  import { kitState, openKit } from "$lib/kit.svelte";
  import { openNews } from "$lib/news.svelte";
  import { ACCENTS, THEMES, appearance, setAccent, setTheme } from "$lib/settings.svelte";
  import { SETTINGS_SECTIONS, relaunchToSettings, type SettingsSection } from "$lib/settingsNav.svelte";
  import { webLinked } from "$lib/web.svelte";
  import { toast } from "$lib/toast.svelte";
  import SettingRow from "./SettingRow.svelte";
  import AppLockSettings from "./AppLockSettings.svelte";
  import TraySettings from "./TraySettings.svelte";
  import ShellMenuSetting from "./ShellMenuSetting.svelte";
  import DiscreetSetting from "./DiscreetSetting.svelte";
  import RemoteBackupSetting from "./RemoteBackupSetting.svelte";
  import WebLinkSetting from "./WebLinkSetting.svelte";
  import ServerSetting from "./ServerSetting.svelte";
  import AgentLogDialog from "./AgentLogDialog.svelte";
  import HelpLink from "./HelpLink.svelte";
  import RelTime from "./RelTime.svelte";

  interface Props {
    repos: Repo[];
    /** Sección a la que ir al abrir. */
    section?: SettingsSection;
    appVersion: string;
    /** Salida de `restic version`; vacía si no se pudo ejecutar. */
    resticVersion: string;
    /** Abrir «Todos mis equipos» (para iniciar sesión). */
    onequipos: () => void;
  }
  let { repos, section = "general", appVersion, resticVersion, onequipos }: Props = $props();

  // ---------- Índice ----------

  let active = $state<SettingsSection>("general");
  const els: Partial<Record<SettingsSection, HTMLElement>> = {};

  function go(id: SettingsSection, smooth = true) {
    active = id;
    els[id]?.scrollIntoView({ behavior: smooth ? "smooth" : "auto", block: "start" });
  }

  $effect(() => {
    const want = section;
    void tick().then(() => go(want, false));
  });

  onMount(() => {
    // La sección visible arriba es la activa en el índice.
    const root = els.general?.closest("main") ?? null;
    const obs = new IntersectionObserver(
      (entries) => {
        const top = entries.filter((e) => e.isIntersecting).sort((a, b) => a.boundingClientRect.top - b.boundingClientRect.top)[0];
        if (top) active = top.target.id.replace("ajustes-", "") as SettingsSection;
      },
      { root, rootMargin: "0px 0px -70% 0px" },
    );
    for (const el of Object.values(els)) if (el) obs.observe(el);
    return () => obs.disconnect();
  });

  // ---------- General ----------

  const resticShort = $derived(resticVersion.split(" ").slice(0, 2).join(" "));
  /** Colores de las miniaturas de cada modo: [fondo, barra lateral, tarjeta, texto]. */
  const PREVIEW = {
    light: ["#f4f5f7", "#ffffff", "#ffffff", "#c9ced6"],
    dark: ["#0d1014", "#151a1f", "#1a2027", "#3a4450"],
    black: ["#000000", "#0b0b0c", "#111113", "#2e2e33"],
  } as const;
  const current = $derived(ACCENTS.find((a) => a.id === appearance.accent)!);

  // ---------- Este equipo ----------

  const info = $derived(agent.info);
  let showLog = $state(false);
  let agentError = $state("");
  const agentStatus = $derived.by(() => {
    if (!info) return "Consultando…";
    if (!info.supported) return "El agente solo existe en Windows.";
    if (!info.repos.length) return "Aún no hay copias automáticas: el agente no tiene nada que hacer.";
    const n = info.repos.length;
    return info.task_installed
      ? `Activo · ${n} ${n === 1 ? "repositorio" : "repositorios"} con copias automáticas`
      : "La tarea del agente no está instalada: las copias automáticas no se harán.";
  });

  async function repair() {
    agentError = "";
    try {
      if (info?.elevated) {
        agent.info = await api.agentRepair();
        toast("Agente reparado", "success");
      } else await relaunchToSettings("equipo");
    } catch (e) {
      agentError = String(e);
    }
  }

  // ---------- Seguridad ----------

  const kitOk = $derived(repos.filter((r) => kitState(r) === "ok").length);
  let account = $state<AccountStatus | null>(null);
  let accountError = $state("");
  onMount(() => {
    api.accountStatus().then(
      (s) => (account = s),
      (e) => (accountError = String(e)),
    );
  });
  async function signOut() {
    try {
      account = await api.accountSignOut();
      toast("Sesión cerrada en este equipo", "info");
    } catch (e) {
      accountError = String(e);
    }
  }
</script>

{#snippet mini(mode: "light" | "dark" | "black", accent: string)}
  {@const [bg, side, card, line] = PREVIEW[mode]}
  <svg viewBox="0 0 120 76" aria-hidden="true">
    <rect width="120" height="76" fill={bg} />
    <rect width="34" height="76" fill={side} />
    <rect x="6" y="8" width="22" height="7" rx="2" fill={accent} />
    <rect x="6" y="20" width="18" height="4" rx="2" fill={line} />
    <rect x="6" y="28" width="20" height="4" rx="2" fill={line} />
    <rect x="42" y="8" width="70" height="26" rx="4" fill={card} stroke={line} stroke-opacity="0.5" />
    <rect x="48" y="14" width="30" height="4" rx="2" fill={line} />
    <rect x="92" y="14" width="14" height="7" rx="2" fill={accent} />
    <rect x="48" y="24" width="56" height="4" rx="2" fill={accent} fill-opacity="0.5" />
    <rect x="42" y="40" width="70" height="28" rx="4" fill={card} stroke={line} stroke-opacity="0.5" />
    <rect x="48" y="47" width="44" height="4" rx="2" fill={line} />
    <rect x="48" y="56" width="36" height="4" rx="2" fill={line} />
  </svg>
{/snippet}

<div class="page settings" in:fade={{ duration: dur(160) }}>
  <header class="page-top">
    <div>
      <h1>Ajustes</h1>
      <p>Todo lo que se puede ajustar de Resguardo en este equipo.</p>
    </div>
  </header>

  <div class="layout">
    <nav class="toc" aria-label="Secciones de Ajustes">
      {#each SETTINGS_SECTIONS as s (s.id)}
        <button class:on={active === s.id} aria-current={active === s.id ? "true" : undefined} onclick={() => go(s.id)}>{s.label}</button>
      {/each}
    </nav>

    <div class="sections">
      <!-- General -->
      <section id="ajustes-general" class="card block" bind:this={els.general} aria-labelledby="h-general">
        <h2 id="h-general" class="section-title">General</h2>
        <SettingRow label="Modo" description="«Sistema» sigue el modo claro u oscuro de Windows automáticamente.">
          {#snippet below()}
            <div class="themes" role="radiogroup" aria-label="Modo">
              {#each THEMES as t (t.id)}
                <button class="theme" class:on={appearance.theme === t.id} role="radio" aria-checked={appearance.theme === t.id} onclick={() => setTheme(t.id)}>
                  <span class="thumb">
                    {#if t.id === "system"}
                      <span class="split">
                        <span class="half">{@render mini("light", current.light)}</span>
                        <span class="half right">{@render mini("dark", current.dark)}</span>
                      </span>
                    {:else}
                      {@render mini(t.id, t.id === "light" ? current.light : current.dark)}
                    {/if}
                  </span>
                  <span class="theme-label">{#if appearance.theme === t.id}<Check size={13} />{/if} {t.label}</span>
                </button>
              {/each}
            </div>
          {/snippet}
        </SettingRow>
        <SettingRow label="Color de acento" description={current.label}>
          <div class="accents" role="radiogroup" aria-label="Color de acento">
            {#each ACCENTS as a (a.id)}
              <button
                class="swatch"
                class:on={appearance.accent === a.id}
                role="radio"
                aria-checked={appearance.accent === a.id}
                aria-label={a.label}
                title={a.label}
                style="--l: {a.light}; --d: {a.dark}"
                onclick={() => setAccent(a.id)}
              >
                <span class="dot">{#if appearance.accent === a.id}<Check size={13} strokeWidth={3} />{/if}</span>
              </button>
            {/each}
          </div>
        </SettingRow>
        <SettingRow label="Atajos de teclado" description="F1 ayuda · Ctrl+K ir a… · Ctrl+N nueva copia · Ctrl+Mayús+N añadir repositorio · Ctrl+1 estado · Ctrl+2 actividad · Ctrl+, ajustes · Ctrl+↑/↓ anterior o siguiente." />
        <SettingRow label="Acerca de Resguardo">
          <button class="btn btn-sm" onclick={openNews}><Sparkles size={14} /> Novedades</button>
          {#snippet status()}
            <span class="selectable">
              <strong>Resguardo{appVersion ? ` v${appVersion}` : ""}</strong> · Software libre con licencia AGPL-3.0 o posterior. Incluye {resticShort || "restic"} (licencia
              BSD-2-Clause), que es quien cifra y guarda las copias.
              {#if !resticVersion}<span class="bad-text">No se pudo ejecutar el restic incluido: reinstala Resguardo.</span>{/if}
            </span>
          {/snippet}
        </SettingRow>
      </section>

      <!-- Este equipo -->
      <section id="ajustes-equipo" class="card block" bind:this={els.equipo} aria-labelledby="h-equipo">
        <h2 id="h-equipo" class="section-title">Este equipo</h2>
        <p class="intro">Lo que hace el agente de copias de este equipo, aunque Resguardo esté cerrado. Lo comparten todos sus usuarios.</p>
        {#if info && !info.elevated}
          <div class="notice notice-info elevate">
            <ShieldCheck size={16} />
            <p>Para cambiar estos ajustes hay que abrir Resguardo como administrador. Windows lo pedirá y volverás aquí.</p>
            <button class="btn btn-sm" onclick={() => relaunchToSettings("equipo").catch((e) => (agentError = String(e)))}>Abrir como administrador</button>
          </div>
        {/if}
        <DiscreetSetting />
        <RemoteBackupSetting linked={webLinked()} />
        <WebLinkSetting />
        <ServerSetting />
        <SettingRow label="Agente de copias" description="Hace las copias automáticas, las subidas a la nube y las verificaciones. Su registro dice qué hizo cada vez.">
          <button class="btn btn-ghost btn-sm" onclick={() => (showLog = true)}><ScrollText size={14} /> Ver registro</button>
          {#if info?.supported && info.repos.length && !info.task_installed}
            <button class="btn btn-sm" onclick={repair}><Wrench size={14} /> Reparar</button>
          {/if}
          {#snippet status()}
            <span class:warn-text={!!info && info.repos.length > 0 && !info.task_installed}>{agentStatus}</span>
            {#if info?.state.last_tick}<span class="faint"> · última vuelta <RelTime iso={info.state.last_tick} /></span>{/if}
          {/snippet}
          {#snippet below()}
            {#if agentError}<p class="bad-text small" role="alert">{agentError}</p>{/if}
          {/snippet}
        </SettingRow>
        <SettingRow
          label="Datos del agente"
          description="En C:\ProgramData\Resguardo: la programación, el historial y las contraseñas, cifradas y solo legibles por el sistema. Al desinstalar puedes elegir borrarlos; las copias de tus repositorios no se tocan."
        />
      </section>

      <!-- Bandeja y avisos -->
      <section id="ajustes-bandeja" class="card block" bind:this={els.bandeja} aria-labelledby="h-bandeja">
        <h2 id="h-bandeja" class="section-title">Bandeja y avisos <HelpLink topic="bandeja" label="la bandeja y los avisos" /></h2>
        <p class="intro">Solo para tu usuario.</p>
        <TraySettings />
      </section>

      <!-- Explorador -->
      <section id="ajustes-explorador" class="card block" bind:this={els.explorador} aria-labelledby="h-explorador">
        <h2 id="h-explorador" class="section-title">Explorador de archivos <HelpLink topic="ver-versiones" label="ver versiones desde el Explorador" /></h2>
        <p class="intro">Solo para tu usuario.</p>
        <ShellMenuSetting />
      </section>

      <!-- Seguridad -->
      <section id="ajustes-seguridad" class="card block" bind:this={els.seguridad} aria-labelledby="h-seguridad">
        <h2 id="h-seguridad" class="section-title">Seguridad</h2>
        <AppLockSettings />
        <SettingRow
          label="Kit de recuperación"
          description="La contraseña y la ubicación de cada repositorio, para imprimir o guardar fuera del equipo. Sin él, si pierdes este equipo no podrás abrir las copias."
          needs={["password"]}
        >
          <button class="btn btn-sm" onclick={() => openKit(null)} disabled={!repos.length}><KeyRound size={14} /> Preparar el kit</button>
          {#snippet status()}
            {#if !repos.length}
              Aún no hay repositorios.
            {:else if kitOk === repos.length}
              <span class="ok-text">Guardado para {repos.length === 1 ? "el repositorio" : `los ${repos.length} repositorios`}.</span>
            {:else}
              <span class="warn-text">Guardado para {kitOk} de {repos.length} repositorios.</span>
            {/if}
          {/snippet}
        </SettingRow>
        <SettingRow label="Cuenta de Resguardo Web en este equipo" description="La sesión de «Todos mis equipos». Se guarda cifrada para tu usuario de Windows; la contraseña nunca se guarda.">
          {#if account?.signed_in}
            <button class="btn btn-sm" onclick={signOut}><LogOut size={14} /> Cerrar sesión</button>
          {:else if account}
            <button class="btn btn-sm" onclick={onequipos}><LogIn size={14} /> Iniciar sesión</button>
          {/if}
          {#snippet status()}
            {#if accountError}
              <span class="bad-text">{accountError}</span>
            {:else if !account}
              Consultando…
            {:else if account.signed_in}
              Sesión iniciada como <strong>{account.email}</strong>.
            {:else}
              Sin sesión iniciada.
            {/if}
          {/snippet}
        </SettingRow>
      </section>
    </div>
  </div>
</div>

{#if showLog}<AgentLogDialog onclose={() => (showLog = false)} />{/if}

<style>
  .layout {
    display: grid;
    grid-template-columns: 180px minmax(0, 1fr);
    gap: var(--sp-8);
    align-items: start;
  }
  .toc {
    position: sticky;
    top: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .toc button {
    height: 32px;
    padding: 0 10px;
    font: inherit;
    font-size: var(--fs-body);
    text-align: left;
    color: var(--text-2);
    background: none;
    border: none;
    border-radius: var(--radius-sm);
    cursor: pointer;
  }
  .toc button:hover {
    background: var(--surface-2);
  }
  .toc button.on {
    color: var(--text-1);
    font-weight: 500;
    background: var(--surface-2);
  }
  .sections {
    display: flex;
    flex-direction: column;
    gap: var(--sp-6);
    min-width: 0;
  }
  .block {
    padding: 18px 20px 6px;
    scroll-margin-top: 8px;
  }
  .block h2 {
    display: flex;
    align-items: center;
    gap: 4px;
    margin-bottom: 2px;
  }
  .intro {
    margin: 0 0 4px;
    font-size: var(--fs-sm);
    color: var(--text-3);
  }
  .elevate {
    margin: 10px 0 4px;
    align-items: center;
  }
  .elevate .btn {
    margin-left: auto;
    flex: none;
  }
  .small {
    margin: 0;
    font-size: var(--fs-sm);
  }
  .bad-text {
    color: var(--bad);
  }
  .warn-text {
    color: var(--warn);
  }
  .ok-text {
    color: var(--ok);
  }
  .themes {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 132px));
    gap: 10px;
  }
  .theme {
    display: flex;
    flex-direction: column;
    gap: 7px;
    padding: 0;
    font: inherit;
    color: var(--text-2);
    background: none;
    border: none;
    cursor: pointer;
  }
  .thumb {
    display: block;
    overflow: hidden;
    border-radius: var(--radius);
    border: 2px solid var(--border);
    transition:
      border-color 0.15s,
      box-shadow 0.15s;
  }
  .thumb :global(svg) {
    display: block;
    width: 100%;
    height: auto;
  }
  .theme:hover .thumb {
    border-color: var(--border-strong);
  }
  .theme.on .thumb {
    border-color: var(--accent);
    box-shadow: 0 0 0 3px var(--accent-soft);
  }
  .split {
    position: relative;
    display: block;
  }
  .half.right {
    position: absolute;
    inset: 0;
    clip-path: polygon(55% 0, 100% 0, 100% 100%, 35% 100%);
  }
  .theme-label {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 4px;
    font-size: var(--fs-sm);
    font-weight: 550;
  }
  .theme.on .theme-label {
    color: var(--text-1);
  }
  .accents {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
  }
  .swatch {
    display: grid;
    place-items: center;
    width: 34px;
    height: 34px;
    padding: 0;
    background: none;
    border: 2px solid transparent;
    border-radius: 50%;
    cursor: pointer;
    transition: border-color 0.15s;
  }
  .swatch.on {
    border-color: var(--text-3);
  }
  .dot {
    display: grid;
    place-items: center;
    width: 26px;
    height: 26px;
    border-radius: 50%;
    color: #fff;
    background: linear-gradient(135deg, var(--l) 50%, var(--d) 50%);
    box-shadow: inset 0 0 0 1px rgb(0 0 0 / 0.1);
  }
  /* Ventanas estrechas: el índice pasa arriba, en una fila. */
  @media (max-width: 1000px) {
    .layout {
      grid-template-columns: minmax(0, 1fr);
      gap: var(--sp-4);
    }
    .toc {
      position: static;
      flex-direction: row;
      flex-wrap: wrap;
    }
  }
</style>
