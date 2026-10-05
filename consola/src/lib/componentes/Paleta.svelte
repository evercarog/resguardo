<script lang="ts">
  // Paleta de órdenes (Ctrl+K / ⌘K): escribir para ir a una sección, un
  // cliente, un equipo, un repositorio o una copia del cliente abierto, o
  // para lanzar una acción («Copiar ahora en…», «Restaurar archivos de…»,
  // «Añadir equipo», «Nuevo repositorio»). Las acciones abren los mismos
  // diálogos que sus botones de siempre: la paleta no manda nada por su
  // cuenta. Flechas para moverse, Intro para ir, Escape para cerrar.
  import { tick, untrack } from "svelte";
  import { fade, fly } from "svelte/transition";
  import Anuncio from "./Anuncio.svelte";
  import { goto } from "$app/navigation";
  import { ArrowRight, Building2, Layers, CornerDownLeft, Database, DatabaseZap, FileText, History, Monitor, NotebookPen, Play, Plus, Search } from "@lucide/svelte";
  import { dur } from "$ui/movimiento";
  import { actual, app, puede } from "$lib/estado.svelte";
  import { saludEquipo } from "$lib/salud";
  import { atajos, MOD } from "$lib/atajos.svelte";
  import { acciones } from "$lib/acciones.svelte";
  import { ICONO_SECCION } from "$lib/iconos";
  import { asegurarIndice, notas } from "$lib/notas.svelte";

  interface Item {
    grupo: string;
    texto: string;
    sub?: string;
    /** Adónde lleva… */
    href?: string;
    /** …o qué abre (un diálogo de siempre). */
    accion?: () => void;
    /** …o qué escribe en el campo (para elegir después: «Copiar ahora en…»). */
    rellenar?: string;
    icono: typeof Search;
    /** Texto en el que se busca (sin tildes, en minúsculas). */
    claves: string;
  }

  let texto = $state("");
  let elegido = $state(0);
  let entrada = $state<HTMLInputElement>();
  let lista = $state<HTMLUListElement>();

  const sinTildes = (s: string) => s.normalize("NFD").replace(/[̀-ͯ]/g, "").toLowerCase();

  const todos = $derived.by((): Item[] => {
    const c = actual.id;
    const out: Item[] = [];
    const rol = actual.cliente?.rol;
    if (c) {
      const secciones: [string, string, typeof Search][] = [
        ["Estado", "", ICONO_SECCION.estado],
        ["Equipos", "/equipos", ICONO_SECCION.equipos],
        ["Copias", "/copias", ICONO_SECCION.copias],
        ["Repositorios y destinos", "/repositorios", ICONO_SECCION.repositorios],
        ["Restaurar archivos", "/restaurar", ICONO_SECCION.restaurar],
        ["Órdenes", "/ordenes", ICONO_SECCION.ordenes],
        ["Avisos", "/avisos", ICONO_SECCION.avisos],
        ["Informes", "/informes", ICONO_SECCION.informes],
        ...(puede.ordenar(rol) ? ([["Actividad", "/auditoria", ICONO_SECCION.actividad]] as [string, string, typeof Search][]) : []),
        ...(puede.administrar(rol) ? ([["Servidor", "/servidor", ICONO_SECCION.servidor]] as [string, string, typeof Search][]) : []),
        ...(puede.propietario(rol) ? ([["Personas y ajustes", "/ajustes", ICONO_SECCION.ajustes]] as [string, string, typeof Search][]) : []),
      ];
      // Acciones: abren lo mismo que sus botones (y piden lo mismo).
      const activos = actual.equipos.filter((e) => e.confirmado !== false && e.modo !== "trasladado");
      if (puede.administrar(rol)) {
        out.push({ grupo: "Acciones", texto: "Añadir equipo", sub: "instalador listo o código", href: `/c/${c}/emparejar`, icono: Plus, claves: "anadir equipo nuevo emparejar instalar agente" });
        if (activos.length) out.push({ grupo: "Acciones", texto: "Nuevo repositorio", sub: "dónde guardar las copias", accion: () => (acciones.nuevoRepo = true), icono: DatabaseZap, claves: "nuevo repositorio crear almacen destino" });
      }
      if (puede.ordenar(rol)) {
        const copias = activos.flatMap((e) => (e.resumen?.copias ?? []).map((k) => ({ e, k })));
        const repos = activos.flatMap((e) => (e.resumen?.repositorios ?? []).map((r) => ({ e, r })));
        if (copias.length) out.push({ grupo: "Acciones", texto: "Copiar ahora en…", sub: "elige la copia", rellenar: "copiar ahora ", icono: Play, claves: "copiar ahora en" });
        if (repos.length) out.push({ grupo: "Acciones", texto: "Restaurar archivos de…", sub: "elige el repositorio", rellenar: "restaurar ", icono: History, claves: "restaurar archivos de recuperar" });
        for (const { e, k } of copias)
          out.push({ grupo: "Copiar ahora", texto: `Copiar ahora «${k.nombre}»`, sub: e.nombre, accion: () => (acciones.copiar = { equipo: e, repo: k.repo, copia: k.id, nombre: k.nombre }), icono: Play, claves: sinTildes(`copiar ahora ${k.nombre} ${e.nombre}`) });
        for (const { e, r } of repos)
          out.push({ grupo: "Restaurar", texto: `Restaurar archivos de «${r.nombre}»`, sub: e.nombre, href: `/c/${c}/restaurar?equipo=${encodeURIComponent(e.id)}&repo=${encodeURIComponent(r.id)}`, icono: History, claves: sinTildes(`restaurar archivos ${r.nombre} ${e.nombre}`) });
      }
      for (const [t, r, ic] of secciones) out.push({ grupo: "Ir a", texto: t, sub: actual.cliente?.nombre, href: `/c/${c}${r}`, icono: ic, claves: `${sinTildes(t)} ir a` });
      for (const e of actual.equipos) {
        const s = saludEquipo(e);
        out.push({ grupo: "Equipos", texto: e.nombre, sub: `${s.texto} · ${e.so}`, href: `/c/${c}/equipos/${e.id}`, icono: Monitor, claves: sinTildes(`${e.nombre} ${e.so} ${(e.etiquetas ?? []).join(" ")}`) });
      }
      for (const e of actual.equipos)
        for (const r of e.resumen?.repositorios ?? [])
          out.push({ grupo: "Repositorios", texto: r.nombre, sub: e.nombre, href: `/c/${c}/equipos/${e.id}/repositorios/${encodeURIComponent(r.id)}`, icono: Database, claves: sinTildes(`${r.nombre} ${e.nombre}`) });
      for (const e of actual.equipos)
        for (const k of e.resumen?.copias ?? [])
          out.push({ grupo: "Copias", texto: k.nombre, sub: e.nombre, href: `/c/${c}/equipos/${e.id}/copias/${encodeURIComponent(k.id)}`, icono: ICONO_SECCION.copias, claves: sinTildes(`${k.nombre} ${e.nombre}`) });
      // v1.40: observaciones, por su primera línea (nunca el texto entero ni los comentarios).
      if (notas.cliente === c)
        for (const n of Object.values(notas.porClave)) {
          if (!n.titulo) continue;
          const [eid, id] = n.tipo === "repositorio" || n.tipo === "copia" ? [n.objeto.slice(0, n.objeto.indexOf("/")), n.objeto.slice(n.objeto.indexOf("/") + 1)] : [n.objeto, n.objeto];
          const e = actual.equipos.find((x) => x.id === eid);
          const sitio =
            n.tipo === "cliente"
              ? { nombre: actual.cliente?.nombre ?? "Cliente", href: `/c/${c}` }
              : n.tipo === "equipo" && e
                ? { nombre: e.nombre, href: `/c/${c}/equipos/${e.id}` }
                : n.tipo === "repositorio" && e
                  ? { nombre: `${e.resumen?.repositorios?.find((r) => r.id === id)?.nombre ?? id} · ${e.nombre}`, href: `/c/${c}/equipos/${e.id}/repositorios/${encodeURIComponent(id)}` }
                  : n.tipo === "copia" && e
                    ? { nombre: `${e.resumen?.copias?.find((k) => k.id === id)?.nombre ?? id} · ${e.nombre}`, href: `/c/${c}/equipos/${e.id}/copias/${encodeURIComponent(id)}` }
                    : n.tipo === "destino"
                      ? { nombre: actual.equipos.flatMap((x) => x.resumen?.destinos ?? []).find((d) => d.id === n.objeto)?.nombre ?? n.objeto, href: `/c/${c}/repositorios` }
                      : null;
          if (sitio) out.push({ grupo: "Observaciones", texto: n.titulo, sub: sitio.nombre, href: sitio.href, icono: NotebookPen, claves: sinTildes(`observaciones notas ${n.titulo} ${sitio.nombre}`) });
        }
    }
    for (const x of app.clientes)
      if (x.id !== c) out.push({ grupo: "Clientes", texto: x.nombre, sub: `${x.equipos} ${x.equipos === 1 ? "equipo" : "equipos"}`, href: `/c/${x.id}`, icono: Building2, claves: sinTildes(x.nombre) });
    // v1.38: el Estado de todos los clientes juntos (con más de uno), y la lista para gestionarlos.
    // (el primero de «Ir a», para que se vea sin escribir nada)
    if (app.clientes.length > 1) {
      const i = out.findIndex((x) => x.grupo === "Ir a");
      out.splice(i < 0 ? out.length : i, 0, { grupo: "Ir a", texto: "Todos los clientes", sub: "estado y mapa de todos", href: "/todos", icono: Layers, claves: "todos los clientes estado panel mapa global resumen" });
    }
    out.push({ grupo: "Ir a", texto: "Lista de clientes", href: "/clientes", icono: Building2, claves: "lista de clientes gestionar nuevo cliente" });
    out.push({ grupo: "Ir a", texto: "Ayuda", href: "/ayuda", icono: FileText, claves: "ayuda" });
    out.push({ grupo: "Ir a", texto: "Mi cuenta y servidor", href: "/ajustes", icono: FileText, claves: "mi cuenta y servidor ajustes" });
    return out;
  });

  /** Sin texto, lo de siempre (secciones y equipos); con texto, todo lo que lleve todas las palabras. */
  const vistos = $derived.by(() => {
    const t = sinTildes(texto.trim());
    if (!t) return todos.filter((x) => x.grupo === "Acciones" || x.grupo === "Ir a" || x.grupo === "Equipos").slice(0, 18);
    const palabras = t.split(/\s+/);
    return todos
      // «Copiar ahora en…» ya está escrito: sobran el atajo y se ven las copias.
      .filter((x) => palabras.every((p) => x.claves.includes(p)) && !(x.rellenar && t.startsWith(sinTildes(x.rellenar.trim()))))
      .sort((a, b) => Number(!a.claves.startsWith(palabras[0])) - Number(!b.claves.startsWith(palabras[0])))
      .slice(0, 30);
  });
  // Agrupados en el orden en que aparecen, con su índice global para el teclado.
  const grupos = $derived.by(() => {
    const m = new Map<string, { item: Item; i: number }[]>();
    vistos.forEach((item, i) => (m.get(item.grupo) ?? m.set(item.grupo, []).get(item.grupo)!).push({ item, i }));
    return [...m.entries()];
  });

  $effect(() => {
    void texto;
    elegido = 0;
  });

  async function mover(d: number) {
    if (!vistos.length) return;
    elegido = (elegido + d + vistos.length) % vistos.length;
    await tick();
    lista?.querySelector(`[data-i="${elegido}"]`)?.scrollIntoView({ block: "nearest" });
  }
  function ir(x: Item | undefined) {
    if (!x) return;
    if (x.rellenar) {
      texto = x.rellenar;
      entrada?.focus();
      return;
    }
    cerrar();
    if (x.accion) x.accion();
    else if (x.href) void goto(x.href);
  }
  function cerrar() {
    atajos.paleta = false;
    texto = "";
  }
  function teclas(e: KeyboardEvent) {
    if (e.key === "ArrowDown") (e.preventDefault(), mover(1));
    else if (e.key === "ArrowUp") (e.preventDefault(), mover(-1));
    else if (e.key === "Enter") (e.preventDefault(), ir(vistos[elegido]));
    else if (e.key === "Escape") (e.preventDefault(), cerrar());
    else if (e.key === "Tab") e.preventDefault();
  }
  $effect(() => {
    if (atajos.paleta) void tick().then(() => entrada?.focus());
  });
  $effect(() => {
    const c = atajos.paleta ? actual.id : "";
    if (c) untrack(() => void asegurarIndice(c));
  });
  // Al cerrar, el foco vuelve a donde estaba.
  let previo: HTMLElement | null = null;
  $effect.pre(() => {
    if (atajos.paleta) previo = document.activeElement as HTMLElement | null;
    else if (previo && document.contains(previo)) (previo.focus(), (previo = null));
  });
</script>

{#if atajos.paleta}
  <div class="velo" role="presentation" onclick={cerrar} transition:fade={{ duration: dur(120) }}></div>
  <div class="paleta card" role="dialog" aria-modal="true" aria-label="Buscar o hacer…" transition:fly={{ y: -8, duration: dur(160) }}>
    <div class="campo">
      <Search size={16} />
      <input
        bind:this={entrada}
        bind:value={texto}
        onkeydown={teclas}
        type="text"
        placeholder="Busca o escribe una acción: «copiar ahora», «restaurar»…"
        role="combobox"
        aria-expanded="true"
        aria-controls="paleta-lista"
        aria-activedescendant={vistos.length ? `paleta-${elegido}` : undefined}
        aria-autocomplete="list"
        autocomplete="off"
        spellcheck="false"
      />
      <kbd>Esc</kbd>
    </div>
    <Anuncio texto={texto.trim() ? (vistos.length ? `${vistos.length} ${vistos.length === 1 ? "resultado" : "resultados"}` : "Nada coincide") : ""} />
    <ul id="paleta-lista" role="listbox" aria-label="Resultados" bind:this={lista}>
      {#each grupos as [g, xs] (g)}
        <li role="presentation" class="grupo">{g}</li>
        {#each xs as { item, i } (item.href ?? `${item.texto}|${item.sub ?? ""}`)}
          <!-- El teclado va por el campo (patrón combobox): las opciones no se enfocan. -->
          <!-- svelte-ignore a11y_click_events_have_key_events -->
          <li
            id="paleta-{i}"
            data-i={i}
            role="option"
            aria-selected={i === elegido}
            class:on={i === elegido}
            onpointermove={() => (elegido = i)}
            onclick={() => ir(item)}
          >
            <item.icono size={16} />
            <span class="t">{item.texto}</span>
            {#if item.sub}<span class="s">{item.sub}</span>{/if}
            {#if i === elegido}<ArrowRight size={14} class="flecha" />{/if}
          </li>
        {/each}
      {:else}
        <li class="nada" role="presentation">Nada coincide con «{texto}».</li>
      {/each}
    </ul>
    <p class="pie"><span><kbd>↑</kbd><kbd>↓</kbd> moverse</span><span><kbd><CornerDownLeft size={11} /></kbd> ir</span><span><kbd>{MOD}</kbd><kbd>K</kbd> abrir o cerrar</span><span><kbd>?</kbd> atajos</span></p>
  </div>
{/if}

<style>
  .velo {
    position: fixed;
    inset: 0;
    z-index: 40;
    background: var(--overlay);
  }
  .paleta {
    position: fixed;
    top: min(14vh, 120px);
    left: 50%;
    z-index: 41;
    display: flex;
    flex-direction: column;
    width: min(600px, calc(100vw - 24px));
    max-height: min(70vh, 560px);
    translate: -50% 0;
    overflow: hidden;
    border-radius: var(--radius-xl);
    box-shadow: var(--shadow-lg);
  }
  .campo {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 0 var(--sp-4);
    color: var(--text-3);
    border-bottom: 1px solid var(--border);
  }
  input {
    flex: 1;
    min-width: 0;
    height: 52px;
    font: inherit;
    font-size: 15px;
    color: var(--text-1);
    background: none;
    border: none;
    outline: none;
  }
  input::placeholder {
    color: var(--text-3);
  }
  ul {
    flex: 1;
    margin: 0;
    padding: 6px;
    overflow-y: auto;
    list-style: none;
  }
  .grupo {
    padding: 10px 10px 4px;
    font-size: var(--fs-overline);
    font-weight: 600;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--text-3);
  }
  [role="option"] {
    display: flex;
    align-items: center;
    gap: 10px;
    min-height: 38px;
    padding: 0 10px;
    color: var(--text-2);
    border-radius: var(--radius);
    cursor: pointer;
  }
  [role="option"].on {
    color: var(--text-1);
    background: var(--surface-2);
  }
  .t {
    color: var(--text-1);
    font-weight: 500;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .s {
    flex: 1;
    min-width: 0;
    font-size: var(--fs-sm);
    color: var(--text-3);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .paleta :global(.flecha) {
    flex: none;
    margin-left: auto;
    color: var(--text-3);
  }
  .nada {
    padding: var(--sp-6) var(--sp-4);
    text-align: center;
    color: var(--text-3);
  }
  .pie {
    display: flex;
    flex-wrap: wrap;
    gap: 6px 16px;
    margin: 0;
    padding: 8px var(--sp-4);
    font-size: var(--fs-xs);
    color: var(--text-3);
    background: var(--bg-subtle);
    border-top: 1px solid var(--border);
  }
  .pie span {
    display: inline-flex;
    align-items: center;
    gap: 3px;
  }
  .pie kbd,
  .campo kbd {
    min-width: 18px;
    padding: 0 4px;
    font-size: 11px;
    line-height: 17px;
    font-weight: 500;
    color: var(--text-2);
  }
  .pie kbd :global(svg) {
    vertical-align: -1px;
  }
  @media (max-width: 640px) {
    .pie {
      display: none;
    }
    .paleta {
      top: 12px;
    }
  }
</style>
