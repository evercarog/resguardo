<script lang="ts">
  // «Añadir una zona» en un almacén (tarea 7b): otra carpeta (otro disco) con
  // su propio servidor de solo añadir en su puerto. Orden `guarda_copias
  // { zona: { nombre?, carpeta, puerto } }` con la clave de administración.
  import OrdenDialog from "./OrdenDialog.svelte";
  import { errorCarpetaDestino } from "$lib/ganchos";
  import { errorZonaNueva, puertoParaZona } from "$lib/destinos";
  import type { Cliente, Equipo } from "$lib/tipos";

  let { cliente, equipo, onclose, alTerminar }: { cliente: Cliente; equipo: Equipo; onclose: () => void; alTerminar?: () => void } = $props();

  const win = $derived(/windows/i.test(equipo.so));
  // El puerto propuesto al abrir (después manda lo que se escriba).
  const propuesto = () => puertoParaZona(equipo);
  let nueva = $state({ nombre: "", carpeta: "", puerto: propuesto() });
  const errorCarpeta = $derived(nueva.carpeta.trim() ? (errorCarpetaDestino(nueva.carpeta, win) ?? errorZonaNueva(equipo, nueva.carpeta, Number(nueva.puerto))) : null);
  const cuerpo = $derived({ zona: { ...(nueva.nombre.trim() ? { nombre: nueva.nombre.trim() } : {}), carpeta: nueva.carpeta.trim(), puerto: Number(nueva.puerto) } });
</script>

<OrdenDialog
  {cliente}
  {equipo}
  tipo="guarda_copias"
  {cuerpo}
  titulo="Añadir una zona en {equipo.nombre}"
  descripcion="Otra carpeta (mejor en otro disco) que {equipo.nombre} sirve con su propio servidor de solo añadir, en su puerto y con el mismo certificado. Será un destino más al crear un repositorio."
  valido={!!nueva.carpeta.trim() && !errorCarpeta}
  {onclose}
  {alTerminar}
>
  {#snippet campos()}
    <div class="field">
      <label class="field-label" for="z-carpeta">Carpeta o disco</label>
      <input id="z-carpeta" class="input mono" bind:value={nueva.carpeta} placeholder={win ? "E:\\Resguardo" : "/mnt/disco2/resguardo"} spellcheck="false" />
      {#if errorCarpeta}<p class="error-campo">{errorCarpeta}</p>{:else}<span class="field-hint">Solo SYSTEM y los administradores podrán entrar en ella.</span>{/if}
    </div>
    <div class="fila">
      <div class="field">
        <label class="field-label" for="z-nombre">Nombre <span class="faint">(opcional)</span></label>
        <input id="z-nombre" class="input" bind:value={nueva.nombre} maxlength="60" placeholder="Disco E" />
      </div>
      <div class="field">
        <label class="field-label" for="z-puerto">Puerto</label>
        <input id="z-puerto" class="input num corto" type="number" min="1024" max="65535" bind:value={nueva.puerto} />
      </div>
    </div>
    <p class="faint pequeno">El cortafuegos de {equipo.nombre} abrirá también este puerto, con la misma regla que el principal ({equipo.resumen?.guarda_copias?.solo_red_local ? "solo redes internas" : "abierto a otras sedes"}). Si el puerto está ocupado, no se crea.</p>
  {/snippet}
</OrdenDialog>

<style>
  .fila {
    display: grid;
    grid-template-columns: 1fr auto;
    gap: var(--sp-3);
  }
  .pequeno {
    font-size: var(--fs-xs);
    margin: 0;
  }
  .corto {
    max-width: 8rem;
  }
  @media (max-width: 480px) {
    .fila {
      grid-template-columns: 1fr;
    }
  }
</style>
