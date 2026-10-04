<script lang="ts">
  // Índice de una página larga (repositorio, copia): una barra fina que se
  // queda arriba al bajar, con un enlace a cada sección y la que se está
  // viendo marcada. Solo en pantallas anchas; en móvil se baja sin más.
  let { items }: { items: { id: string; texto: string }[] } = $props();
  let activa = $state("");

  // La sección que se lee es la última cuyo principio ya pasó bajo la barra.
  // Se vuelve a mirar al bajar y cuando cambian las secciones (p. ej. llegan las gráficas con el informe).
  $effect(() => {
    const ids = items.map((x) => x.id);
    // Pocas secciones: medirlas en cada «scroll» es barato (el navegador ya los da uno por fotograma).
    const mirar = () => {
      let actual = ids[0] ?? "";
      for (const id of ids) {
        const el = document.getElementById(id);
        if (el && el.getBoundingClientRect().top <= 90) actual = id;
      }
      // Al llegar al final, la última (aunque sea corta y no llegue arriba).
      if (innerHeight + scrollY >= document.documentElement.scrollHeight - 4) actual = ids.at(-1) ?? actual;
      activa = actual;
    };
    mirar();
    addEventListener("scroll", mirar, { passive: true });
    addEventListener("resize", mirar, { passive: true });
    return () => {
      removeEventListener("scroll", mirar);
      removeEventListener("resize", mirar);
    };
  });

  function ir(e: MouseEvent, id: string) {
    const el = document.getElementById(id);
    if (!el) return;
    e.preventDefault();
    const reducir = matchMedia("(prefers-reduced-motion: reduce)").matches;
    el.scrollIntoView({ behavior: reducir ? "auto" : "smooth", block: "start" });
    history.replaceState(history.state, "", `#${id}`);
  }
</script>

<nav class="indice" aria-label="Secciones de esta página">
  {#each items as x (x.id)}
    <a href="#{x.id}" class:on={activa === x.id} aria-current={activa === x.id ? "location" : undefined} onclick={(e) => ir(e, x.id)}>{x.texto}</a>
  {/each}
</nav>

<style>
  .indice {
    position: sticky;
    top: 0;
    z-index: 4;
    display: flex;
    gap: 2px;
    margin: 0 calc(-1 * var(--sp-2));
    padding: 6px var(--sp-2);
    overflow-x: auto;
    scrollbar-width: none;
    background: color-mix(in srgb, var(--bg) 90%, transparent);
    backdrop-filter: blur(8px);
    border-bottom: 1px solid var(--border);
  }
  a {
    flex: none;
    padding: 4px 10px;
    font-size: var(--fs-sm);
    font-weight: 500;
    color: var(--text-2);
    text-decoration: none;
    border-radius: 999px;
    transition:
      background var(--dur-fast) var(--ease),
      color var(--dur-fast) var(--ease);
  }
  a:hover {
    color: var(--text-1);
    background: var(--surface-2);
  }
  a.on {
    color: var(--text-1);
    background: var(--surface-3);
  }
  @media (max-width: 860px) {
    .indice {
      display: none;
    }
  }
  /* Que el título de la sección no quede bajo la barra al saltar. */
  :global(:target),
  :global([id^="t-"]),
  :global([id^="sec-"]) {
    scroll-margin-top: 64px;
  }
</style>
