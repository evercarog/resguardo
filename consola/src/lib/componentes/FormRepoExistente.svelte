<script lang="ts">
  // Dónde está un repositorio que ya existe (de la app de escritorio, de otro
  // programa o hecho a mano) y su contraseña: lo usan «Usar uno que ya existe»
  // y «Traer historial». Todo viaja sellado solo para el equipo. También la
  // ventana del equipo en modo local (por eso de direccion.ts: sin la API).
  import { ETIQUETA_TIPO_EXISTENTE, partirDireccion, type RepoExistente, type TipoExistente } from "$lib/direccion";
  import CampoClave from "./CampoClave.svelte";

  let {
    repo = $bindable(),
    id = "rx",
    nombreEquipo = "el equipo",
    etiquetaTipo = "Dónde está",
    local = false,
  }: { repo: RepoExistente; id?: string; nombreEquipo?: string; etiquetaTipo?: string; local?: boolean } = $props();

  const EJEMPLO: Record<TipoExistente, string> = {
    rest: "http://192.168.1.20:8000/Contabilidad",
    local: "D:\\Copias\\Contabilidad",
    b2: "mi-bucket:contabilidad",
    s3: "s3.amazonaws.com/mi-bucket/contabilidad",
    sftp: "usuario@servidor:/copias/contabilidad",
  };
  const partes = $derived(repo.direccion.trim() ? partirDireccion(repo.tipo, repo.direccion) : null);
  const nube = $derived(repo.tipo === "s3" || repo.tipo === "b2");
</script>

<div class="existente">
  <div class="field">
    <label class="field-label" for="{id}-tipo">{etiquetaTipo}</label>
    <select id="{id}-tipo" class="input" bind:value={repo.tipo}>
      {#each Object.entries(ETIQUETA_TIPO_EXISTENTE) as [k, t] (k)}<option value={k}>{t}</option>{/each}
    </select>
  </div>
  <div class="field">
    <label class="field-label" for="{id}-dir">Dirección del repositorio</label>
    <input id="{id}-dir" class="input mono" bind:value={repo.direccion} spellcheck="false" autocomplete="off" placeholder={EJEMPLO[repo.tipo]} />
    <span class="field-hint">
      La misma que usaba la app de escritorio, con la carpeta del repositorio al final.
      {#if partes?.ruta}Se usará la carpeta «{partes.ruta}».{:else if partes}Sin carpeta: el repositorio es todo el destino.{/if}
    </span>
  </div>
  {#if repo.tipo !== "local" && repo.tipo !== "sftp"}
    <div class="dos">
      <div class="field">
        <label class="field-label" for="{id}-usuario">{nube ? "Id de la clave" : "Usuario (si tiene)"}</label>
        <input id="{id}-usuario" class="input mono" bind:value={repo.usuario} autocomplete="off" spellcheck="false" />
      </div>
      <CampoClave id="{id}-secreto" etiqueta={nube ? "Clave secreta" : "Contraseña del servidor (si tiene)"} bind:value={repo.secreto} />
    </div>
  {/if}
  {#if repo.tipo === "rest" && repo.direccion.trim().toLowerCase().startsWith("https")}
    <details class="avanzado">
      <summary>Certificado propio del servidor (opcional)</summary>
      <div class="field">
        <label class="field-label" for="{id}-ca">Autoridad TLS (PEM)</label>
        <textarea id="{id}-ca" class="input mono" rows="3" bind:value={repo.ca} placeholder="-----BEGIN CERTIFICATE-----"></textarea>
        <span class="field-hint">Solo si el servidor usa un certificado propio (no uno de una autoridad conocida).</span>
      </div>
    </details>
  {/if}
  <CampoClave requerido id="{id}-contrasena" etiqueta="Contraseña del repositorio" ayuda="La que abre las copias: la del kit de recuperación o la que guardaba la app de escritorio." bind:value={repo.contrasena} />
  {#if local}
    <p class="faint nota">La dirección, las credenciales y la contraseña van al servicio de este equipo por su canal local: no salen de él.</p>
  {:else}
    <p class="faint nota">La dirección, las credenciales y la contraseña van selladas solo para {nombreEquipo}: el servidor no las ve ni las guarda.</p>
  {/if}
</div>

<style>
  .existente {
    display: flex;
    flex-direction: column;
    gap: var(--sp-4);
  }
  .dos {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
    gap: var(--sp-4);
  }
  .avanzado summary {
    cursor: pointer;
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  .avanzado .field {
    margin-top: var(--sp-3);
  }
  .nota {
    margin: 0;
    font-size: var(--fs-xs);
  }
</style>
