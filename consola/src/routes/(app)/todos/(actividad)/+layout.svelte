<script lang="ts">
  // Plan 0.8.1, B1 (15bb): «Actividad» de todos los clientes, con el mismo
  // criterio que dentro de un cliente: las órdenes en espera y lo que está en
  // curso, en pestañas (las direcciones de siempre).
  import type { Snippet } from "svelte";
  import { page } from "$app/state";
  import Pestanas, { type Pestana } from "$ui/componentes/Pestanas.svelte";
  import { SECCIONES_TODOS, enPestanaTodos } from "$lib/secciones";
  import { todos } from "$lib/todos.svelte";

  let { children }: { children: Snippet } = $props();
  const sec = SECCIONES_TODOS.find((s) => s.id === "actividad")!;
  const pendientes = $derived(todos.clientes.reduce((n, x) => n + (x.pendientes || 0), 0));
  const enMarcha = $derived(todos.progreso.reduce((n, x) => n + x.tareas.length, 0));
  const pestanas = $derived<Pestana[]>(
    sec.pestanas.map((p) => ({
      href: p.href,
      texto: p.texto,
      icono: p.icono,
      ...(p.href.endsWith("/ordenes") ? { cuenta: pendientes || undefined, tono: pendientes ? ("warn" as const) : undefined } : { cuenta: enMarcha || undefined }),
      activa: enPestanaTodos(p, page.url.pathname),
    })),
  );
  const activa = $derived(Math.max(0, pestanas.findIndex((p) => p.activa)));
</script>

<div class="tira">
  <Pestanas {pestanas} etiqueta="Actividad de todos los clientes" idPanel="panel-todos-actividad" prefijo="pestana-todos" />
</div>
<div class="panel" id="panel-todos-actividad" role="tabpanel" aria-labelledby="pestana-todos-{activa}">
  {@render children()}
</div>

<style>
  .tira {
    max-width: var(--content-max);
    margin: calc(-1 * var(--sp-2)) auto var(--sp-6);
  }
  @media (max-width: 640px) {
    .tira {
      margin-top: 0;
      margin-bottom: var(--sp-4);
    }
  }
</style>
