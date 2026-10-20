<script lang="ts">
  // 14uu (plan 0.8.0, D3): la tarjeta «Equipo» de la ficha: sistema exacto,
  // procesador, memoria, discos (con barra de uso), fabricante y modelo, y el
  // último arranque. Con un agente anterior (sin `datos_basicos`), lo dice.
  import { Cpu, HardDrive, MemoryStick, Monitor, Power, Server, TriangleAlert, Wifi } from "@lucide/svelte";
  import { tip } from "$lib/tooltip";
  import { bytes, fechaLarga, relativo } from "$lib/formato";
  import { datosBasicos, textoMemoria, textoModelo, textoProcesador, textoSistema, usoDisco } from "$lib/datosBasicos";
  import type { Equipo } from "$lib/tipos";

  let { equipo, ahora = Date.now() }: { equipo: Equipo; ahora?: number } = $props();
  const d = $derived(datosBasicos(equipo));
  const sistema = $derived(textoSistema(d));
  const detalleSistema = $derived(
    d?.sistema ? [d.sistema.compilacion ? `compilación ${d.sistema.compilacion}` : null, d.sistema.kernel ? `núcleo ${d.sistema.kernel}` : null, d.sistema.variante && d.sistema.familia === "windows" ? d.sistema.variante : null].filter(Boolean).join(" · ") : "",
  );
  const procesador = $derived(textoProcesador(d));
  const modelo = $derived(textoModelo(d));
  const TONO = { ok: "var(--graf-marca)", warn: "var(--warn)", bad: "var(--bad)" } as const;
</script>

<section class="card p datos-equipo" aria-labelledby="t-datos-equipo">
  <h3 class="section-title" id="t-datos-equipo">Equipo</h3>
  {#if !d}
    <p class="faint">Actualiza el agente para ver estos datos: sistema, procesador, memoria, discos y último arranque.</p>
  {:else}
    <dl class="datos">
      <div>
        <dt><Monitor size={14} />Sistema</dt>
        <dd>{sistema ?? equipo.so}{#if detalleSistema}<span class="faint sub">{detalleSistema}</span>{/if}</dd>
      </div>
      {#if procesador}
        <div><dt><Cpu size={14} />Procesador</dt><dd>{procesador}</dd></div>
      {/if}
      {#if d.memoria}
        <div><dt><MemoryStick size={14} />Memoria</dt><dd class="num">{textoMemoria(d)}</dd></div>
      {/if}
      {#if modelo}
        <div><dt><Server size={14} />Fabricante y modelo</dt><dd>{modelo}</dd></div>
      {/if}
      {#if d.arranque}
        <div>
          <dt><Power size={14} />Último arranque</dt>
          <dd><time datetime={d.arranque} use:tip={fechaLarga(d.arranque)}>{relativo(d.arranque, ahora)}</time></dd>
        </div>
      {/if}
    </dl>
    {#if d.discos?.length}
      <h4 class="sub-titulo"><HardDrive size={14} />Discos</h4>
      <ul class="discos">
        {#each d.discos as k (k.punto)}
          {@const u = usoDisco(k)}
          <li>
            <div class="disco-cab">
              <span class="mono punto">{k.punto}</span>
              {#if k.sistema_archivos}<span class="pastilla mono">{k.sistema_archivos}</span>{/if}
              {#if k.en_uso}<span class="faint en-uso" use:tip={"Hay carpetas que se copian, un destino local o el almacén en este disco"}>con copias</span>{/if}
              <span class="num libre" class:aviso={u.tono !== "ok"}>{bytes(k.libre)} libres de {bytes(k.total)}</span>
            </div>
            <div class="pista" role="meter" aria-valuemin="0" aria-valuemax="100" aria-valuenow={u.usado} aria-label="{k.punto}: {u.usado} % usado">
              <span style="width: {u.usado}%; background: {TONO[u.tono]}"></span>
            </div>
            {#if u.tono === "bad"}<p class="casi-lleno"><TriangleAlert size={12} />Casi lleno: si se llena, las copias pueden fallar.</p>{/if}
          </li>
        {/each}
      </ul>
    {/if}
    {#if d.red_ahorro?.length}
      <!-- 15ll (2): tarjetas de red que Windows puede apagar para ahorrar energía. -->
      <div class="notice notice-info red-ahorro" role="note">
        <Wifi size={16} />
        <p>
          Windows puede apagar {d.red_ahorro.length === 1 ? `la tarjeta de red «${d.red_ahorro[0]}»` : `${d.red_ahorro.length} tarjetas de red (${d.red_ahorro.map((n) => `«${n}»`).join(", ")})`} para ahorrar energía. Mientras copia, el equipo no se suspende y no pasa nada; si se suspende igual, la copia se corta. Para quitarlo: Administrador de dispositivos → la tarjeta → Propiedades → Administración de energía → quita «Permitir que el equipo apague este dispositivo para ahorrar energía».
        </p>
      </div>
    {/if}
    {#if d.leido}<p class="faint leido">Leído {relativo(d.leido, ahora)}. El equipo los vuelve a leer una vez al día.</p>{/if}
  {/if}
</section>

<style>
  .datos {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
    gap: var(--sp-3) var(--sp-6);
    margin: 0;
  }
  .datos > div {
    min-width: 0;
  }
  .red-ahorro {
    margin-top: var(--sp-4);
  }
  .red-ahorro p {
    margin: 0;
    font-size: var(--fs-sm);
  }
  dt {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: var(--fs-xs);
    font-weight: 500;
    color: var(--text-3);
  }
  dd {
    margin: 2px 0 0;
    font-size: var(--fs-sm);
    overflow-wrap: anywhere;
  }
  .sub {
    display: block;
    font-size: var(--fs-xs);
  }
  .sub-titulo {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: var(--sp-5) 0 var(--sp-2);
    font-size: var(--fs-xs);
    font-weight: 500;
    color: var(--text-3);
  }
  .discos {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .disco-cab {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
    font-size: var(--fs-sm);
  }
  .punto {
    font-weight: 500;
    overflow-wrap: anywhere;
  }
  .en-uso {
    font-size: var(--fs-xs);
  }
  .libre {
    margin-left: auto;
    color: var(--text-2);
  }
  .libre.aviso {
    color: var(--text-1);
    font-weight: 500;
  }
  .pista {
    height: 6px;
    margin-top: 6px;
    overflow: hidden;
    background: var(--surface-3);
    border-radius: 999px;
  }
  .pista > span {
    display: block;
    height: 100%;
    border-radius: inherit;
  }
  .casi-lleno {
    display: flex;
    align-items: center;
    gap: 4px;
    margin: 4px 0 0;
    font-size: var(--fs-xs);
    color: var(--bad);
  }
  .leido {
    margin: var(--sp-4) 0 0;
    font-size: var(--fs-xs);
  }
</style>
