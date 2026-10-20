<script lang="ts">
  // «Ordenar nombres de carpeta» como aviso suave (plan 0.8.1, B3; plan-mejoras 15h): en el
  // almacén («N equipos tienen carpetas con nombres antiguos · Ordenar») y en cada repositorio
  // afectado. Dice antes qué espejos tendrían que volver a subir y cuánto (aproximado: lo que
  // ocupa cada repositorio en ese espejo o en el almacén), para decidir con datos. No es un
  // fallo: tono neutro. La orden es la de siempre (OrdenarCarpetas.svelte).
  import { FolderTree } from "@lucide/svelte";
  import { bytes, plural } from "$lib/formato";
  import { textoCarpetaVieja, type PorOrdenar } from "$lib/nombres";

  let {
    items,
    repo = false,
    onordenar,
  }: {
    items: PorOrdenar[];
    /** En la página de un repositorio: solo el suyo. */
    repo?: boolean;
    /** Abre «Ordenar nombres de carpeta» (con ese equipo); sin él, solo se informa. */
    onordenar?: (equipo: string) => void;
  } = $props();

  /** Lo que volverían a subir los espejos, juntando los de todos los equipos. */
  const porEspejo = $derived.by(() => {
    const m = new Map<string, { nombre: string; bytes: number; repos: number; vieja: string }>();
    for (const x of items)
      for (const r of x.resubidas) {
        const k = r.trabajo.id;
        const y = m.get(k) ?? { nombre: r.trabajo.nombre.trim() || "Espejo", bytes: 0, repos: 0, vieja: textoCarpetaVieja(r.trabajo) };
        y.bytes += r.bytes;
        y.repos += r.repos;
        m.set(k, y);
      }
    return [...m.values()].sort((a, b) => b.bytes - a.bytes);
  });
  const queMueve = (x: PorOrdenar) => [x.plan.usuario ? "la carpeta del equipo" : "", x.plan.repos.length ? plural(x.plan.repos.length, "repositorio", "repositorios") : ""].filter(Boolean).join(" y ");
</script>

{#if items.length}
  <div class="aviso" role="note">
    <FolderTree size={15} aria-hidden="true" />
    <div class="cuerpo">
      {#if repo}
        <p><strong>Su carpeta en el almacén tiene un nombre antiguo</strong>{#if items[0].plan.repos[0]} (<span class="mono">{items[0].plan.repos[0].de}</span> pasaría a <span class="mono">{items[0].plan.repos[0].propuesta}</span>){/if}. Funciona igual; ordenarla solo hace que se lea mejor.</p>
      {:else}
        <p><strong>{plural(items.length, "equipo tiene", "equipos tienen")} carpetas con nombres antiguos</strong>: {items.map((x) => `${x.en.equipo.nombre} (${queMueve(x)})`).join(", ")}. Funcionan igual; ordenarlas solo hace que se lean mejor.</p>
      {/if}
      {#if porEspejo.length}
        <p class="faint">Si se ordena, la carpeta nueva se sube entera a {porEspejo.length === 1 ? "este espejo" : "estos espejos"}:</p>
        <ul>
          {#each porEspejo as e (e.nombre)}
            <li>«{e.nombre}»: unos <strong class="num">{bytes(e.bytes)}</strong> ({plural(e.repos, "repositorio", "repositorios")}); {e.vieja}.</li>
          {/each}
        </ul>
      {:else}
        <p class="faint">Ningún espejo tendría que volver a subir nada.</p>
      {/if}
      {#if onordenar}
        <div class="acc">
          {#each items as x (x.en.equipo.id)}
            <button class="btn btn-sm" onclick={() => onordenar?.(x.en.equipo.id)}>{repo ? "Ordenar su carpeta…" : items.length === 1 ? "Ordenar…" : `Ordenar ${x.en.equipo.nombre}…`}</button>
          {/each}
        </div>
      {/if}
    </div>
  </div>
{/if}

<style>
  .aviso {
    display: flex;
    gap: 10px;
    align-items: flex-start;
    padding: 10px 14px;
    border-radius: var(--radius);
    background: var(--neutral-soft);
    color: var(--text-2);
    font-size: var(--fs-sm);
    line-height: 1.5;
  }
  .aviso :global(svg) {
    flex: none;
    margin-top: 3px;
  }
  .cuerpo {
    display: grid;
    gap: 4px;
    min-width: 0;
  }
  p {
    margin: 0;
    color: var(--text-1);
    overflow-wrap: anywhere;
  }
  p.faint {
    color: var(--text-2);
  }
  ul {
    margin: 0;
    padding-left: 18px;
    color: var(--text-1);
  }
  .acc {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin-top: 4px;
  }
</style>
