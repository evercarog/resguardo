<script lang="ts">
  // «?» con la explicación del glosario y enlace a la ayuda.
  import InfoTip from "$ui/componentes/InfoTip.svelte";
  import { GLOSARIO } from "$lib/glosario";
  let { id }: { id: string } = $props();
</script>

<InfoTip entry={GLOSARIO[id]} href="/ayuda#{id}" />
