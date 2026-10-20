<script lang="ts">
  // «Todos los clientes» y sus secciones (plan 0.8.0, D4, 14xx): lo de todos los
  // clientes de la cuenta se carga una vez aquí (GET /api/panel y lo que está en
  // marcha), y cada sección lo enseña a su manera. Solo los clientes de los que la
  // cuenta es miembro: el servidor lo comprueba en cada petición.
  import { onMount, type Snippet } from "svelte";
  import { cargarClientes } from "$lib/estado.svelte";
  import { vigilarTodos } from "$lib/todos.svelte";

  let { children }: { children: Snippet } = $props();

  onMount(() => {
    void cargarClientes().catch(() => {}); // el fallo lo enseña la página que necesita la lista
    return vigilarTodos();
  });
</script>

{@render children()}
