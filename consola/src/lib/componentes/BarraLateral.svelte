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
    PanelLeftClose,
    PanelLeftOpen,
  } from "@lucide/svelte";
  import { barra, plegarBarra } from "$lib/barra.svelte";
  import { atajos, MOD } from "$lib/atajos.svelte";
  import { page } from "$app/state";
  import { goto } from "$app/navigation";
  import Logo from "$ui/componentes/Logo.svelte";
  import * as api from "$lib/api";
  import { actual, app, puede } from "$lib/estado.svelte";
  import CopiasEnMarcha from "./CopiasEnMarcha.svelte";
  import MarcaCliente from "./MarcaCliente.svelte";

  // `plegable`: la de escritorio, que se puede plegar a iconos; en el cajón del móvil, siempre abierta.
  let { alNavegar, plegable = false }: { alNavegar?: () => void; plegable?: boolean } = $props();
  let selector = $state(false);
  const plegada = $derived(plegable && barra.plegada);

  const c = $derived(actual.id);
  const rol = $derived(actual.cliente?.rol);
  type Seccion = { href: string; texto: string; icono: typeof Monitor; exacto?: boolean; cuenta?: number; tono?: "warn" | "bad" };
  // Las secciones del cliente, en tres grupos con su etiqueta (el orden de siempre).
  const grupos = $derived.by(() => {
    const gs: { titulo: string; items: Seccion[] }[] = [
      {
        titulo: "Protección",
        items: [
          { href: `/c/${c}`, texto: "Estado", icono: LayoutDashboard, exacto: true },
          { href: `/c/${c}/equipos`, texto: "Equipos", icono: Monitor, cuenta: actual.equipos.length || undefined },
          { href: `/c/${c}/repositorios`, texto: "Repositorios y destinos", icono: Database },
        ],
      },
      {
        titulo: "Operación",
        items: [
          { href: `/c/${c}/restaurar`, texto: "Restaurar", icono: History },
          { href: `/c/${c}/ordenes`, texto: "Órdenes", icono: ClipboardList, cuenta: actual.pendientes || undefined, tono: actual.pendientes ? "warn" : undefined },
          { href: `/c/${c}/avisos`, texto: "Avisos", icono: BellRing, cuenta: actual.avisosAbiertos || undefined, tono: actual.avisosAbiertos ? "bad" : undefined },
        ],
      },
      {
        titulo: "Gestión",
        items: [
          { href: `/c/${c}/informes`, texto: "Informes", icono: FileBarChart },
          // Los técnicos también leen la actividad (exportarla, solo administradores).
          ...(puede.ordenar(rol) ? [{ href: `/c/${c}/auditoria`, texto: "Actividad", icono: Activity }] : []),
          ...(puede.administrar(rol) ? [{ href: `/c/${c}/servidor`, texto: "Servidor", icono: ArrowRightLeft }] : []),
          ...(puede.propietario(rol) ? [{ href: `/c/${c}/ajustes`, texto: "Personas y ajustes", icono: Users }] : []),
        ],
      },
    ];
    return gs.filter((g) => g.items.length);
  });
  /** Plegada, el nombre (con su contador) va en el `aria-label` y en el tooltip. */
  const nombreDe = (s: Seccion) => `${s.texto}${s.cuenta ? `, ${s.cuenta}` : ""}`;

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

<nav class="barra" class:plegada aria-label="Navegación principal">
  <div class="cabeza">
    <a class="marca" href="/" onclick={alNavegar} aria-label={plegada ? "Resguardo, inicio" : undefined} use:tip={plegada ? "Resguardo, inicio" : null}
      ><Logo size={26} />{#if !plegada}<span>Resguardo</span>{/if}</a
    >
    {#if plegable}
      <button class="icon-btn plegar" aria-label={plegada ? "Desplegar la barra lateral" : "Plegar la barra lateral"} aria-expanded={!plegada} onclick={() => plegarBarra(!plegada)}>
        {#if plegada}<PanelLeftOpen size={16} />{:else}<PanelLeftClose size={16} />{/if}
      </button>
    {/if}
  </div>

  <div class="selector">
    <button
      class="cliente-btn"
      bind:this={botonCliente}
      aria-expanded={selector}
      aria-controls={selector ? "menu-clientes" : undefined}
      use:tip={plegada ? (actual.cliente?.nombre ?? "Elige un cliente") : null}
      onclick={() => (selector = !selector)}
    >
      <MarcaCliente nombre={actual.cliente?.nombre ?? "·"} marca={actual.cliente?.marca ?? app.clientes.find((x) => x.id === c)?.marca} />
      <span class="nombre" class:sr-only={plegada}>{actual.cliente?.nombre ?? "Elige un cliente"}</span>
      {#if !plegada}<ChevronsUpDown size={14} />{/if}
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
    aria-label={plegada ? "Buscar o ir a…" : undefined}
    use:tip={plegada ? "Buscar o ir a…" : null}
  >
    <Search size={15} />{#if !plegada}<span>Buscar o ir a…</span><kbd>{MOD} K</kbd>{/if}
  </button>

  {#if c}
    <div class="grupos">
      {#each grupos as g, gi (g.titulo)}
        <div class="grupo">
          <!-- La etiqueta da nombre a la lista; plegada, solo queda una raya fina entre grupos. -->
          <p class="grupo-titulo" class:sr-only={plegada} id="grupo-{gi}">{g.titulo}</p>
          {#if plegada && gi > 0}<span class="raya" aria-hidden="true"></span>{/if}
          <ul class="items" aria-labelledby="grupo-{gi}">
            {#each g.items as s (s.href)}
              <li>
                <a
                  href={s.href}
                  class:on={activo(s.href, s.exacto)}
                  aria-current={activo(s.href, s.exacto) ? "page" : undefined}
                  aria-label={plegada ? nombreDe(s) : undefined}
                  use:tip={plegada ? nombreDe(s) : null}
                  onclick={alNavegar}
                >
                  <s.icono size={16} />
                  {#if !plegada}<span class="texto">{s.texto}</span>{/if}
                  {#if s.cuenta}<span class="cuenta num" class:warn={s.tono === "warn"} class:bad={s.tono === "bad"}>{s.cuenta}</span>{/if}
                </a>
              </li>
            {/each}
          </ul>
        </div>
      {/each}
    </div>
    <CopiasEnMarcha {alNavegar} compacto={plegada} />
  {/if}

  <ul class="items abajo">
    <li>
      <a href="/ayuda" class:on={activo("/ayuda")} aria-current={activo("/ayuda") ? "page" : undefined} aria-label={plegada ? "Ayuda" : undefined} use:tip={plegada ? "Ayuda" : null} onclick={alNavegar}
        ><CircleHelp size={16} />{#if !plegada}<span class="texto">Ayuda</span>{/if}</a
      >
    </li>
    <li>
      <a
        href="/ajustes"
        class:on={activo("/ajustes")}
        aria-current={activo("/ajustes") ? "page" : undefined}
        aria-label={plegada ? "Mi cuenta y servidor" : undefined}
        use:tip={plegada ? "Mi cuenta y servidor" : null}
        onclick={alNavegar}><Settings size={16} />{#if !plegada}<span class="texto">Mi cuenta y servidor</span>{/if}</a
      >
    </li>
    <li class="yo">
      <span class="avatar" aria-hidden="true"><UserRound size={14} /></span>
      <span class="quien" class:sr-only={plegada}><span>{app.cuenta?.nombre}</span><span class="faint">{app.cuenta?.correo}</span></span>
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
    overflow-x: hidden;
    overflow-y: auto;
    background: var(--bg-subtle);
    border-right: 1px solid var(--border);
  }
  /* Si no cabe, se desplaza la barra: nada se aplasta. */
  .barra > :global(*) {
    flex-shrink: 0;
  }
  .cabeza {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--sp-2);
    min-height: 30px;
  }
  .marca {
    display: flex;
    align-items: center;
    gap: 9px;
    min-width: 0;
    padding: 2px 6px;
    font-size: 15px;
    font-weight: 650;
    letter-spacing: -0.01em;
    color: var(--text-1);
    text-decoration: none;
    border-radius: var(--radius-sm);
  }
  .plegar {
    flex: none;
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
    background: var(--surface);
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
    background: var(--surface-2);
    border-color: var(--border);
  }
  .cliente-btn {
    display: flex;
    align-items: center;
    gap: 9px;
    width: 100%;
    height: 42px;
    padding: 0 10px 0 7px;
    font: inherit;
    font-weight: 550;
    color: var(--text-1);
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    box-shadow: var(--shadow-sm);
    cursor: pointer;
    transition: border-color var(--dur-fast) var(--ease);
  }
  .cliente-btn:hover,
  .cliente-btn[aria-expanded="true"] {
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
    min-width: 240px;
    padding: 4px;
    box-shadow: var(--shadow-md);
    animation: menu-entra var(--dur) var(--ease-out);
  }
  @keyframes menu-entra {
    from {
      opacity: 0;
      transform: translateY(-4px);
    }
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
  .menu a:focus-visible {
    outline-offset: -2px;
  }
  .menu a :global(svg:last-child) {
    color: var(--accent-text);
  }
  .menu hr {
    margin: 4px 0;
    border: none;
    border-top: 1px solid var(--border);
  }
  /* Grupos de secciones con su etiqueta pequeña en mayúsculas (docs/diseno.md §5). */
  .grupos {
    display: flex;
    flex-direction: column;
    gap: var(--sp-4);
  }
  .grupo-titulo {
    margin: 0 0 4px;
    padding: 0 8px;
    font-size: var(--fs-overline);
    line-height: 16px;
    font-weight: 600;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--text-3);
  }
  .raya {
    display: block;
    width: 20px;
    height: 1px;
    margin: 0 auto var(--sp-2);
    background: var(--border-strong);
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
    position: relative;
    display: flex;
    align-items: center;
    gap: 10px;
    height: 32px;
    padding: 0 8px 0 10px;
    border-radius: var(--radius-sm);
    color: var(--text-2);
    text-decoration: none;
    transition:
      background var(--dur-fast) var(--ease),
      color var(--dur-fast) var(--ease);
  }
  .items a :global(svg) {
    flex: none;
    color: var(--text-3);
    transition: color var(--dur-fast) var(--ease);
  }
  .items a:hover {
    color: var(--text-1);
    background: var(--surface-2);
  }
  .items a:hover :global(svg) {
    color: var(--text-2);
  }
  .items a:focus-visible {
    outline-offset: -2px;
  }
  /* Activo: superficie suave con un borde fino y una raya corta del acento a la izquierda. */
  .items a.on {
    font-weight: 550;
    color: var(--text-1);
    background: var(--surface);
    box-shadow:
      0 0 0 1px var(--border),
      var(--shadow-sm);
  }
  .items a.on::before {
    content: "";
    position: absolute;
    left: 0;
    top: 8px;
    bottom: 8px;
    width: 3px;
    border-radius: 0 3px 3px 0;
    background: var(--accent);
  }
  .items a.on :global(svg) {
    color: var(--accent-text);
  }
  .items .texto {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .cuenta {
    min-width: 20px;
    padding: 0 6px;
    font-size: 11px;
    line-height: 18px;
    font-weight: 600;
    text-align: center;
    color: var(--text-2);
    background: var(--surface-3);
    border-radius: 999px;
  }
  .cuenta.warn {
    color: var(--warn);
    background: var(--warn-soft);
  }
  .cuenta.bad {
    color: var(--bad);
    background: var(--bad-soft);
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
    padding: 6px 2px 0 6px;
    color: var(--text-3);
  }
  .avatar {
    display: grid;
    place-items: center;
    flex: none;
    width: 24px;
    height: 24px;
    color: var(--text-2);
    background: var(--surface-3);
    border-radius: 999px;
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

  /* ---------- Plegada a iconos (escritorio estrecho o a gusto) ---------- */
  .plegada {
    align-items: center;
    padding-left: 10px;
    padding-right: 10px;
  }
  .plegada .cabeza {
    flex-direction: column;
    gap: var(--sp-2);
  }
  .plegada .marca {
    padding: 2px;
  }
  .plegada .selector,
  .plegada .grupos,
  .plegada .items {
    width: 100%;
  }
  .plegada .cliente-btn {
    justify-content: center;
    width: 40px;
    margin: 0 auto;
    padding: 0;
  }
  .plegada .buscar {
    justify-content: center;
    width: 36px;
    margin-top: 0;
    padding: 0;
  }
  .plegada .grupos {
    gap: var(--sp-2);
  }
  .plegada .items a {
    justify-content: center;
    width: 36px;
    height: 36px;
    margin: 0 auto;
    padding: 0;
  }
  .plegada .items a.on::before {
    left: -10px;
  }
  /* Plegada, el contador es un punto con la cifra encima del icono. */
  .plegada .cuenta {
    position: absolute;
    top: 1px;
    right: -2px;
    min-width: 16px;
    padding: 0 4px;
    font-size: 10px;
    line-height: 15px;
    box-shadow: 0 0 0 2px var(--bg-subtle);
  }
  .plegada .cuenta.warn {
    color: var(--accent-contrast);
    background: var(--warn);
  }
  .plegada .cuenta.bad {
    color: var(--bad-contrast);
    background: var(--bad);
  }
  .plegada .yo {
    flex-direction: column;
    gap: var(--sp-2);
    padding: 6px 0 0;
  }
  /* Lo que está en marcha, en una pastilla de icono y porcentaje. */
  .plegada :global(.chip) {
    flex-direction: column;
    gap: 2px;
    width: 40px;
    padding: 6px 0;
    font-size: 10.5px;
    line-height: 12px;
    white-space: nowrap;
    border-radius: var(--radius);
  }
  .plegada .buscar {
    height: 32px;
  }
  .plegada .menu {
    right: auto;
    width: 260px;
  }
  @media (prefers-reduced-motion: reduce) {
    .menu {
      animation: none;
    }
  }
</style>
