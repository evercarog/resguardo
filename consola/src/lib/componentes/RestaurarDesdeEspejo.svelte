<script lang="ts">
  // «Restaurar desde el espejo…» (docs/espejo.md §3e): si el almacén se pierde,
  // cada repositorio está entero en cada destino del espejo, en
  // `<destino>/<usuario>/<repo>`, con la misma contraseña (la del kit). Aquí
  // se elige el destino y el repositorio; después, «Restaurar en otro equipo»
  // con los datos del kit ya puestos lo abre en el equipo que elijas.
  import { ArrowRight, LifeBuoy } from "@lucide/svelte";
  import Modal from "$ui/componentes/Modal.svelte";
  import { claveEspejo } from "$lib/cripto/ordenes";
  import { etiquetaCarpeta, kitDesdeEspejo, nombresRepos, nombreTipoNube, type DestinoEspejoResumen, type KitDesdeEspejo } from "$lib/espejo";
  import type { Cliente, Equipo } from "$lib/tipos";
  import RestaurarEnOtro from "./RestaurarEnOtro.svelte";
  import Ayuda from "./Ayuda.svelte";

  let {
    cliente,
    almacen,
    equipos,
    nombre = (r: string) => r,
    onclose,
  }: { cliente: Cliente; almacen: Equipo; equipos: Equipo[]; nombre?: (r: string) => string; onclose: () => void } = $props();

  const g = $derived(almacen.resumen?.guarda_copias);
  const destinos = $derived((g?.espejo?.destinos ?? []) as DestinoEspejoResumen[]);
  const tipoDe = (d: DestinoEspejoResumen) => g?.nubes?.find((n) => n.nombre === d.nube)?.tipo ?? null;
  let clave = $state("");
  let repo = $state("");
  const destino = $derived(destinos.find((d) => claveEspejo(d) === clave) ?? null);
  // Los repositorios que van a ese destino (todos, o su selección).
  const repos = $derived(destino ? (Array.isArray(destino.repos) ? [...destino.repos].sort() : nombresRepos(g?.repositorios)) : []);
  const kit = $derived<KitDesdeEspejo | null>(destino && repo ? kitDesdeEspejo(destino, tipoDe(destino), repo) : null);
  let seguir = $state<KitDesdeEspejo | null>(null);
  $effect(() => {
    if (!clave && destinos.length) clave = claveEspejo(destinos[0]);
  });
  $effect(() => {
    if (repo && !repos.includes(repo)) repo = "";
  });
  const etiqueta = (d: DestinoEspejoResumen) => (d.tipo === "nube" ? `${d.nube} (${nombreTipoNube(tipoDe(d) ?? "")}) · ${d.carpeta}` : `${etiquetaCarpeta(d.carpeta)} · ${d.carpeta}`);
</script>

{#if seguir}
  <RestaurarEnOtro {cliente} {equipos} kitInicial={seguir} {onclose} />
{:else}
  <Modal labelledby="t-desde-espejo" {onclose} width={560}>
    <div class="dlg-title">
      <span class="ticon"><LifeBuoy size={18} /></span>
      <div>
        <h2 id="t-desde-espejo">Restaurar desde el espejo de {almacen.nombre}</h2>
        <p>Si {almacen.nombre} se pierde, cada repositorio está entero en su espejo. Se abre con la contraseña de su kit de recuperación, en el equipo que elijas. <Ayuda id="espejo" /></p>
      </div>
    </div>
    <div class="form">
      {#if !destinos.length}
        <p class="faint">Este equipo no tiene espejo.</p>
      {:else}
        <div class="field">
          <label class="field-label" for="de-destino">Destino del espejo</label>
          <select id="de-destino" class="input" bind:value={clave}>
            {#each destinos as d (claveEspejo(d))}<option value={claveEspejo(d)}>{etiqueta(d)}</option>{/each}
          </select>
          {#if destino?.ultima}<span class="field-hint">Copiado al espejo por última vez: {new Date(destino.ultima).toLocaleString("es")}. Lo copiado después en el almacén no está en el espejo.</span>{/if}
        </div>
        <div class="field">
          <label class="field-label" for="de-repo">Repositorio</label>
          <select id="de-repo" class="input" bind:value={repo}>
            <option value="" disabled>Elige uno</option>
            {#each repos as r (r)}<option value={r}>{nombre(r)}{nombre(r) !== r ? ` · ${r}` : ""}</option>{/each}
          </select>
        </div>
        {#if kit}
          <div class="notice notice-info"><p>{kit.nota}</p></div>
          <p class="faint dir">En el espejo: <code>{kit.descargar ? `${destino?.carpeta}/${repo}` : kit.tipo === "local" ? `${kit.donde}\\${repo}`.replace(/[\\/]+/g, kit.donde.includes("/") ? "/" : "\\") : `${kit.donde}/${repo}`}</code></p>
        {/if}
      {/if}
      <footer>
        <button type="button" class="btn btn-ghost" onclick={onclose}>Cancelar</button>
        <button type="button" class="btn btn-primary" disabled={!kit} onclick={() => (seguir = kit)}>Seguir<ArrowRight size={15} /></button>
      </footer>
    </div>
  </Modal>
{/if}

<style>
  .dir {
    margin: 0;
    overflow-wrap: anywhere;
  }
</style>
