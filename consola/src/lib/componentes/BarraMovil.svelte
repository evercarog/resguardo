<script lang="ts">
  // 16r: lo que se hace desde el móvil, a dos toques. Abajo, solo en pantallas estrechas y
  // dentro de un cliente: Estado, Copias (¿terminó?), Equipos («Copiar ahora» en el menú de
  // cada uno), Avisos y Órdenes (las que esperan su turno, con «Cancelar»), con sus números.
  // Lo demás sigue en el menú de arriba.
  import { page } from "$app/state";
  import { BellRing, ClipboardList, FolderSync, LayoutDashboard, Monitor } from "@lucide/svelte";
  import { actual } from "$lib/estado.svelte";

  const base = $derived(`/c/${actual.id}`);
  const ruta = $derived(page.url.pathname);
  const items = $derived([
    { href: base, texto: "Estado", icono: LayoutDashboard, activo: ruta === base, n: 0, que: "" },
    { href: `${base}/copias`, texto: "Copias", icono: FolderSync, activo: ruta.startsWith(`${base}/copias`), n: 0, que: "" },
    { href: `${base}/equipos`, texto: "Equipos", icono: Monitor, activo: ruta.startsWith(`${base}/equipos`), n: 0, que: "" },
    { href: `${base}/avisos`, texto: "Avisos", icono: BellRing, activo: ruta.startsWith(`${base}/avisos`), n: actual.avisosAbiertos, que: "sin revisar" },
    { href: `${base}/actividad`, texto: "Órdenes", icono: ClipboardList, activo: ruta.startsWith(`${base}/actividad`), n: actual.pendientes, que: "esperando su turno" },
  ]);
</script>

<nav class="barra-movil" aria-label="Lo principal del cliente">
  {#each items as it (it.href)}
    {@const Ic = it.icono}
    <a href={it.href} class:activo={it.activo} aria-current={it.activo ? "page" : undefined}>
      <span class="ic"><Ic size={20} />{#if it.n}<span class="n num" aria-hidden="true">{it.n > 99 ? "99+" : it.n}</span>{/if}</span>
      <span class="t">{it.texto}</span>
      {#if it.n}<span class="sr-only">: {it.n} {it.que}</span>{/if}
    </a>
  {/each}
</nav>

<style>
  .barra-movil {
    display: none;
  }
  @media (max-width: 860px) {
    .barra-movil {
      position: fixed;
      right: 0;
      bottom: 0;
      left: 0;
      z-index: 6;
      display: grid;
      grid-template-columns: repeat(5, minmax(0, 1fr));
      padding: 4px 4px calc(4px + env(safe-area-inset-bottom));
      background: color-mix(in srgb, var(--bg) 92%, transparent);
      backdrop-filter: blur(8px);
      border-top: 1px solid var(--border);
    }
    a {
      display: flex;
      flex-direction: column;
      align-items: center;
      justify-content: center;
      gap: 2px;
      min-width: 0;
      min-height: 48px;
      font-size: 11px;
      font-weight: 550;
      color: var(--text-2);
      text-decoration: none;
      border-radius: var(--radius);
    }
    a.activo {
      color: var(--accent-text);
      background: var(--accent-soft);
    }
    .ic {
      position: relative;
      display: inline-flex;
    }
    .n {
      position: absolute;
      top: -6px;
      left: 12px;
      min-width: 16px;
      height: 16px;
      padding: 0 4px;
      font-size: 10px;
      line-height: 16px;
      text-align: center;
      color: #fff;
      background: var(--bad);
      border-radius: 8px;
    }
    .t {
      max-width: 100%;
      overflow: hidden;
      text-overflow: ellipsis;
      white-space: nowrap;
    }
  }
</style>
