<script lang="ts">
  // «Mis notificaciones» (Ajustes de la cuenta; api-servidor.md §13): horas de
  // silencio, resúmenes y, por cliente, qué avisos me llegan por correo.
  import { onMount } from "svelte";
  import { BellRing, Info, MoonStar } from "@lucide/svelte";
  import * as api from "$lib/api";
  import { avisar, fallo } from "$lib/avisos.svelte";
  import { app, NOMBRE_ROL } from "$lib/estado.svelte";
  import { DIAS_SEMANA, SEVERIDAD, SEVERIDADES } from "$lib/notificaciones";
  import type { MisNotif, PrefEtiqueta, Severidad } from "$lib/tipos";
  import Ayuda from "$lib/componentes/Ayuda.svelte";
  import Modal from "$ui/componentes/Modal.svelte";
  import AvisosPorEtiqueta from "./AvisosPorEtiqueta.svelte";
  import BotonCargando from "./BotonCargando.svelte";

  /** null: servidor anterior (sin notificaciones); undefined: cargando. */
  let m = $state<MisNotif | null | undefined>(undefined);
  let desde = $state("22:00");
  let hasta = $state("07:00");
  let salvo = $state(true);

  onMount(async () => {
    try {
      m = await api.misNotif();
      if (m?.silencio) {
        desde = m.silencio.desde;
        hasta = m.silencio.hasta;
        salvo = m.silencio.salvo_criticos;
      }
    } catch {
      m = null;
    }
  });

  async function cambiar(b: Parameters<typeof api.cambiarMisNotif>[0], texto: string) {
    try {
      m = await api.cambiarMisNotif(b);
      avisar(texto);
    } catch (e) {
      fallo(e);
    }
  }
  const silencio = (activo: boolean) =>
    cambiar({ silencio: activo ? { desde, hasta, salvo_criticos: salvo } : null }, activo ? `Horas de silencio: de ${desde} a ${hasta}.` : "Sin horas de silencio.");

  async function marcar(c: MisNotif["clientes"][number], sev: Severidad, si: boolean) {
    const inmediatos = si ? [...c.preferencias.inmediatos, sev] : c.preferencias.inmediatos.filter((x) => x !== sev);
    await prefs(c, inmediatos, c.preferencias.resumen);
  }
  // v1.4x: por etiqueta de los equipos de un cliente.
  let porEtiqueta = $state<{ c: MisNotif["clientes"][number]; valor: PrefEtiqueta[] } | null>(null);
  let guardandoEt = $state(false);
  async function guardarPorEtiqueta(e: SubmitEvent) {
    e.preventDefault();
    if (!porEtiqueta) return;
    guardandoEt = true;
    const { c, valor } = porEtiqueta;
    await prefs(c, c.preferencias.inmediatos, c.preferencias.resumen, valor);
    guardandoEt = false;
    porEtiqueta = null;
  }
  async function prefs(c: MisNotif["clientes"][number], inmediatos: Severidad[], resumen: boolean, etiquetas?: PrefEtiqueta[]) {
    try {
      const p = await api.ponerPrefsNotif(c.id, app.cuenta!.id, { inmediatos, resumen, ...(etiquetas ? { etiquetas } : {}) });
      if (m) m.clientes = m.clientes.map((x) => (x.id === c.id ? { ...x, preferencias: p } : x));
      avisar(`Guardado para ${c.nombre}.`);
    } catch (e) {
      fallo(e);
    }
  }
</script>

{#if porEtiqueta}
  <Modal labelledby="t-mis-et" onclose={() => (porEtiqueta = null)} width={480}>
    <form class="form" onsubmit={guardarPorEtiqueta}>
      <div class="dlg-title">
        <span class="ticon"><BellRing size={18} /></span>
        <div>
          <h2 id="t-mis-et">Avisos por etiqueta · {porEtiqueta.c.nombre}</h2>
          <p>Lo que te llega al momento de los equipos con cada etiqueta, en lugar de lo general de este cliente.</p>
        </div>
      </div>
      <AvisosPorEtiqueta id="mis-et" etiquetas={[...new Set([...(porEtiqueta.c.etiquetas ?? []), ...porEtiqueta.valor.map((x) => x.etiqueta)])]} bind:valor={porEtiqueta.valor} />
      <footer>
        <button type="button" class="btn btn-ghost" onclick={() => (porEtiqueta = null)}>Cancelar</button>
        <BotonCargando class="btn btn-primary" type="submit" cargando={guardandoEt}>Guardar</BotonCargando>
      </footer>
    </form>
  </Modal>
{/if}

{#if m}
  <section class="card p" id="mis-notificaciones">
    <h2 class="section-title"><BellRing size={16} /> Mis notificaciones <Ayuda id="notificaciones" /></h2>
    <p class="faint intro">Lo que te llega por correo. Nunca llevan contraseñas, claves ni nombres de archivos.</p>

    <div class="bloque">
      <label class="switch-row">
        <input class="switch" type="checkbox" checked={!!m.silencio} onchange={(e) => silencio(e.currentTarget.checked)} />
        <span><strong><MoonStar size={13} /> Horas de silencio</strong><span class="faint">Lo que llegue dentro te espera hasta el final, en un solo correo (hora del servidor).</span></span>
      </label>
      <div class="horas" class:apagado={!m.silencio}>
        <label>De <input class="input" type="time" bind:value={desde} onchange={() => m?.silencio && silencio(true)} /></label>
        <label>a <input class="input" type="time" bind:value={hasta} onchange={() => m?.silencio && silencio(true)} /></label>
        <label class="check"><input type="checkbox" bind:checked={salvo} onchange={() => m?.silencio && silencio(true)} /><span>Los críticos, al momento</span></label>
      </div>
    </div>

    <div class="bloque">
      <label class="switch-row">
        <input class="switch" type="checkbox" checked={m.resumen_semanal} onchange={(e) => cambiar({ resumen_semanal: e.currentTarget.checked }, e.currentTarget.checked ? "Recibirás el resumen semanal." : "Sin resumen semanal.")} />
        <span><strong>Resumen semanal de copias</strong><span class="faint">Los {DIAS_SEMANA[m.dia_semanal - 1]} a las {m.hora_resumen}: cómo está cada equipo, la última copia correcta, los fallos y lo que necesita atención.</span></span>
      </label>
      <label class="switch-row">
        <input class="switch" type="checkbox" checked={m.resumen_diario} onchange={(e) => cambiar({ resumen_diario: e.currentTarget.checked }, e.currentTarget.checked ? "Recibirás el resumen diario." : "Sin resumen diario.")} />
        <span><strong>Resumen diario</strong><span class="faint">Cada día a las {m.hora_resumen}.</span></span>
      </label>
    </div>

    {#if m.clientes.length}
      <div class="tabla" role="table" aria-label="Avisos por cliente">
        <div class="tr cab" role="row">
          <span role="columnheader">Cliente</span>
          {#each SEVERIDADES as s (s)}<span role="columnheader" class="c">{SEVERIDAD[s].texto}</span>{/each}
          <span role="columnheader" class="c">Resumen</span>
        </div>
        {#each m.clientes as c (c.id)}
          <div class="tr" role="row">
            <span role="cell" class="cli"
              >{c.nombre}<span class="faint">{NOMBRE_ROL[c.rol]}{c.correo ? "" : " · sin correo"}</span>
              {#if c.etiquetas?.length}<button type="button" class="link por-et" onclick={() => (porEtiqueta = { c, valor: [...(c.preferencias.etiquetas ?? [])] })}
                  >Por etiqueta{c.preferencias.etiquetas?.length ? ` (${c.preferencias.etiquetas.length})` : "…"}</button
                >{/if}</span
            >
            {#each SEVERIDADES as s (s)}
              <span role="cell" class="c"><input type="checkbox" aria-label="{SEVERIDAD[s].texto} de {c.nombre}" checked={c.preferencias.inmediatos.includes(s)} onchange={(e) => marcar(c, s, e.currentTarget.checked)} /></span>
            {/each}
            <span role="cell" class="c"><input type="checkbox" aria-label="Resumen de {c.nombre}" checked={c.preferencias.resumen} onchange={(e) => prefs(c, c.preferencias.inmediatos, e.currentTarget.checked)} /></span>
          </div>
        {/each}
      </div>
      {#if m.clientes.some((c) => !c.correo)}
        <div class="notice notice-info"><Info size={16} /><p>Donde dice «sin correo», aún no hay un canal de correo: lo pone el propietario del servidor (o el del cliente).</p></div>
      {/if}
    {/if}
  </section>
{/if}

<style>
  .por-et {
    align-self: flex-start;
    font-size: var(--fs-xs);
  }
  .intro {
    margin: 4px 0 var(--sp-3);
    font-size: var(--fs-sm);
  }
  .bloque {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
    padding: var(--sp-3) 0;
    border-top: 1px solid var(--border);
  }
  .switch-row strong {
    display: inline-flex;
    align-items: center;
    gap: 4px;
  }
  .horas {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--sp-3);
    padding-left: 40px;
    font-size: var(--fs-sm);
  }
  .horas.apagado {
    opacity: 0.6;
  }
  .horas > label:not(.check) {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
  .horas .input {
    width: 120px;
  }
  .check {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
  .tabla {
    display: flex;
    flex-direction: column;
    border-top: 1px solid var(--border);
    font-size: var(--fs-sm);
  }
  .tr {
    display: grid;
    grid-template-columns: minmax(0, 1fr) repeat(4, 84px);
    align-items: center;
    gap: var(--sp-2);
    padding: 8px 0;
    border-bottom: 1px solid var(--border);
  }
  .tr.cab {
    font-size: var(--fs-xs);
    font-weight: 600;
    color: var(--text-3);
  }
  .c {
    text-align: center;
  }
  .cli {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .cli .faint {
    font-size: var(--fs-xs);
  }
  .card > .notice {
    margin-top: var(--sp-3);
  }
  @media (max-width: 560px) {
    .tr {
      grid-template-columns: minmax(0, 1fr) repeat(4, 56px);
    }
    .horas {
      padding-left: 0;
    }
  }
</style>
