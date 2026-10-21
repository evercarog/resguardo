<script lang="ts">
  // 14uu (plan 0.8.0, D3): la tarjeta «Equipo» de la ficha: sistema exacto,
  // procesador, memoria, discos (con barra de uso), fabricante y modelo, y el
  // último arranque. Con un agente anterior (sin `datos_basicos`), lo dice.
  //
  // Plan 0.8.1: su tipo (C3: físico, máquina virtual o contenedor, detectado y
  // corregible), dónde está físicamente (C4, si se anotó la infraestructura) y
  // el conjunto de discos de cada disco (grupo de ZFS, RAID…).
  //
  // Plan 0.8.2 (A3, 15ww): los discos en tres niveles (discos físicos con su
  // nombre de Infraestructura, conjuntos o discos virtuales con sus discos, y los
  // volúmenes con lo que guarda cada uno y, si se anotó, dónde está cada uno);
  // nunca «disco0» (el número interno de Windows). «Anotar» (A1) lleva a Ajustes
  // → Infraestructura con este equipo ya elegido.
  import { Cpu, HardDrive, Layers, MemoryStick, Monitor, Pencil, Power, Server, TriangleAlert, Wifi } from "@lucide/svelte";
  import { tip } from "$lib/tooltip";
  import { bytes, fechaLarga, relativo } from "$lib/formato";
  import { datosBasicos, textoMemoria, textoModelo, textoProcesador, textoSistema } from "$lib/datosBasicos";
  import type { Equipo } from "$lib/tipos";
  import { actual, puede } from "$lib/estado.svelte";
  import { admiteTipoEquipo, textoTipo, TEXTO_ORIGEN, tipoEquipo } from "$lib/tipoEquipo";
  import { discosEnNiveles, lineaFisica, sitioDeEquipo, type VolumenVista } from "$lib/infra";
  import TipoEquipo from "./TipoEquipo.svelte";

  let { equipo, ahora = Date.now() }: { equipo: Equipo; ahora?: number } = $props();
  const tipo = $derived(tipoEquipo(equipo));
  const fisico = $derived(lineaFisica(sitioDeEquipo(actual.infra, equipo.id), actual.organizacion));
  const puedeAnotar = $derived(!!actual.cliente && puede.administrar(actual.cliente.rol));
  const hrefAnotar = $derived(`/c/${actual.id}/ajustes/infraestructura?equipo=${encodeURIComponent(equipo.id)}`);
  const niveles = $derived(discosEnNiveles(equipo, actual.infra, actual.organizacion));
  const puedeCambiarTipo = $derived(!!actual.cliente && puede.administrar(actual.cliente.rol) && admiteTipoEquipo(equipo));
  let cambiandoTipo = $state(false);
  const d = $derived(datosBasicos(equipo));
  const sistema = $derived(textoSistema(d));
  const detalleSistema = $derived(
    d?.sistema ? [d.sistema.compilacion ? `compilación ${d.sistema.compilacion}` : null, d.sistema.kernel ? `núcleo ${d.sistema.kernel}` : null, d.sistema.variante && d.sistema.familia === "windows" ? d.sistema.variante : null].filter(Boolean).join(" · ") : "",
  );
  const procesador = $derived(textoProcesador(d));
  const modelo = $derived(textoModelo(d));
  const TONO = { ok: "var(--graf-marca)", warn: "var(--warn)", bad: "var(--bad)" } as const;
</script>

{#snippet volumen(v: VolumenVista)}
  <div class="disco-cab">
    <span class="mono punto">{v.punto}</span>
    {#if v.etiqueta}<span class="etiqueta-vol">«{v.etiqueta}»</span>{/if}
    {#if v.sistemaArchivos}<span class="pastilla mono">{v.sistemaArchivos}</span>{/if}
    {#each v.guarda as g (g)}<span class="faint en-uso" use:tip={"Lo que guarda este volumen"}>{g}</span>{/each}
    <span class="num libre" class:aviso={v.tono !== "ok"}>{bytes(v.libre)} libres de {bytes(v.total)}</span>
  </div>
  <div class="pista" role="meter" aria-valuemin="0" aria-valuemax="100" aria-valuenow={v.usado} aria-label="{v.punto}: {v.usado} % usado">
    <span style="width: {v.usado}%; background: {TONO[v.tono]}"></span>
  </div>
  {#if v.fisico}<p class="faint fisico-vol"><Server size={11} aria-hidden="true" />Físicamente en {v.fisico}</p>{/if}
  {#if v.tono === "bad"}<p class="casi-lleno"><TriangleAlert size={12} />Casi lleno: si se llena, las copias pueden fallar.</p>{/if}
{/snippet}

<section class="card p datos-equipo" aria-labelledby="t-datos-equipo">
  <h3 class="section-title" id="t-datos-equipo">Equipo</h3>
  {#if !d}
    <p class="faint">Actualiza el agente para ver estos datos: sistema, procesador, memoria, discos y último arranque.</p>
  {:else}
    <dl class="datos">
      <div>
        <dt><Cpu size={14} />Tipo</dt>
        <dd>
          <span use:tip={TEXTO_ORIGEN[tipo.origen]}>{textoTipo(tipo)}</span>
          {#if tipo.origen === "a_mano"}<span class="faint sub">Corregido a mano</span>{/if}
          {#if puedeCambiarTipo}<button type="button" class="btn btn-sm btn-ghost cambiar" onclick={() => (cambiandoTipo = true)}><Pencil size={13} />Cambiar</button>{/if}
        </dd>
      </div>
      {#if fisico || puedeAnotar}
        <div>
          <dt><Server size={14} />Dónde está físicamente</dt>
          <dd>
            {#if fisico}<a class="link" href={hrefAnotar}>{fisico}</a>{:else}<span class="faint">Sin anotar</span>{/if}
            {#if puedeAnotar}<a class="btn btn-sm btn-ghost cambiar" href={hrefAnotar}><Pencil size={13} />{fisico ? "Cambiar" : "Anotar"}</a>{/if}
            {#if niveles.porVolumen}<span class="faint sub">Algunos volúmenes están en otro sitio: abajo, en cada uno.</span>{/if}
          </dd>
        </div>
      {/if}
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
    {#if d.discos?.length || niveles.nodos.length}
      <h4 class="sub-titulo"><HardDrive size={14} />Discos</h4>
      {#if niveles.anfitrion}<p class="anfitrion"><Server size={13} aria-hidden="true" />Sus discos virtuales están en {niveles.anfitrion}</p>{/if}
      <ul class="nodos">
        {#each niveles.nodos as n (n.clave)}
          <li class="nodo">
            <div class="nodo-cab">
              <span class="nodo-ic" aria-hidden="true">{#if n.nivel === "conjunto"}<Layers size={14} />{:else}<HardDrive size={14} />{/if}</span>
              <span class="nodo-t">{n.titulo}</span>
              {#if n.detalle}<span class="faint nodo-d">{n.detalle}</span>{/if}
              {#if n.estado}<span class="badge tone-{n.tono}">{#if n.tono !== "ok"}<TriangleAlert size={12} />{/if}{n.estado}</span>{/if}
            </div>
            {#if n.miembros.length}
              <p class="faint miembros">Hecho de {n.miembros.map((m) => (m.detalle ? `${m.titulo} (${m.detalle})` : m.titulo)).join(" + ")}</p>
            {/if}
            {#if n.volumenes.length}
              <ul class="discos dentro">
                {#each n.volumenes as v (v.punto)}<li>{@render volumen(v)}</li>{/each}
              </ul>
            {/if}
          </li>
        {/each}
      </ul>
      {#if niveles.sueltos.length}
        <ul class="discos" class:dentro={niveles.nodos.length > 0}>
          {#each niveles.sueltos as v (v.punto)}<li>{@render volumen(v)}</li>{/each}
        </ul>
      {/if}
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

{#if cambiandoTipo && actual.cliente}<TipoEquipo cliente={actual.cliente} {equipo} onclose={() => (cambiandoTipo = false)} />{/if}

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
  .cambiar {
    margin-left: 6px;
    vertical-align: middle;
  }
  .anfitrion {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 0 0 var(--sp-2);
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  .nodos {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .nodo {
    padding: var(--sp-3);
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    min-width: 0;
  }
  .nodo-cab {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
    font-size: var(--fs-sm);
  }
  .nodo-ic {
    display: inline-flex;
    color: var(--text-3);
  }
  .nodo-t {
    font-weight: 500;
    overflow-wrap: anywhere;
  }
  .nodo-d {
    overflow-wrap: anywhere;
  }
  .miembros {
    margin: 4px 0 0;
    font-size: var(--fs-xs);
    overflow-wrap: anywhere;
  }
  .discos.dentro {
    margin-top: var(--sp-3);
    padding-left: var(--sp-3);
    border-left: 2px solid var(--border);
  }
  .etiqueta-vol {
    color: var(--text-2);
    overflow-wrap: anywhere;
  }
  .fisico-vol {
    display: flex;
    align-items: center;
    gap: 4px;
    margin: 4px 0 0;
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
