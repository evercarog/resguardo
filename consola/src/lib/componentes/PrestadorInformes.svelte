<script lang="ts">
  // Servidor → «Informes para los clientes» (propietario del servidor; plan 0.8.0, G1):
  // el nombre y el logo de quien presta el servicio, para el informe ejecutivo en
  // PDF de todos los clientes (cada cliente puede usar el de su marca en su lugar).
  // El logo se convierte a PNG en este navegador antes de mandarlo (lib/marca.ts):
  // ningún SVG llega al servidor.
  import { onMount } from "svelte";
  import { Check, FileText, ImagePlus, Trash2 } from "@lucide/svelte";
  import * as api from "$lib/api";
  import { avisar, fallo } from "$lib/avisos.svelte";
  import { logoAPng, TIPOS_LOGO } from "$lib/marca";
  import type { PrestadorInforme } from "$lib/tipos";
  import BotonCargando from "./BotonCargando.svelte";

  /** null: servidor anterior; undefined: cargando. */
  let p = $state<PrestadorInforme | null | undefined>(undefined);
  let nombre = $state("");
  let nuevo = $state<{ base64: string; vista: string } | null>(null);
  let quitar = $state(false);
  let error = $state("");
  let guardando = $state(false);
  let entrada = $state<HTMLInputElement>();

  onMount(async () => {
    try {
      p = await api.prestadorInformes();
      nombre = p.nombre ?? "";
    } catch {
      p = null;
    }
  });

  const vista = $derived(nuevo?.vista ?? (quitar ? null : (p?.logo ?? null)));
  const cambiado = $derived(!!p && (!!nuevo || quitar || (p.nombre ?? "") !== nombre.trim()));

  async function elegir(e: Event) {
    const f = (e.currentTarget as HTMLInputElement).files?.[0];
    (e.currentTarget as HTMLInputElement).value = "";
    if (!f) return;
    error = "";
    try {
      const r = await logoAPng(f);
      nuevo = { base64: r.base64, vista: r.vista };
      quitar = false;
    } catch (x) {
      error = (x as Error).message;
    }
  }

  async function guardar(e: SubmitEvent) {
    e.preventDefault();
    guardando = true;
    try {
      p = await api.cambiarPrestadorInformes({ nombre: nombre.trim(), ...(nuevo ? { logo: nuevo.base64 } : {}), ...(quitar ? { quitar_logo: true } : {}) });
      nombre = p.nombre ?? "";
      nuevo = null;
      quitar = false;
      avisar("Datos del prestador guardados: los llevarán los próximos informes.");
    } catch (err) {
      fallo(err);
    } finally {
      guardando = false;
    }
  }
</script>

{#if p}
  <section class="card p" id="prestador" aria-labelledby="t-prestador">
    <div class="cab">
      <span class="card-icon"><FileText size={18} /></span>
      <div class="cab-texto">
        <h2 id="t-prestador">Informes para los clientes</h2>
        <p class="faint">Tu nombre y tu logo en el informe ejecutivo en PDF de todos los clientes. Cada cliente puede elegir en Informes el logo de su marca en su lugar.</p>
      </div>
    </div>
    <form class="form" onsubmit={guardar}>
      <div class="field">
        <label class="field-label" for="pr-nombre">Nombre de quien presta el servicio</label>
        <input id="pr-nombre" class="input" bind:value={nombre} maxlength="80" placeholder="Soporte Ejemplo" />
        <span class="field-hint">Sale en el pie: «Preparado por … con Resguardo».</span>
      </div>
      <div class="field">
        <span class="field-label">Logo</span>
        <div class="logo">
          {#if vista}<img src={vista} alt="Logo del prestador" />{:else}<span class="faint">Sin logo</span>{/if}
          <input bind:this={entrada} type="file" accept={TIPOS_LOGO} hidden onchange={elegir} />
          <button type="button" class="btn btn-sm" onclick={() => entrada?.click()}><ImagePlus size={14} />{vista ? "Cambiar" : "Elegir una imagen"}</button>
          {#if vista}<button type="button" class="btn btn-sm btn-ghost" onclick={() => ((nuevo = null), (quitar = true))}><Trash2 size={14} />Quitar</button>{/if}
        </div>
        <span class="field-hint">PNG, JPG, WebP o SVG de hasta 1 MB: se guarda como PNG (el SVG se convierte en este navegador, sin scripts).</span>
        {#if error}<span class="field-error">{error}</span>{/if}
      </div>
      <div>
        <BotonCargando class="btn btn-sm btn-primary" type="submit" cargando={guardando} disabled={!cambiado}><Check size={14} />Guardar</BotonCargando>
      </div>
    </form>
  </section>
{/if}

<style>
  .cab {
    display: flex;
    align-items: flex-start;
    gap: var(--sp-3);
  }
  .cab-texto {
    flex: 1;
    min-width: 0;
  }
  .cab h2 {
    margin: 0;
    font-size: var(--fs-h2);
  }
  .cab p {
    margin: 2px 0 0;
    font-size: var(--fs-sm);
  }
  .form {
    display: grid;
    gap: var(--sp-3);
    margin-top: var(--sp-4);
    max-width: 36rem;
  }
  .logo {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    flex-wrap: wrap;
  }
  .logo img {
    max-height: 36px;
    max-width: 160px;
    object-fit: contain;
    background: var(--surface-2);
    border-radius: 6px;
    padding: 4px;
  }
</style>
