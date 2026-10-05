<script lang="ts">
  // «Retención en el almacén» (v1.22, docs/compartir.md): el repositorio está
  // en un almacén de solo añadir, así que el equipo no puede podar. Con la
  // contraseña del repositorio y la clave de administración:
  // 1) si la regla cambia, se guarda en el equipo dueño (cambiar_retencion, espera);
  // 2) si el almacén aún no tiene una clave que abra el repositorio, se
  //    genera aquí y el equipo dueño la añade (clave_almacen: restic key add);
  // 3) el almacén recibe la regla, el horario y esa clave (retencion_almacen,
  //    espera) y desde entonces la aplica solo, en local, a su hora.
  // Ni la clave ni la contraseña las ve el servidor: van selladas para cada equipo.
  import { onDestroy } from "svelte";
  import { CalendarClock, KeyRound, LoaderCircle, TriangleAlert } from "@lucide/svelte";
  import Modal from "$ui/componentes/Modal.svelte";
  import * as api from "$lib/api";
  import { ErrorLlavesCambiadas, mandarOrden } from "$lib/ordenar";
  import { cuentaAtras } from "$lib/formato";
  import { avisar } from "$lib/avisos.svelte";
  import { cargarCliente } from "$lib/estado.svelte";
  import EditorHorario from "./EditorHorario.svelte";
  import { errorReglas, reglasDe } from "$lib/horario";
  import { horarioEnFrase } from "$lib/formato";
  import type { Horario } from "$lib/tipos";
  import { admitePlazos, copiaRegla, DIAS_CORTOS, errorHorario, errorRegla, HORARIO_POR_DEFECTO, horarioDeCopias, mismaRegla, nuevaClave, REGLA_POR_DEFECTO, reglaDe, reglaParaOrden, textoHorario, type EnAlmacen } from "$lib/retencion";
  import EditorRetencion from "./EditorRetencion.svelte";
  import type { Cliente, Equipo, Orden, Regla, RepositorioResumen } from "$lib/tipos";
  import Ayuda from "./Ayuda.svelte";
  import CampoClave from "./CampoClave.svelte";
  import AlertaLlaves from "./AlertaLlaves.svelte";

  let { cliente, equipo, repo, en, onclose }: { cliente: Cliente; equipo: Equipo; repo: RepositorioResumen; en: EnAlmacen; onclose: () => void } = $props();

  // Lo que ya hay (al abrir): la regla del almacén o, si no, la guardada en el equipo.
  const reglaEquipo = $derived(reglaDe(repo));
  const inicio = () => ({
    regla: copiaRegla(en.retencion?.retencion ?? reglaDe(repo) ?? REGLA_POR_DEFECTO) as Regla,
    dias: [...(en.retencion?.horario.dias ?? HORARIO_POR_DEFECTO.dias)],
    hora: en.retencion?.horario.hora ?? HORARIO_POR_DEFECTO.hora,
    verificar: en.retencion?.verificar ?? true,
  });
  let regla = $state<Regla>(inicio().regla);
  let dias = $state<number[]>(inicio().dias);
  let hora = $state(inicio().hora);
  let verificar = $state(inicio().verificar);
  // v1.3x (almacén con `admite: "retencion_almacen_horario"`): o con un horario de
  // reglas, como el de las copias. `dias` y `hora` siguen para un almacén anterior.
  const admiteReglasAlm = $derived(!!en.almacen.resumen?.admite?.includes("retencion_almacen_horario"));
  const reglasIniciales = () => en.retencion?.horario.reglas ?? [];
  let conReglas = $state(reglasIniciales().length > 0);
  let horarioAlm = $state<Horario>(reglasIniciales().length ? { dias: [], horas: [], reglas: $state.snapshot(reglasIniciales()) } : { dias: inicio().dias, horas: [inicio().hora] });
  const usaReglas = $derived(conReglas && admiteReglasAlm);
  const fraseReglas = $derived(horarioEnFrase({ dias: [], horas: [], reglas: reglasDe(horarioAlm) }).replace(/^./, (c) => c.toLowerCase()));
  let contrasena = $state("");
  let claveAdmin = $state("");
  let ocupado = $state(false);
  let paso = $state("");
  let error = $state("");
  let cambiadas = $state<Equipo | null>(null);
  let hecho = $state<Orden | null>(null);

  const almacen = $derived(en.almacen);
  /** Hace falta otra clave si el almacén no tiene una que ya abra el repositorio. */
  const claveNueva = $derived(en.retencion?.clave !== "ok");
  const cambiaRegla = $derived(!mismaRegla(regla, reglaEquipo));
  const pideContrasena = $derived(claveNueva || cambiaRegla);
  /** Horarias, plazos y «siempre» (v1.28): los dos agentes tienen que entenderlos. */
  const admite = $derived(admitePlazos(equipo) && admitePlazos(en.almacen));
  const errReg = $derived(errorRegla(regla, admite));
  const errHor = $derived(errorHorario({ dias, hora }) ?? (usaReglas ? errorReglas(reglasDe(horarioAlm)) : null));
  const listo = $derived(!errReg && !errHor && !!claveAdmin && (!pideContrasena || !!contrasena));
  const espera = $derived(almacen.espera_min_horas ?? cliente.espera_min_horas);

  onDestroy(() => (contrasena = claveAdmin = ""));

  function alternar(d: number) {
    dias = dias.includes(d) ? dias.filter((x) => x !== d) : [...dias, d].sort((a, b) => a - b);
  }

  /** Espera la respuesta firmada de una orden (como mucho 2 min). */
  async function respuesta(eq: Equipo, o: Orden): Promise<Orden> {
    for (let i = 0; i < 80; i++) {
      const x = (await api.ordenesEquipo(cliente.id, eq.id, 10)).find((y) => y.id === o.id);
      if (x && ["hecha", "fallida", "rechazada", "cancelada", "caducada"].includes(x.estado)) return x;
      await new Promise((r) => setTimeout(r, 1500));
    }
    throw new Error(`${eq.nombre} no ha respondido todavía: tiene que estar encendido para añadir la clave del almacén. Vuelve a intentarlo cuando lo esté.`);
  }

  async function guardar(e: SubmitEvent) {
    e.preventDefault();
    error = "";
    ocupado = true;
    const secretosRepo = { claveAdmin, repo: { repo: repo.id, contrasena } };
    let clave: string | undefined;
    try {
      // 1) La regla, guardada en el repositorio (en el equipo dueño), si cambia.
      if (cambiaRegla) {
        await mandarOrden({ cliente, equipo, tipo: "cambiar_retencion", cuerpo: { repo: repo.id, ...reglaParaOrden(regla) }, secretos: secretosRepo, alPaso: (t) => (paso = t) });
      }
      // 2) Una clave propia del almacén, añadida por el equipo dueño (antes de mandarla al almacén).
      if (claveNueva) {
        clave = nuevaClave();
        const o = await mandarOrden({ cliente, equipo, tipo: "clave_almacen", cuerpo: { repo: repo.id, clave }, secretos: secretosRepo, alPaso: (t) => (paso = t) });
        paso = `Esperando a ${equipo.nombre}…`;
        const r = await respuesta(equipo, o);
        // Un agente anterior a v1.22 no conoce la orden: hay que actualizarlo.
        if (r.estado !== "hecha" && /desconocid|aún no admite/i.test(r.mensaje ?? "")) throw new Error(`${equipo.nombre} necesita un agente más nuevo para darle su clave al almacén: actualízalo y vuelve a intentarlo.`);
        if (r.estado !== "hecha") throw new Error(r.mensaje ?? `${equipo.nombre} no pudo añadir la clave del almacén.`);
      }
      // 3) La regla, el horario y la clave, al almacén (espera).
      hecho = await mandarOrden({
        cliente,
        equipo: almacen,
        tipo: "retencion_almacen",
        cuerpo: { usuario: en.usuario, repo: en.carpeta, ...(clave ? { clave } : {}), retencion: reglaParaOrden(regla), horario: { dias: [...dias], hora, ...(usaReglas ? { reglas: reglasDe(horarioAlm) } : {}) }, verificar },
        secretos: { claveAdmin },
        alPaso: (t) => (paso = t),
      });
      avisar(`Retención de «${repo.nombre}» enviada a ${almacen.nombre}.`);
      void cargarCliente(cliente.id, { silencioso: true });
    } catch (err) {
      if (err instanceof ErrorLlavesCambiadas) cambiadas = err.equipo === almacen.nombre ? almacen : equipo;
      else error = (err as Error).message;
    } finally {
      contrasena = claveAdmin = "";
      clave = undefined;
      ocupado = false;
      paso = "";
    }
  }
</script>

<Modal labelledby="t-ret-alm" {onclose} width={560} dismissible={!ocupado}>
  <div class="dlg-title">
    <span class="ticon danger"><CalendarClock size={18} /></span>
    <div>
      <h2 id="t-ret-alm">Retención en {almacen.nombre}</h2>
      <p>«{repo.nombre}» de {equipo.nombre} está en {almacen.nombre}, que es de solo añadir: desde el equipo no se puede borrar nada. La retención la aplica el almacén, en local y a su hora. <Ayuda id="retencion-almacen" /></p>
    </div>
  </div>

  {#if cambiadas}
    <AlertaLlaves equipo={cambiadas} cliente={cliente.id} />
    <footer><button class="btn btn-primary" onclick={onclose}>Entendido</button></footer>
  {:else if hecho}
    <div class="form">
      <p>
        Listo. {almacen.nombre} aplicará la retención de «{repo.nombre}» {usaReglas ? fraseReglas : textoHorario({ dias, hora })}
        {#if hecho.not_before}cuando pase la espera (dentro de <strong>{cuentaAtras(hecho.not_before)}</strong>), salvo que alguien la cancele desde <a href="/c/{cliente.id}/ordenes">Órdenes</a>.{:else}.{/if}
      </p>
      <footer><button class="btn btn-primary" onclick={onclose}>Cerrar</button></footer>
    </div>
  {:else}
    <form class="form" onsubmit={guardar}>
      <EditorRetencion id="ra" bind:regla {admite} {...horarioDeCopias(equipo.resumen?.copias, repo.id)} />

      {#if admiteReglasAlm}
        <label class="switch-row"><input type="checkbox" class="switch" bind:checked={conReglas} /><span>Con un horario como el de las copias<span class="faint">Cada N días, un día de cada mes, varias horas…</span></span></label>
      {/if}
      {#if usaReglas}
        <EditorHorario id="ra-horario" bind:horario={horarioAlm} admiteReglas para="verificacion" />
        {#if errHor}<p class="error-campo" role="alert">{errHor}</p>{/if}
      {:else}
      <fieldset class="horario">
        <legend class="field-label">Cuándo la aplica</legend>
        <div class="dias" role="group" aria-label="Días">
          {#each DIAS_CORTOS as d, i (d)}
            <button type="button" class="dia" class:on={dias.includes(i + 1)} aria-pressed={dias.includes(i + 1)} onclick={() => alternar(i + 1)}>{d}</button>
          {/each}
        </div>
        <label class="hora">a las <input class="input" type="time" bind:value={hora} /></label>
      </fieldset>
      <p class="faint nota">{errHor ?? `${textoHorario({ dias, hora })[0].toUpperCase()}${textoHorario({ dias, hora }).slice(1)}, hora de ${almacen.nombre}. Mejor fuera de las horas de copia: mientras poda, las copias de este repositorio esperan.`}</p>
      {/if}
      <label class="switch-row"><input type="checkbox" bind:checked={verificar} /><span>Comprobar el repositorio después (que todo se puede leer)</span></label>

      <div class="notice notice-warn">
        <KeyRound size={16} />
        <p>
          {#if claveNueva}{equipo.nombre} añadirá al repositorio una <strong>clave propia de {almacen.nombre}</strong> (tiene que estar encendido). Con ella el almacén puede podar, y también <strong>leer</strong> este repositorio. Se revoca con «Dejar de aplicarla».{:else}{almacen.nombre} ya tiene su clave de este repositorio: solo cambian la regla o el horario.{/if}
          Borra versiones, así que <strong>espera {espera} h</strong> antes de aplicarse y cualquiera del cliente puede cancelarla. Después se aplica sola.
        </p>
      </div>

      {#if pideContrasena}
        <CampoClave requerido id="ra-repo" etiqueta="Contraseña del repositorio «{repo.nombre}»" bind:value={contrasena} autofocus ayuda="Está en el kit de recuperación. Va cifrada solo para {equipo.nombre}." />
      {/if}
      <CampoClave requerido id="ra-admin" etiqueta="Clave de administración de {cliente.nombre}" bind:value={claveAdmin} autofocus={!pideContrasena} ayuda="Autoriza la retención en {almacen.nombre}. El servidor nunca la ve.">
        {#snippet extra()}<Ayuda id="clave-admin" />{/snippet}
      </CampoClave>

      {#if error}<div class="notice notice-danger" role="alert"><TriangleAlert size={16} /><p>{error}</p></div>{/if}
      <footer>
        {#if ocupado}<span class="espera" role="status"><LoaderCircle size={15} class="spin" />{paso}</span>{/if}
        <button type="button" class="btn btn-ghost" disabled={ocupado} onclick={onclose}>Cancelar</button>
        <button class="btn btn-danger" disabled={ocupado || !listo}>{pideContrasena ? "Confirmar con las dos claves" : "Confirmar con la clave de administración"}</button>
      </footer>
    </form>
  {/if}
</Modal>

<style>
  fieldset {
    margin: 0;
    padding: 0;
    border: none;
  }
  .horario legend {
    margin-bottom: 6px;
  }
  .horario {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--sp-3);
  }
  .dias {
    display: inline-flex;
    gap: 4px;
  }
  .dia {
    width: 32px;
    height: 32px;
    border: 1px solid var(--border-strong);
    border-radius: var(--radius);
    background: var(--surface);
    color: var(--text-2);
    font-weight: 600;
    cursor: pointer;
  }
  .dia.on {
    background: var(--accent, var(--text-1));
    border-color: transparent;
    color: var(--surface);
  }
  .hora {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: var(--fs-sm);
  }
  .hora .input {
    width: 7.5em;
  }
  .nota {
    margin: 0;
    font-size: var(--fs-sm);
  }
  .espera {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    margin-right: auto;
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
</style>
