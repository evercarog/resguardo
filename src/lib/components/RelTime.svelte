<script lang="ts">
  // Tiempo relativo («hace 5 minutos») con la fecha y hora exactas en el tooltip.
  import { formatDate, formatRelative } from "$lib/format";

  let { iso }: { iso: string } = $props();
</script>

<time datetime={iso} title={formatDate(iso)}>{formatRelative(iso)}</time>
