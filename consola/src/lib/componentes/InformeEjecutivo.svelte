<script lang="ts">
  // Informes → «Informe ejecutivo» (plan 0.8.0, G1): el PDF que prepara el
  // servidor (sin navegador) para descargarlo, enviarlo ahora o recibirlo cada
  // semana o cada mes por correo, con el logo del prestador o el del cliente.
  // La regla 3-2-1-1-0 la calcula esta consola (ve todos los equipos y el
  // catálogo de destinos): al abrir Informes se le manda al servidor (solo ids
  // y estados) para que el informe programado la lleve al día.
  import { untrack } from "svelte";
  import { CalendarClock, Check, Download, FileText, Info, Send } from "@lucide/svelte";
  import * as api from "$lib/api";
  import { actual, puede } from "$lib/estado.svelte";
  import { avisar, fallo } from "$lib/avisos.svelte";
  import { guardar as guardarArchivo } from "$lib/descarga";
  import { fechaLarga, relativo } from "$lib/formato";
  import { DIAS_SEMANA } from "$lib/notificaciones";
  import { reglaParaInforme } from "$lib/informeEjecutivo";
  import type { ReglaCopia } from "$lib/regla321";
  import type { InformeEjecutivo, LogoInforme, PeriodoInforme } from "$lib/tipos";
  import BotonCargando from "./BotonCargando.svelte";

  let { reglas }: { reglas: ReglaCopia[] } = $props();

  /** null: servidor anterior (sin informe ejecutivo); undefined: cargando. */
  let info = $state<InformeEjecutivo | null | undefined>(undefined);
  let cual = $state<"semanal:anterior" | "semanal:actual" | "mensual:anterior" | "mensual:actual">("semanal:anterior");
  let descargando = $state(false);
  let enviando = $state(false);
  let guardando = $state(false);

  // El formulario del programa.
  let activo = $state(false);
  let periodo = $state<PeriodoInforme>("semanal");
  let dia = $state(1);
  let destinatarios = $state("");
  let logo = $state<LogoInforme>("prestador");

  const rol = $derived(actual.cliente?.rol);
  const admin = $derived(puede.administrar(rol));

  function rellenar(i: InformeEjecutivo) {
    activo = i.programa.activo;
    periodo = i.programa.periodo;
    dia = i.programa.dia_semana;
    destinatarios = i.programa.destinatarios.join("\n");
    logo = i.programa.logo;
    cual = `${i.programa.periodo}:anterior`;
  }

  $effect(() => {
    const c = actual.id;
    if (!c) return;
    untrack(() => {
      info = undefined;
      api
        .informeEjecutivo(c)
        .then((i) => {
          if (c !== actual.id) return;
          info = i;
          rellenar(i);
        })
        .catch(() => (info = null));
    });
  });

  // La regla, al servidor: una vez por cliente y visita (si cambió), y solo quien puede mandar órdenes.
  // Con unos segundos de calma: mientras llegan los informes de los equipos y el catálogo, la regla cambia.
  let reglaMandada = "";
  $effect(() => {
    const c = actual.id;
    const copias = reglaParaInforme(reglas);
    const puedeMandar = puede.ordenar(rol) && !!info && copias.length > 0;
    const firma = `${c}|${JSON.stringify(copias)}`;
    if (!puedeMandar || firma === reglaMandada) return;
    const t = setTimeout(() => {
      reglaMandada = firma;
      void api
        .ponerReglaInforme(c, copias)
        .then(() => untrack(() => info && c === actual.id && (info.regla = { calculada: new Date().toISOString(), por: "", copias: copias.length })))
        .catch(() => (reglaMandada = ""));
    }, 4000);
    return () => clearTimeout(t);
  });

  const lista = $derived(
    destinatarios
      .split(/[\s,;]+/)
      .map((x) => x.trim())
      .filter(Boolean),
  );
  const cambiado = $derived(
    !!info &&
      (info.programa.activo !== activo ||
        info.programa.periodo !== periodo ||
        info.programa.dia_semana !== dia ||
        info.programa.logo !== logo ||
        info.programa.destinatarios.join("\n") !== lista.join("\n")),
  );
  const falta = $derived(activo && !lista.length ? "Añade al menos un destinatario." : "");
  const cuando = $derived(
    periodo === "semanal"
      ? `Sale los ${DIAS_SEMANA[dia - 1]} a las ${info?.hora ?? "08:00"} con la semana anterior entera (de lunes a domingo).`
      : `Sale el día 1 de cada mes a las ${info?.hora ?? "08:00"} con el mes anterior.`,
  );

  async function guardar(e: SubmitEvent) {
    e.preventDefault();
    if (falta) return;
    guardando = true;
    try {
      const p = await api.cambiarInformeEjecutivo(actual.id, { activo, periodo, dia_semana: dia, destinatarios: lista, logo });
      if (info) {
        info.programa = p;
        rellenar(info);
      }
      avisar(p.activo ? `Informe ${p.periodo} programado. ${cuando}` : "Informe programado apagado.");
    } catch (err) {
      fallo(err);
    } finally {
      guardando = false;
    }
  }

  async function descargar() {
    const [p, c] = cual.split(":") as [PeriodoInforme, "anterior" | "actual"];
    descargando = true;
    try {
      const r = await api.pdfInforme(actual.id, p, c);
      await guardarArchivo(r.datos, r.archivo);
    } catch (err) {
      fallo(err);
    } finally {
      descargando = false;
    }
  }

  async function enviar() {
    const [p, c] = cual.split(":") as [PeriodoInforme, "anterior" | "actual"];
    enviando = true;
    try {
      const r = await api.enviarInformeEjecutivo(actual.id, { periodo: p, cual: c });
      avisar(`Informe enviado a ${r.enviados === 1 ? "1 destinatario" : `${r.enviados} destinatarios`}. Llega en un momento (con el PDF adjunto).`);
      info = await api.informeEjecutivo(actual.id);
    } catch (err) {
      fallo(err);
    } finally {
      enviando = false;
    }
  }

  const porQueNoEnviar = $derived(
    !info ? "" : !info.correo ? "Falta un canal de correo encendido." : !info.programa.destinatarios.length ? "Guarda antes los destinatarios." : "",
  );
</script>

{#if info}
  <section class="card p no-imprimir ejecutivo" aria-labelledby="t-ejecutivo">
    <div class="cab">
      <span class="card-icon"><FileText size={18} /></span>
      <div class="cab-texto">
        <h2 id="t-ejecutivo">Informe ejecutivo</h2>
        <p class="faint">
          Un PDF con lo importante del periodo para enseñar o mandar: copias correctas, lo que falló, la regla 3-2-1-1-0, verificaciones, pruebas y restauraciones,
          espacio y avisos. Sin rutas, nombres de archivos ni contraseñas.
        </p>
      </div>
    </div>

    <div class="ahora">
      <label class="field-label" for="ie-cual">Periodo</label>
      <div class="fila">
        <select id="ie-cual" class="input" bind:value={cual}>
          <option value="semanal:anterior">La semana pasada</option>
          <option value="semanal:actual">Esta semana, hasta hoy</option>
          <option value="mensual:anterior">El mes pasado</option>
          <option value="mensual:actual">Este mes, hasta hoy</option>
        </select>
        <BotonCargando class="btn" cargando={descargando} textoCargando="Preparando…" onclick={descargar}><Download size={15} />Descargar el informe</BotonCargando>
        {#if admin}
          <BotonCargando class="btn" cargando={enviando} textoCargando="Enviando…" disabled={!!porQueNoEnviar} title={porQueNoEnviar || undefined} onclick={enviar}>
            <Send size={15} />Enviar ahora
          </BotonCargando>
        {/if}
      </div>
      {#if admin && porQueNoEnviar}<p class="faint pista">«Enviar ahora»: {porQueNoEnviar}</p>{/if}
    </div>

    <h3 class="sub"><CalendarClock size={15} />Programado por correo</h3>
    {#if !info.correo}
      <div class="notice notice-info">
        <Info size={16} />
        <p>No hay un canal de correo encendido: lo pone el propietario del servidor (Servidor → Notificaciones) o el de este cliente (Personas y ajustes).</p>
      </div>
    {/if}
    <form class="programa" onsubmit={guardar}>
      <label class="switch-row ancho">
        <input class="switch" type="checkbox" bind:checked={activo} disabled={!admin} />
        <span>Mandar el informe por correo cada {periodo === "semanal" ? "semana" : "mes"}</span>
      </label>
      <div class="field">
        <span class="field-label" id="ie-periodo">Cada</span>
        <div class="segmented inline" role="group" aria-labelledby="ie-periodo">
          <button type="button" class:on={periodo === "semanal"} aria-pressed={periodo === "semanal"} disabled={!admin} onclick={() => (periodo = "semanal")}>Semana</button>
          <button type="button" class:on={periodo === "mensual"} aria-pressed={periodo === "mensual"} disabled={!admin} onclick={() => (periodo = "mensual")}>Mes</button>
        </div>
      </div>
      {#if periodo === "semanal"}
        <div class="field">
          <label class="field-label" for="ie-dia">Los</label>
          <select id="ie-dia" class="input" bind:value={dia} disabled={!admin}>
            {#each DIAS_SEMANA as d, i (d)}<option value={i + 1}>{d}</option>{/each}
          </select>
        </div>
      {/if}
      <div class="field">
        <label class="field-label" for="ie-logo">Logo</label>
        <select id="ie-logo" class="input" bind:value={logo} disabled={!admin}>
          <option value="prestador">{info.prestador.nombre ? `El de ${info.prestador.nombre}` : "El del prestador"}{info.prestador.logo ? "" : " (aún no hay)"}</option>
          <option value="cliente">El de {actual.cliente?.nombre ?? "este cliente"}{info.logo_cliente ? "" : " (aún no hay)"}</option>
          <option value="ninguno">Ninguno</option>
        </select>
      </div>
      <div class="field ancho">
        <label class="field-label" for="ie-para">Destinatarios</label>
        <textarea id="ie-para" class="input" rows="3" bind:value={destinatarios} placeholder="direccion@ejemplo.com" disabled={!admin} spellcheck="false"></textarea>
        <span class="field-hint">Una dirección por línea (hasta 20). No hace falta que tengan cuenta en la consola. {cuando}</span>
      </div>
      {#if admin}
        <div class="ancho guardar">
          <BotonCargando class="btn btn-sm btn-primary" type="submit" cargando={guardando} disabled={!cambiado || !!falta}><Check size={14} />Guardar</BotonCargando>
          {#if falta}<span class="faint pista">{falta}</span>{/if}
        </div>
      {/if}
    </form>

    <p class="faint pie">
      {#if info.ultimo}
        Último envío: {fechaLarga(info.ultimo.cuando)} ({relativo(info.ultimo.cuando)}), a {info.ultimo.destinatarios === 1 ? "1 destinatario" : `${info.ultimo.destinatarios} destinatarios`}{info.ultimo.a_mano ? `, a mano${info.ultimo.por ? ` por ${info.ultimo.por}` : ""}` : ""}.
      {:else}
        Aún no se ha enviado ninguno.
      {/if}
      {#if info.regla}
        La regla 3-2-1-1-0 del informe se calculó {relativo(info.regla.calculada)} (se pone al día al abrir Informes).
      {:else}
        La regla 3-2-1-1-0 del informe se calcula al abrir Informes.
      {/if}
    </p>
  </section>
{/if}

<style>
  .ejecutivo {
    margin-bottom: var(--sp-4);
  }
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
  .ahora {
    margin-top: var(--sp-4);
  }
  .fila {
    display: flex;
    flex-wrap: wrap;
    gap: var(--sp-2);
    align-items: center;
  }
  .fila select {
    width: auto;
    min-width: 13rem;
  }
  .sub {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    margin: var(--sp-5) 0 var(--sp-2);
    font-size: var(--fs-sm);
    font-weight: 600;
  }
  .programa {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: var(--sp-3);
    align-items: start;
  }
  .ancho {
    grid-column: 1 / -1;
  }
  .guardar {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
  }
  .pista {
    margin: var(--sp-1) 0 0;
    font-size: var(--fs-xs);
  }
  .pie {
    margin: var(--sp-4) 0 0;
    font-size: var(--fs-xs);
  }
  .notice {
    margin-bottom: var(--sp-3);
  }
  @media (max-width: 640px) {
    .programa {
      grid-template-columns: 1fr;
    }
  }
</style>
