<script lang="ts">
  // «Retención» de un repositorio, desde donde se esté mirando (editor de
  // copias, página de la copia o del repositorio, ficha del equipo;
  // docs/editor-de-copias.md). En un almacén que la aplica él (v1.22), la
  // «Retención en el almacén»; si no, «Cambiar la retención» en el equipo
  // dueño, con la contraseña del repositorio. Solo guarda la regla: borrar
  // versiones sigue siendo «Aplicar retención», aparte.
  import OrdenDialog from "./OrdenDialog.svelte";
  import RetencionAlmacen from "./RetencionAlmacen.svelte";
  import EditorRetencion from "./EditorRetencion.svelte";
  import { destinoDe } from "$lib/repo";
  import { admitePlazos, almacenDe, copiaRegla, errorRegla, horarioDeCopias, REGLA_POR_DEFECTO, reglaDe, reglaParaOrden } from "$lib/retencion";
  import type { Cliente, Equipo, RepositorioResumen } from "$lib/tipos";

  interface Props {
    cliente: Cliente;
    equipo: Equipo;
    repo: RepositorioResumen;
    equipos: Equipo[];
    onclose: () => void;
  }
  let { cliente, equipo, repo, equipos, onclose }: Props = $props();

  // svelte-ignore state_referenced_locally
  const en = almacenDe(repo, destinoDe(equipo.resumen?.destinos, repo), equipos);
  // svelte-ignore state_referenced_locally
  let regla = $state(copiaRegla(reglaDe(repo) ?? REGLA_POR_DEFECTO));
</script>

{#if en?.admite}
  <RetencionAlmacen {cliente} {equipo} {repo} {en} {onclose} />
{:else}
  <OrdenDialog
    {cliente}
    {equipo}
    tipo="cambiar_retencion"
    cuerpo={{ repo: repo.id, ...reglaParaOrden(regla) }}
    titulo="Retención de «{repo.nombre}»"
    descripcion="Qué versiones guarda «{repo.nombre}». Solo se guarda la regla: las sobrantes se borran al «Aplicar retención»."
    repo={{ id: repo.id, nombre: repo.nombre }}
    valido={!errorRegla(regla, admitePlazos(equipo))}
    {onclose}
  >
    {#snippet campos()}
      <EditorRetencion id="ret-{repo.id}" bind:regla admite={admitePlazos(equipo)} {...horarioDeCopias(equipo.resumen?.copias, repo.id)} />
    {/snippet}
  </OrdenDialog>
{/if}
