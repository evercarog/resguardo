<script lang="ts">
  // Ajustes (se carga al abrirlo): todo pide la clave de administración del
  // equipo, que comprueba el servicio (ipc_local). Sin clave todavía: «Usar
  // sin consola». Con ella: la ventana y los avisos; en modo local, además,
  // todo lo del equipo (copias, repositorios, restaurar…).
  import { onMount } from "svelte";
  import { KeyRound, Lock, LockOpen, ShieldCheck } from "@lucide/svelte";
  import { esBloqueo, pedir, servicio, vivo } from "../puente.svelte";
  import type { Avisos, Escritorio, Modo, Ventana } from "../tipos";
  import CampoClave from "./CampoClave.svelte";

  let modo = $state<Modo | null>(null);
  let abierta = $state(false);
  let clave = $state("");
  let clave2 = $state("");
  let ocupado = $state(false);
  let error = $state("");
  let hecho = $state("");

  onMount(async () => {
    try {
      modo = (await pedir<{ modo: Modo }>("hola")).modo;
      abierta = (await pedir<{ abierta: boolean }>("desbloqueada")).abierta;
    } catch (e) {
      error = (e as Error).message;
    }
  });

  async function desbloquear(ev: SubmitEvent) {
    ev.preventDefault();
    ocupado = true;
    error = "";
    try {
      const r = await pedir<{ modo: Modo }>("desbloquear", { clave });
      modo = r.modo;
      abierta = true;
      clave = "";
    } catch (e) {
      error = (e as Error).message;
    } finally {
      ocupado = false;
    }
  }

  async function crear(ev: SubmitEvent) {
    ev.preventDefault();
    error = "";
    if (clave.length < 12) return void (error = "La clave tiene que tener al menos 12 caracteres.");
    if (clave !== clave2) return void (error = "Las dos claves no coinciden.");
    ocupado = true;
    try {
      await pedir("crear_clave", { clave });
      modo = "local";
      abierta = true;
      clave = clave2 = "";
    } catch (e) {
      error = (e as Error).message;
    } finally {
      ocupado = false;
    }
  }

  async function bloquear() {
    await pedir("bloquear").catch(() => {});
    abierta = false;
  }

  // ---- La ventana y los avisos ----
  const actual = $derived<Escritorio>(vivo.datos?.bandeja?.escritorio ?? { ventana: "siempre_disponible", avisos: "errores" });
  let pedido = $state<Escritorio | null>(null);
  const esc = $derived(pedido ?? actual);
  async function poner(cambio: Partial<Escritorio>) {
    const nuevo = { ...esc, ...cambio };
    pedido = nuevo;
    error = hecho = "";
    try {
      await servicio("ajustes", { escritorio: nuevo });
      hecho = modo === "gestionado" ? "Guardado. La consola verá el cambio («cambiado en el equipo»)." : "Guardado.";
    } catch (e) {
      if (esBloqueo(e)) abierta = false;
      error = (e as Error).message;
      pedido = null;
    }
  }
  const VENTANAS: [Ventana, string][] = [
    ["off", "Sin ventana"],
    ["siempre_disponible", "Desde el icono"],
    ["al_trabajar", "Al trabajar"],
  ];
  const AVISOS: [Avisos, string][] = [
    ["off", "Ninguno"],
    ["errores", "Errores"],
    ["todo", "Todo"],
  ];
  const cargarLocal = () => import("./local/Local.svelte");
</script>

<div class="v-pila">
  {#if modo === null}
    {#if error}<p class="v-error" role="alert">{error}</p>{:else}<p class="v-sub">Preguntando al servicio…</p>{/if}
  {:else if modo === "web"}
    <div class="v-tarjeta"><p class="v-sub">Este equipo lo gestiona la consola web de Resguardo: sus ajustes se cambian allí.</p></div>
  {:else if modo === "pendiente"}
    <div class="v-tarjeta"><p class="v-sub">Este equipo se está vinculando con una consola: termina el alta en la consola. Después, sus ajustes se cambian con la clave de administración.</p></div>
  {:else if modo === "sin_clave"}
    <form class="v-tarjeta v-pila" onsubmit={crear}>
      <div class="v-fila"><ShieldCheck size={20} aria-hidden="true" /><h2 class="v-titulo">Usar sin consola</h2></div>
      <p class="v-sub">
        Elige una <b>clave de administración</b> para este equipo. Con ella se cambian las copias, se restaura y se ajusta todo aquí. Guárdala bien: si más
        adelante vinculas el equipo a una consola, se usa la misma y el equipo conserva todo.
      </p>
      <CampoClave etiqueta="Clave de administración" bind:valor={clave} nueva />
      <CampoClave etiqueta="Repite la clave" bind:valor={clave2} nueva />
      <p class="v-mini">Al menos 12 caracteres. Una frase de varias palabras es fácil de recordar y difícil de adivinar.</p>
      {#if error}<p class="v-error" role="alert">{error}</p>{/if}
      <button class="btn btn-primary" disabled={ocupado}>{ocupado ? "Guardando (unos segundos)…" : "Poner la clave"}</button>
    </form>
  {:else if !abierta}
    <form class="v-tarjeta v-pila" onsubmit={desbloquear}>
      <div class="v-fila"><Lock size={18} aria-hidden="true" /><h2 class="v-titulo">Ajustes de este equipo</h2></div>
      <p class="v-sub">Escribe la clave de administración del equipo{modo === "gestionado" ? " (la de la consola)" : ""}.</p>
      <CampoClave etiqueta="Clave de administración" bind:valor={clave} />
      {#if error}<p class="v-error" role="alert">{error}</p>{/if}
      <button class="btn btn-primary" disabled={ocupado || !clave}>{ocupado ? "Comprobando…" : "Desbloquear"}</button>
    </form>
  {:else}
    <section class="v-tarjeta v-pila" aria-labelledby="t-escritorio">
      <div class="v-fila">
        <LockOpen size={18} aria-hidden="true" />
        <h2 class="v-titulo" id="t-escritorio">En este equipo</h2>
        <button class="btn btn-ghost btn-sm bloquear" onclick={bloquear}><KeyRound size={14} aria-hidden="true" />Bloquear</button>
      </div>
      <div class="field">
        <span class="field-label" id="l-ventana">Ventana de Resguardo</span>
        <div class="segmented" role="radiogroup" aria-labelledby="l-ventana">
          {#each VENTANAS as [v, t] (v)}<button role="radio" aria-checked={esc.ventana === v} class:on={esc.ventana === v} onclick={() => poner({ ventana: v })}>{t}</button>{/each}
        </div>
        <span class="field-hint">«Al trabajar»: se abre sola al empezar una copia, una restauración, una verificación o una subida.</span>
      </div>
      <div class="field">
        <span class="field-label" id="l-avisos">Avisos</span>
        <div class="segmented" role="radiogroup" aria-labelledby="l-avisos">
          {#each AVISOS as [v, t] (v)}<button role="radio" aria-checked={esc.avisos === v} class:on={esc.avisos === v} onclick={() => poner({ avisos: v })}>{t}</button>{/each}
        </div>
        <span class="field-hint">«Errores»: al fallar y al recuperarse. «Todo»: también al empezar y al terminar. Windows los guarda en silencio en «No molestar».</span>
      </div>
      {#if hecho}<p class="v-ok" role="status">{hecho}</p>{/if}
      {#if error}<p class="v-error" role="alert">{error}</p>{/if}
    </section>
    {#if modo === "local"}
      {#await cargarLocal()}
        <p class="v-sub">Cargando…</p>
      {:then m}
        <m.default alBloquear={() => (abierta = false)} />
      {/await}
    {:else if modo === "gestionado"}
      <p class="v-mini">Las copias de este equipo las decide la consola que lo gestiona.</p>
    {/if}
  {/if}
</div>

<style>
  .bloquear {
    margin-left: auto;
  }
</style>
