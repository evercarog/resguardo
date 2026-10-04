<script lang="ts">
  // Región viva discreta para lectores de pantalla (docs/diseno.md §7): dice
  // `texto` solo cuando cambia, nunca al cargar la página (lo que ya estaba no
  // es una novedad). Para resultados que llegan solos: el titular del estado,
  // las fases y los hitos de una copia en marcha. Quien la usa decide cuándo
  // cambia el texto, para no anunciar cada segundo.
  let { texto }: { texto: string } = $props();

  let dicho = $state("");
  let primero = true;
  $effect(() => {
    const t = texto;
    if (primero) {
      primero = false;
      return;
    }
    dicho = t;
  });
</script>

<span class="sr-only" aria-live="polite" aria-atomic="true">{dicho}</span>
