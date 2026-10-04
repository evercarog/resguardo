<script lang="ts">
  import { tip } from "$lib/tooltip";
  // Barra lateral (docs/diseno.md §5): selector de cliente arriba, las
  // secciones del cliente y, abajo, ayuda y cuenta. En móvil es un cajón.
  import {
    Activity,
    ArrowRightLeft,
    BellRing,
    ChevronsUpDown,
    CircleHelp,
    ClipboardList,
    Database,
    FileBarChart,
    Gauge,
    History,
    LayoutDashboard,
    LogOut,
    Monitor,
    Settings,
    UserRound,
    Users,
    Check,
    Plus,
    Search,
  } from "@lucide/svelte";
  import { atajos, MOD } from "$lib/atajos.svelte";
  import { page } from "$app/state";
  import { goto } from "$app/navigation";
  import Logo from "$ui/componentes/Logo.svelte";
  import * as api from "$lib/api";
  import { actual, app, puede } from "$lib/estado.svelte";
  import CopiasEnMarcha from "./CopiasEnMarcha.svelte";
  import MarcaCliente from "./MarcaCliente.svelte";

  let { alNavegar }: { alNavegar?: () => void } = $props();
  let selector = $state(false);

  const c = $derived(actual.id);
  const rol = $derived(actual.cliente?.rol);
  const secciones = $derived([
    { href: `/c/${c}`, texto: "Estado", icono: LayoutDashboard, exacto: true },
    { href: `/c/${c}/equipos`, texto: "Equipos", icono: Monitor, cuenta: actual.equipos.length || undefined },
    { href: `/c/${c}/repositorios`, texto: "Repositorios y destinos", icono: Database },
    { href: `/c/${c}/restaurar`, texto: "Restaurar", icono: History },
    { href: `/c/${c}/ordenes`, texto: "Órdenes", icono: ClipboardList, cuenta: actual.pendientes || undefined, tono: actual.pendientes ? "warn" : undefined },
    { href: `/c/${c}/avisos`, texto: "Avisos", icono: BellRing, cuenta: actual.avisosAbiertos || undefined, tono: actual.avisosAbiertos ? "bad" : undefined },
    { href: `/c/${c}/informes`, texto: "Informes", icono: FileBarChart },
    // Los técnicos también leen la actividad (exportarla, solo administradores).
    ...(puede.ordenar(rol) ? [{ href: `/c/${c}/auditoria`, texto: "Actividad", icono: Activity }] : []),
    ...(puede.administrar(rol) ? [{ href: `/c/${c}/servidor`, texto: "Servidor", icono: ArrowRightLeft }] : []),
    ...(puede.propietario(rol) ? [{ href: `/c/${c}/ajustes`, texto: "Personas y ajustes", icono: Users }] : []),
  ]);

  const activo = (href: string, exacto = false) => (exacto ? page.url.pathname === href : page.url.pathname === href || page.url.pathname.startsWith(href + "/"));

  async function salir() {
    try {
      await api.salir();
    } finally {
      app.cuenta = null;
      await goto("/entrar", { replaceState: true });
    }
  }

  let botonCliente = $state<HTMLButtonElement>();
  function teclasMenu(e: KeyboardEvent) {
    const xs = Array.from((e.currentTarget as HTMLElement).querySelectorAll<HTMLElement>("a[href]"));
    const i = xs.indexOf(document.activeElement as HTMLElement);
    if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      selector = false;
      botonCliente?.focus();
    } else if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      xs[(i + (e.key === "ArrowDown" ? 1 : -1) + xs.length) % xs.length]?.focus();
    }
  }
  /** Si el foco sale del desplegable (Tab), se cierra. */
  function fueraDelMenu(e: FocusEvent) {
    const a = e.relatedTarget as Node | null;
    if (a && !(e.currentTarget as HTMLElement).contains(a) && a !== botonCliente) selector = false;
  }

  function onwindow(e: PointerEvent) {
    if (selector && !(e.target as Element).closest?.(".selector")) selector = false;
  }
</script>

<svelte:window onpointerdown={onwindow} />

<nav class="barra" aria-label="Navegación principal">
  <a class="marca" href="/" onclick={alNavegar}><Logo size={26} /><span>Resguardo</span></a>

  <div class="selector">
    <button class="cliente-btn" bind:this={botonCliente} aria-expanded={selector} aria-controls={selector ? "menu-clientes" : undefined} onclick={() => (selector = !selector)}>
      <MarcaCliente nombre={actual.cliente?.nombre ?? "·"} marca={actual.cliente?.marca ?? app.clientes.find((x) => x.id === c)?.marca} />
      <span class="nombre">{actual.cliente?.nombre ?? "Elige un cliente"}</span>
      <ChevronsUpDown size={14} />
    </button>
    {#if selector}
      <!-- Un desplegable de enlaces (no un listbox): Tab y las flechas lo recorren, Esc lo cierra. -->
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div class="menu card" id="menu-clientes" onkeydown={teclasMenu} onfocusout={fueraDelMenu}>
        {#each app.clientes as x (x.id)}
          <a
            aria-current={x.id === c ? "true" : undefined}
            href="/c/{x.id}"
            onclick={() => {
              selector = false;
              alNavegar?.();
            }}
          >
            <MarcaCliente nombre={x.nombre} marca={x.marca} tam={24} />
            <span class="nombre">{x.nombre}</span>
            {#if x.avisos}<span class="dot" style="--tone: var(--bad)" aria-hidden="true"></span><span class="sr-only">, {x.avisos} {x.avisos === 1 ? "aviso" : "avisos"}</span>{/if}
            {#if x.id === c}<Check size={14} />{/if}
          </a>
        {/each}
        <hr />
        <a
          href="/clientes"
          onclick={() => {
            selector = false;
            alNavegar?.();
          }}><span class="ini neutro"><Users size={13} /></span><span class="nombre">Todos los clientes</span></a
        >
        {#if app.cuenta?.superusuario}
          <a
            href="/clientes?nuevo=1"
            onclick={() => {
              selector = false;
              alNavegar?.();
            }}><span class="ini neutro"><Plus size={13} /></span><span class="nombre">Nuevo cliente</span></a
          >
          <a
            href="/servidor/clientes"
            onclick={() => {
              selector = false;
              alNavegar?.();
            }}><span class="ini neutro"><Gauge size={13} /></span><span class="nombre">Clientes del servidor</span></a
          >
        {/if}
      </div>
    {/if}
  </div>

  <button
    class="buscar"
    onclick={() => {
      alNavegar?.();
      atajos.paleta = true;
    }}
    aria-keyshortcuts="Control+K Meta+K"
  >
    <Search size={15} /><span>Buscar o ir a…</span><kbd>{MOD} K</kbd>
  </button>

  {#if c}
    <ul class="items">
      {#each secciones as s (s.href)}
        <li>
          <a href={s.href} class:on={activo(s.href, s.exacto)} aria-current={activo(s.href, s.exacto) ? "page" : undefined} onclick={alNavegar}>
            <s.icono size={16} />
            <span>{s.texto}</span>
            {#if s.cuenta}<span class="cuenta num" class:warn={s.tono === "warn"} class:bad={s.tono === "bad"}>{s.cuenta}</span>{/if}
          </a>
        </li>
      {/each}
    </ul>
    <CopiasEnMarcha {alNavegar} />
  {/if}

  <ul class="items abajo">
    <li><a href="/ayuda" class:on={activo("/ayuda")} onclick={alNavegar}><CircleHelp size={16} /><span>Ayuda</span></a></li>
    <li><a href="/ajustes" class:on={activo("/ajustes")} onclick={alNavegar}><Settings size={16} /><span>Mi cuenta y servidor</span></a></li>
    <li class="yo">
      <UserRound size={16} />
      <span class="quien"><span>{app.cuenta?.nombre}</span><span class="faint">{app.cuenta?.correo}</span></span>
      <button class="icon-btn" use:tip={"Salir"} aria-label="Salir" onclick={salir}><LogOut size={15} /></button>
    </li>
  </ul>
</nav>

<style>
  .barra {
    display: flex;
    flex-direction: column;
    gap: var(--sp-4);
    height: 100%;
    padding: var(--sp-4) var(--sp-3);
    overflow-y: auto;
    background: var(--bg-subtle);
    border-right: 1px solid var(--border);
  }
  .marca {
    display: flex;
    align-items: center;
    gap: 9px;
    padding: 2px 6px;
    font-size: 15px;
    font-weight: 650;
    letter-spacing: -0.01em;
    color: var(--text-1);
    text-decoration: none;
  }
  .selector {
    position: relative;
  }
  /* Abre la paleta (Ctrl+K): parece un campo, pero es un botón. */
  .buscar {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    height: 32px;
    margin-top: calc(-1 * var(--sp-2));
    padding: 0 8px 0 10px;
    font: inherit;
    font-size: var(--fs-sm);
    color: var(--text-3);
    text-align: left;
    background: none;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    cursor: pointer;
    transition:
      border-color var(--dur-fast) var(--ease),
      color var(--dur-fast) var(--ease);
  }
  .buscar:hover {
    color: var(--text-2);
    border-color: var(--border-strong);
  }
  .buscar span {
    flex: 1;
  }
  .buscar kbd {
    min-width: 0;
    padding: 0 5px;
    font-size: 10.5px;
    line-height: 17px;
    font-weight: 500;
    color: var(--text-3);
    background: var(--surface);
    border-color: var(--border);
  }
  .cliente-btn {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    height: 40px;
    padding: 0 10px 0 6px;
    font: inherit;
    font-weight: 500;
    color: var(--text-1);
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    cursor: pointer;
  }
  .cliente-btn:hover {
    border-color: var(--border-strong);
  }
  .cliente-btn :global(svg) {
    color: var(--text-3);
    flex: none;
  }
  .ini {
    display: grid;
    place-items: center;
    flex: none;
    width: 26px;
    height: 26px;
    font-size: 12px;
    font-weight: 650;
    color: var(--accent-text);
    background: var(--accent-soft);
    border-radius: 7px;
  }
  .ini.neutro {
    color: var(--text-2);
    background: var(--surface-3);
  }
  .nombre {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    text-align: left;
  }
  .menu {
    position: absolute;
    z-index: 20;
    top: calc(100% + 4px);
    left: 0;
    right: 0;
    padding: 4px;
    box-shadow: var(--shadow-md);
  }
  .menu a {
    display: flex;
    align-items: center;
    gap: 8px;
    height: 36px;
    padding: 0 8px 0 5px;
    border-radius: var(--radius-sm);
    color: var(--text-1);
    text-decoration: none;
  }
  .menu a:hover,
  .menu a[aria-current="true"] {
    background: var(--surface-2);
  }
  .menu hr {
    margin: 4px 0;
    border: none;
    border-top: 1px solid var(--border);
  }
  .items {
    display: flex;
    flex-direction: column;
    gap: 1px;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .items a {
    display: flex;
    align-items: center;
    gap: 10px;
    height: 32px;
    padding: 0 8px;
    border-radius: var(--radius-sm);
    color: var(--text-2);
    text-decoration: none;
  }
  .items a :global(svg) {
    flex: none;
    color: var(--text-3);
  }
  .items a:hover {
    color: var(--text-1);
    background: var(--surface-2);
  }
  .items a.on {
    font-weight: 500;
    color: var(--text-1);
    background: var(--surface);
    box-shadow: 0 0 0 1px var(--border);
  }
  .items a.on :global(svg) {
    color: var(--text-1);
  }
  .items a span:first-of-type {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .cuenta {
    font-size: var(--fs-xs);
    font-weight: 600;
    color: var(--text-3);
  }
  .cuenta.warn {
    color: var(--warn);
  }
  .cuenta.bad {
    color: var(--bad);
  }
  .abajo {
    margin-top: auto;
    padding-top: var(--sp-3);
    border-top: 1px solid var(--border);
  }
  .yo {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 6px 4px 0 8px;
    color: var(--text-3);
  }
  .quien {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-width: 0;
    font-size: var(--fs-sm);
    line-height: 17px;
    color: var(--text-1);
  }
  .quien span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .quien .faint {
    font-size: var(--fs-xs);
  }
</style>
