<script lang="ts">
  // «Ahora»: lo que está en marcha, con las ondas en vivo (lectura, subida o
  // escritura, y archivos por segundo); si no hay nada, el estado tranquilo.
  import GraficaOndas from "$ui/componentes/GraficaOndas.svelte";
  import { porSegundoArchivos } from "$ui/ritmos";
  import { CircleAlert, CircleCheck, CloudUpload, DatabaseBackup, HardDriveDownload, LoaderCircle, ShieldCheck, Sparkles } from "@lucide/svelte";
  import { bytes, cuandoFrase, numero, relativo } from "$lib/formato";
  import { pedir, vivo } from "./puente.svelte";
  import { quedan, seriesDe, textoFase, tono, TIPOS } from "./estado";
  import type { Actividad } from "./tipos";

  let { irAjustes, sinConfigurar }: { irAjustes: () => void; sinConfigurar: boolean } = $props();

  const b = $derived(vivo.datos?.bandeja ?? null);
  const acts = $derived(b?.actividades ?? []);
  const series = $derived(seriesDe(vivo.datos?.ventana?.serie));
  const t = $derived(tono(b, vivo.datos?.ahora));
  const ultima = $derived(
    (b?.copias ?? [])
      .filter((c) => c.resultado === "ok" || c.resultado === "warning")
      .map((c) => c.cuando ?? "")
      .filter(Boolean)
      .sort()
      .at(-1),
  );
  const proxima = $derived(
    (b?.copias ?? [])
      .map((c) => c.proxima ?? "")
      .filter(Boolean)
      .sort()[0],
  );
  const pedibles = $derived((b?.copias ?? []).filter((c) => !c.pausada));
  let mensaje = $state("");
  let error = $state("");

  async function copiarTodas() {
    mensaje = error = "";
    try {
      const r = await pedir<{ mensaje: string }>("copiar", { claves: pedibles.map((c) => c.clave) });
      mensaje = r.mensaje;
    } catch (e) {
      error = (e as Error).message;
    }
  }

  const icono = (a: Actividad) =>
    a.tipo === "restauracion" ? HardDriveDownload : a.tipo === "verificacion" ? ShieldCheck : a.tipo === "copia" ? DatabaseBackup : CloudUpload;
  const pct = (a: Actividad) => (a.porcentaje == null ? null : Math.min(100, Math.floor(a.porcentaje * 100)));
  function cifras(a: Actividad): string[] {
    return [
      a.archivos_total ? `${numero(a.archivos ?? 0)} de ${numero(a.archivos_total)} archivos` : null,
      a.bytes_total ? `${bytes(a.bytes ?? 0)} de ${bytes(a.bytes_total)}` : a.bytes ? bytes(a.bytes) : null,
      quedan(a.quedan_s),
    ].filter((x): x is string => !!x);
  }
</script>

<div class="v-pila">
  {#if sinConfigurar}
    <section class="v-tarjeta bienvenida">
      <Sparkles size={22} aria-hidden="true" />
      <h2 class="v-titulo">Resguardo está listo en este equipo</h2>
      <p class="v-sub">
        Aún no tiene copias. Puedes usarlo solo, en este equipo, con una clave de administración, o vincularlo más tarde a una consola sin perder nada.
      </p>
      <button class="btn btn-primary" onclick={irAjustes}>Usar sin consola</button>
    </section>
  {/if}

  {#each acts as a (a.id)}
    {@const Icono = icono(a)}
    {@const p = pct(a)}
    <section class="v-tarjeta en-marcha" aria-label={`${TIPOS[a.tipo]}: ${a.nombre}`}>
      <div class="v-fila">
        <span class="ico" data-tipo={a.tipo}><Icono size={18} aria-hidden="true" /></span>
        <div class="v-cortar">
          <p class="v-mini">{TIPOS[a.tipo]}</p>
          <h2 class="v-titulo v-cortar">{a.nombre}</h2>
        </div>
        {#if p != null}<strong class="pct num">{p} %</strong>{/if}
      </div>
      <div class="barra" class:indeterminada={p == null} role="progressbar" aria-label={`Progreso de «${a.nombre}»`} aria-valuemin={0} aria-valuemax={100} aria-valuenow={p ?? undefined}>
        <span style:width={p == null ? undefined : `${p}%`}></span>
      </div>
      <p class="v-sub"><LoaderCircle size={13} class="spin" aria-hidden="true" /> {textoFase(a)}</p>
      {#if cifras(a).length}<p class="v-mini num">{cifras(a).join(" · ")}</p>{/if}
    </section>
  {/each}

  {#if series.bytes.length}
    <section class="v-tarjeta ondas" aria-label="Ritmo en vivo">
      <GraficaOndas series={series.bytes} titulo="Ritmo de los últimos minutos" alto={150} retraso={3000} animar={acts.length > 0} />
      {#if series.archivos.length}
        <GraficaOndas series={series.archivos} titulo="Archivos por segundo" alto={44} retraso={3000} formato={porSegundoArchivos} animar={acts.length > 0} />
      {/if}
    </section>
  {/if}

  {#if !acts.length && b?.vinculado}
    <section class="v-tarjeta calma" data-tono={t}>
      {#if t === "bad" || t === "warn"}<CircleAlert size={28} aria-hidden="true" />{:else}<CircleCheck size={28} aria-hidden="true" />{/if}
      <h2 class="v-titulo">{b.text}</h2>
      <p class="v-sub">
        {ultima ? `Última copia ${relativo(ultima)}` : "Aún no hay copias terminadas"}{proxima ? ` · la próxima, ${cuandoFrase(proxima)}` : ""}.
      </p>
      {#if b.pedir && pedibles.length}
        <button class="btn btn-primary" onclick={copiarTodas}>Copiar ahora</button>
      {/if}
      {#if mensaje}<p class="v-ok" role="status">{mensaje}</p>{/if}
      {#if error}<p class="v-error" role="alert">{error}</p>{/if}
    </section>
  {/if}

  {#if b?.privacy}<p class="v-mini privacidad">{b.privacy}</p>{/if}
</div>

<style>
  .bienvenida,
  .calma {
    display: grid;
    justify-items: start;
    gap: var(--sp-2);
  }
  .bienvenida :global(svg) {
    color: var(--accent);
  }
  .calma :global(svg) {
    color: var(--ok);
  }
  .calma[data-tono="bad"] :global(svg) {
    color: var(--bad);
  }
  .calma[data-tono="warn"] :global(svg) {
    color: var(--warn);
  }
  .calma .btn,
  .bienvenida .btn {
    margin-top: var(--sp-2);
  }
  .en-marcha {
    display: grid;
    gap: var(--sp-2);
  }
  .ico {
    display: grid;
    place-items: center;
    flex: none;
    width: 34px;
    height: 34px;
    border-radius: var(--radius);
    color: var(--onda-2);
    background: color-mix(in srgb, var(--onda-2) 14%, transparent);
  }
  .ico[data-tipo="verificacion"] {
    color: var(--onda-1);
    background: color-mix(in srgb, var(--onda-1) 14%, transparent);
  }
  .pct {
    margin-left: auto;
    font-size: var(--fs-stat);
    font-weight: 650;
    letter-spacing: -0.02em;
  }
  .barra {
    position: relative;
    height: 6px;
    overflow: hidden;
    background: var(--surface-3);
    border-radius: 999px;
  }
  .barra span {
    position: absolute;
    inset: 0 auto 0 0;
    background: linear-gradient(90deg, var(--onda-1), var(--onda-2));
    border-radius: inherit;
    transition: width 0.9s linear;
  }
  .barra.indeterminada span {
    width: 35%;
    animation: vaiven 1.6s ease-in-out infinite;
  }
  @keyframes vaiven {
    from {
      left: -35%;
    }
    to {
      left: 100%;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .barra span {
      transition: none;
    }
    .barra.indeterminada span {
      left: 0;
      width: 100%;
      opacity: 0.35;
      animation: none;
    }
  }
  .v-sub :global(svg) {
    vertical-align: -2px;
  }
  .ondas {
    display: grid;
    gap: var(--sp-3);
    /* Un poco de «escenario» para las ondas, en claro y en oscuro. */
    background: radial-gradient(120% 90% at 50% 120%, color-mix(in srgb, var(--onda-1) 10%, transparent), transparent 70%), var(--surface);
  }
  .privacidad {
    margin: 0;
  }
  .num {
    font-variant-numeric: tabular-nums;
  }
</style>
