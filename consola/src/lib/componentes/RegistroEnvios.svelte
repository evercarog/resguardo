<script lang="ts">
  // Registro de envíos de las notificaciones (los últimos 100): qué se mandó,
  // a quién, cómo fue y, si falló o espera, por qué (sin secretos ni direcciones).
  import { History, RefreshCw } from "@lucide/svelte";
  import * as api from "$lib/api";
  import type { AmbitoNotif } from "$lib/api";
  import { estadoEnvio, SEVERIDAD, TIPO_CANAL, TIPO_ENVIO } from "$lib/notificaciones";
  import type { EnvioNotif } from "$lib/tipos";
  import { fechaLarga } from "$lib/formato";
  import { app } from "$lib/estado.svelte";
  import Chip from "$lib/componentes/Chip.svelte";
  import Tiempo from "$lib/componentes/Tiempo.svelte";
  import Vacio from "$lib/componentes/Vacio.svelte";

  let { ambito }: { ambito: AmbitoNotif } = $props();

  let envios = $state<EnvioNotif[] | null>(null);
  let error = $state("");
  let cargando = $state(false);
  let todos = $state(false);

  async function cargar() {
    cargando = true;
    try {
      envios = await api.registroNotif(ambito);
      error = "";
    } catch (e) {
      error = (e as Error).message;
    } finally {
      cargando = false;
    }
  }
  $effect(() => {
    void ambito;
    void cargar();
  });

  const visibles = $derived(todos ? (envios ?? []) : (envios ?? []).slice(0, 15));
  const nombreCliente = (id: string | null) => (id ? (app.clientes.find((c) => c.id === id)?.nombre ?? null) : null);
</script>

<div class="registro">
  <div class="cab">
    <span class="faint">Lo último que se mandó o se intentó mandar (hasta 100).</span>
    <button class="btn btn-sm btn-ghost" onclick={cargar} disabled={cargando} aria-label="Actualizar el registro"><RefreshCw size={14} class={cargando ? "spin" : ""} />Actualizar</button>
  </div>
  {#if error}
    <p class="faint">{error}</p>
  {:else if envios && !envios.length}
    <div class="card"><Vacio icono={History} titulo="Aún no se ha mandado nada" texto="Cuando haya un aviso (o pulses «Enviar prueba»), saldrá aquí." /></div>
  {:else if envios}
    <div class="card p-0 lista">
      {#each visibles as e (e.id)}
        {@const st = estadoEnvio(e)}
        <div class="fila envio">
          <span class="fila-texto">
            <span class="fila-titulo">
              {e.titulo}
              {#if e.tipo === "aviso"}<Chip pequeno tono={SEVERIDAD[e.severidad].tono} texto={SEVERIDAD[e.severidad].texto} />{:else}<span class="tipo">{TIPO_ENVIO[e.tipo]}</span>{/if}
            </span>
            <span class="fila-sub">
              {e.canal ? `${e.canal.nombre} (${TIPO_CANAL[e.canal.tipo].texto})` : "Canal quitado"}{#if e.destino} → {e.destino}{/if}
              {#if nombreCliente(e.cliente) && ambito && !("cliente" in ambito)} · {nombreCliente(e.cliente)}{/if}
            </span>
            {#if e.error || e.nota}
              <span class="detalle" class:mal={e.estado === "fallido"}>{[e.error, e.nota].filter(Boolean).join(" · ")}</span>
            {/if}
          </span>
          <span class="derecha">
            <Chip pequeno tono={st.tono} texto={st.texto} />
            <span class="cuando" title={fechaLarga(e.enviado ?? e.creado)}><Tiempo iso={e.enviado ?? e.creado} /></span>
          </span>
        </div>
      {/each}
    </div>
    {#if envios.length > 15 && !todos}
      <button class="btn btn-sm btn-ghost mas" onclick={() => (todos = true)}>Ver los {envios.length}</button>
    {/if}
  {:else}
    <p class="faint">Cargando…</p>
  {/if}
</div>

<style>
  .registro {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
  }
  .cab {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--sp-3);
    font-size: var(--fs-sm);
  }
  .fila-titulo {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
  }
  .tipo {
    font-size: var(--fs-xs);
    color: var(--text-3);
  }
  .detalle {
    font-size: var(--fs-xs);
    color: var(--text-3);
  }
  .detalle.mal {
    color: var(--bad);
  }
  .derecha {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: 2px;
    flex: none;
  }
  .cuando {
    font-size: var(--fs-xs);
    color: var(--text-3);
  }
  .mas {
    align-self: flex-start;
  }
  @media (max-width: 560px) {
    .envio {
      flex-wrap: wrap;
    }
    .derecha {
      flex-direction: row;
      align-items: center;
    }
  }
</style>
