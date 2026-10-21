<script lang="ts">
  // Plan 0.8.2, A1: aviso suave en Estado cuando ya hay algún servidor anotado en
  // la infraestructura y quedan equipos o destinos sin anotar. Se oculta con un
  // clic y se recuerda por persona en este navegador (`recordar.ts`, con
  // try/catch: sin almacenamiento, vuelve a salir). Solo para quien puede anotar.
  import { untrack } from "svelte";
  import { ChevronRight, ServerCog, X } from "@lucide/svelte";
  import { actual, app, puede } from "$lib/estado.svelte";
  import { destinosParaPasos } from "$lib/cadenas";
  import { catalogoDe } from "$lib/catalogoDestinos.svelte";
  import { sinAnotar } from "$lib/infra";
  import { lista, plural } from "$lib/formato";
  import { guardar, leerTexto } from "$lib/recordar";

  const c = $derived(actual.id);
  const admin = $derived(!!actual.cliente && puede.administrar(actual.cliente.rol));
  const destinos = $derived(
    destinosParaPasos(actual.equipos, catalogoDe(c)).map((d) => ({
      clave: d.clave,
      tipo: d.tipo,
      equipo: d.zona?.almacen.id ?? (d.destino?.tipo === "local" && !d.destino.red ? (actual.equipos.find((e) => d.equipos.includes(e.nombre))?.id ?? null) : null),
    })),
  );
  const faltan = $derived(sinAnotar(actual.infra, actual.equipos, destinos));
  const n = $derived(faltan.equipos.length + faltan.destinos.length);
  /** Lo que había al ocultarlo: si después falta algo más, vuelve a salir. */
  const clave = $derived(`infra.aviso.${app.cuenta?.id ?? ""}.${c}`);
  let oculto = $state("");
  $effect(() => {
    const k = clave;
    untrack(() => (oculto = leerTexto(k)));
  });
  const huella = $derived([...faltan.equipos.map((e) => e.id), ...faltan.destinos.map((d) => d.clave)].sort().join(","));
  function ocultar() {
    oculto = huella;
    guardar(clave, huella);
  }
  const partes = $derived(
    [faltan.equipos.length ? plural(faltan.equipos.length, "equipo", "equipos") : "", faltan.destinos.length ? plural(faltan.destinos.length, "destino", "destinos") : ""].filter(Boolean),
  );
</script>

{#if admin && n && oculto !== huella}
  <div class="notice notice-info aviso-infra" role="status">
    <ServerCog size={16} />
    <p>
      {lista(partes)} sin anotar en la infraestructura: no se sabe en qué servidor físico {n === 1 ? "está" : "están"}.
      <a class="notice-action" href="/c/{c}/ajustes/infraestructura{faltan.equipos[0] ? `?equipo=${encodeURIComponent(faltan.equipos[0].id)}` : ''}">Anotar<ChevronRight size={14} /></a>
    </p>
    <button type="button" class="icon-btn" aria-label="Ocultar este aviso" onclick={ocultar}><X size={15} /></button>
  </div>
{/if}

<style>
  .aviso-infra {
    align-items: flex-start;
  }
  .aviso-infra p {
    flex: 1;
    margin: 0;
  }
</style>
