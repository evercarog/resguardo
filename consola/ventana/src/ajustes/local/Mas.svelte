<script lang="ts">
  // Lo demás del equipo en modo local: pausar, el historial completo, el kit
  // de recuperación, cambiar la clave, «Este equipo guarda copias» (con su
  // espejo a otra carpeta o a Dropbox / Google Drive) y vincularlo a una
  // consola sin perder nada.
  import { Cloud, History, KeyRound, Link2, Pause, Play, Printer, Server } from "@lucide/svelte";
  import { bytes, fechaCorta } from "$lib/formato";
  import { esBloqueo, pedir, servicio } from "../../puente.svelte";
  import CampoClave from "../CampoClave.svelte";
  import type { PropsParte } from "./comun";
  import ElegirCarpeta from "./ElegirCarpeta.svelte";
  import Kit from "./Kit.svelte";

  let { estado, recargar, alBloquear }: PropsParte = $props();
  type Abierto = null | "historial" | "kit" | "clave" | "guarda" | "vincular";
  let abierto = $state<Abierto>(null);
  let ocupado = $state(false);
  let error = $state("");
  let hecho = $state("");

  async function hacer<T>(f: () => Promise<T>): Promise<T | null> {
    ocupado = true;
    error = hecho = "";
    try {
      const r = await f();
      await recargar();
      return r;
    } catch (e) {
      if (esBloqueo(e)) alBloquear();
      error = (e as Error).message;
      return null;
    } finally {
      ocupado = false;
    }
  }
  const mensaje = (r: unknown) => (r as { mensaje?: string } | null)?.mensaje ?? "";

  // ---- Pausa ----
  async function pausar(horas: number) {
    const r = await hacer(() => servicio("pausar", { horas }));
    if (r) hecho = mensaje(r);
  }
  async function reanudar() {
    const r = await hacer(() => servicio("reanudar"));
    if (r) hecho = mensaje(r);
  }

  // ---- Historial completo ----
  interface Entrada {
    tipo: string;
    repo: string;
    copia?: string | null;
    cuando: string;
    resultado: string;
    mensaje: string;
    bytes?: number | null;
  }
  let historial = $state<Entrada[] | null>(null);
  async function verHistorial() {
    abierto = "historial";
    historial = await hacer(() => servicio<Entrada[]>("historial"));
  }
  const TIPOS: Record<string, string> = { backup: "Copia", verify: "Verificación", offsite: "Copia externa", config: "Cambio", pause: "Pausa", resume: "Reanudar", kit: "Kit" };

  // ---- Clave ----
  let clave = $state("");
  let clave2 = $state("");
  async function cambiarClave(e: SubmitEvent) {
    e.preventDefault();
    if (clave.length < 12) return void (error = "Al menos 12 caracteres.");
    if (clave !== clave2) return void (error = "Las dos claves no coinciden.");
    const r = await hacer(() => pedir("cambiar_clave", { clave }));
    if (r) {
      hecho = "Clave cambiada. Desde ahora, la nueva (también para vincular el equipo a una consola).";
      clave = clave2 = "";
      abierto = null;
    }
  }

  // ---- Este equipo guarda copias (Servidor de copias) y su espejo ----
  const gc = $derived(estado.resumen.guarda_copias ?? null);
  let carpetaServidor = $state("");
  let puerto = $state(8000);
  let elegir = $state<null | "servidor" | "espejo">(null);
  let espejoTipo = $state<"carpeta" | "nube">("carpeta");
  let espejoCarpeta = $state("");
  let espejoNube = $state("");
  let espejoHora = $state("01:00");
  let nubeTipo = $state<"dropbox" | "drive">("dropbox");
  let nubeNombre = $state("");
  async function activarServidor() {
    const r = await hacer(() => servicio("guarda_copias", { cuerpo: { activo: true, carpeta: carpetaServidor, puerto } }));
    if (r) hecho = mensaje(r);
  }
  async function desactivarServidor() {
    if (!confirm("¿Dejar de guardar copias aquí? Lo guardado se queda en la carpeta.")) return;
    const r = await hacer(() => servicio("guarda_copias", { cuerpo: { activo: false } }));
    if (r) hecho = mensaje(r);
  }
  async function ponerEspejo(quitar = false) {
    const destino = espejoTipo === "carpeta" ? { tipo: "carpeta", carpeta: espejoCarpeta } : { tipo: "nube", nube: espejoNube, carpeta: "Resguardo" };
    const r = await hacer(() => servicio("guarda_copias", { cuerpo: { espejo: quitar ? null : { destinos: [destino], hora: espejoHora } } }));
    if (r) hecho = mensaje(r);
  }
  async function conectarNube() {
    hecho = "Se abre el navegador para dar permiso a Resguardo. Vuelve aquí cuando termines.";
    const r = await hacer(() => pedir("nube_autorizar", { tipo: nubeTipo, nombre: nubeNombre.trim() }));
    if (r) {
      hecho = mensaje(r);
      nubeNombre = "";
    }
  }
  async function quitarNube(nombre: string) {
    if (!confirm(`¿Olvidar la nube «${nombre}»? Lo ya subido se queda en ella.`)) return;
    const r = await hacer(() => servicio("quitar_nube", { nombre }));
    if (r) hecho = mensaje(r);
  }

  // ---- Vincular ----
  let url = $state("https://");
  let codigo = $state("");
  let vinculado = $state<null | { sas: string; huella_ca: string; servidor: string; espera_alta: boolean; sas_v3: boolean }>(null);
  async function vincular(e: SubmitEvent) {
    e.preventDefault();
    vinculado = await hacer(() => servicio("vincular", { url: url.trim(), codigo: codigo.trim() }));
  }
</script>

{#if elegir}
  <ElegirCarpeta
    titulo={elegir === "servidor" ? "Carpeta donde guardar las copias" : "Carpeta del espejo"}
    crear
    alCerrar={() => (elegir = null)}
    alElegir={(rs) => {
      if (elegir === "servidor") carpetaServidor = rs[0];
      else espejoCarpeta = rs[0];
      elegir = null;
    }}
  />
{/if}

<div class="v-pila">
  <section class="v-tarjeta v-pila">
    <div class="v-fila"><Pause size={16} aria-hidden="true" /><h3 class="v-titulo">Pausar las copias automáticas</h3></div>
    {#if estado.pausado_hasta}
      <p class="v-sub">En pausa {estado.pausado_hasta === "indefinido" ? "hasta reanudarlas" : `hasta ${fechaCorta(estado.pausado_hasta)}`}.</p>
      <button class="btn btn-sm" disabled={ocupado} onclick={reanudar}><Play size={14} />Reanudar</button>
    {:else}
      <div class="v-fila botones">
        <button class="btn btn-sm" disabled={ocupado} onclick={() => pausar(1)}>1 hora</button>
        <button class="btn btn-sm" disabled={ocupado} onclick={() => pausar(4)}>4 horas</button>
        <button class="btn btn-sm" disabled={ocupado} onclick={() => pausar(24)}>Hasta mañana</button>
        <button class="btn btn-sm" disabled={ocupado} onclick={() => pausar(0)}>Hasta reanudar</button>
      </div>
    {/if}
  </section>

  <div class="v-fila botones">
    <button class="btn btn-sm" onclick={verHistorial}><History size={14} />Historial completo</button>
    <button class="btn btn-sm" onclick={() => (abierto = abierto === "kit" ? null : "kit")}><Printer size={14} />Kit de recuperación</button>
    <button class="btn btn-sm" onclick={() => (abierto = abierto === "clave" ? null : "clave")}><KeyRound size={14} />Cambiar la clave</button>
    <button class="btn btn-sm" onclick={() => (abierto = abierto === "guarda" ? null : "guarda")}><Server size={14} />Guardar copias de otros</button>
    <button class="btn btn-sm" onclick={() => (abierto = abierto === "vincular" ? null : "vincular")}><Link2 size={14} />Vincular a una consola</button>
  </div>

  {#if abierto === "historial"}
    <section class="v-tarjeta">
      <ul class="historial">
        {#each historial ?? [] as h, i (i)}
          <li>
            <span class="badge badge-sm" class:tone-ok={h.resultado === "ok"} class:tone-warn={h.resultado === "warning"} class:tone-danger={h.resultado === "error"}>{TIPOS[h.tipo] ?? h.tipo}</span>
            <span class="v-cortar"><b>{h.copia ?? h.repo}</b> <span class="v-mini">{h.mensaje}</span></span>
            <span class="v-mini cuando">{fechaCorta(h.cuando)}{h.bytes ? ` · ${bytes(h.bytes)}` : ""}</span>
          </li>
        {:else}
          <li class="v-sub">{historial ? "Aún no hay nada." : "Leyendo…"}</li>
        {/each}
      </ul>
    </section>
  {:else if abierto === "kit"}
    <Kit nombreEquipo={estado.nombre_equipo} />
    <button class="btn btn-sm" onclick={() => window.print()}><Printer size={14} />Imprimir</button>
  {:else if abierto === "clave"}
    <form class="v-tarjeta v-pila" onsubmit={cambiarClave}>
      <p class="v-sub">La clave de administración de este equipo. Si después lo vinculas a una consola, la consola tiene que usar la misma.</p>
      <CampoClave etiqueta="Clave nueva" bind:valor={clave} nueva />
      <CampoClave etiqueta="Repite la clave nueva" bind:valor={clave2} nueva />
      <button class="btn btn-primary" disabled={ocupado}>{ocupado ? "Cambiando…" : "Cambiar la clave"}</button>
    </form>
  {:else if abierto === "guarda"}
    <section class="v-tarjeta v-pila">
      <p class="v-sub">Este equipo puede guardar copias (un Servidor de copias de solo añadir) y copiarlo cada noche a otra carpeta o a la nube (el espejo).</p>
      {#if gc?.activo}
        <p class="v-sub">Guarda copias en <span class="mono">{gc.carpeta}</span>, puerto {gc.puerto}.</p>
        <h4 class="v-titulo">Espejo cada noche</h4>
        {#if gc.espejo}<p class="v-mini">Ahora: a las {gc.espejo.hora} en {(gc.espejo.destinos ?? []).map((d) => d.nube ?? d.carpeta).join(", ")}.</p>{/if}
        <div class="segmented inline">
          <button class:on={espejoTipo === "carpeta"} onclick={() => (espejoTipo = "carpeta")}>Otra carpeta</button>
          <button class:on={espejoTipo === "nube"} onclick={() => (espejoTipo = "nube")}>La nube</button>
        </div>
        {#if espejoTipo === "carpeta"}
          <div class="v-fila"><input class="input mono" bind:value={espejoCarpeta} placeholder="F:\Espejo" aria-label="Carpeta del espejo" /><button class="btn btn-sm" onclick={() => (elegir = "espejo")}>Elegir</button></div>
        {:else}
          <select class="input" bind:value={espejoNube} aria-label="Nube">
            <option value="">Elige una nube conectada…</option>
            {#each estado.nubes as n (n.nombre)}<option value={n.nombre}>{n.nombre} ({n.tipo === "drive" ? "Google Drive" : "Dropbox"})</option>{/each}
          </select>
        {/if}
        <label class="v-fila"><span class="field-label">A las</span><input class="input hora" type="time" bind:value={espejoHora} /></label>
        <div class="v-fila botones">
          <button class="btn btn-primary btn-sm" disabled={ocupado || (espejoTipo === "carpeta" ? !espejoCarpeta : !espejoNube)} onclick={() => ponerEspejo()}>Poner el espejo</button>
          {#if gc.espejo}<button class="btn btn-ghost btn-sm" disabled={ocupado} onclick={() => ponerEspejo(true)}>Quitar el espejo</button>{/if}
          <button class="btn btn-ghost btn-sm" disabled={ocupado} onclick={desactivarServidor}>Dejar de guardar copias</button>
        </div>
      {:else}
        <div class="v-fila"><input class="input mono" bind:value={carpetaServidor} placeholder="D:\CopiasDeOtros" aria-label="Carpeta" /><button class="btn btn-sm" onclick={() => (elegir = "servidor")}>Elegir</button></div>
        <label class="v-fila"><span class="field-label">Puerto</span><input class="input hora" type="number" min="1024" max="65535" bind:value={puerto} /></label>
        <button class="btn btn-primary btn-sm" disabled={ocupado || !carpetaServidor} onclick={activarServidor}>Guardar copias aquí</button>
      {/if}

      <h4 class="v-titulo"><Cloud size={15} aria-hidden="true" /> Nubes conectadas</h4>
      <ul class="nubes">
        {#each estado.nubes as n (n.nombre)}<li class="v-fila"><span class="v-cortar">{n.nombre} · {n.tipo === "drive" ? "Google Drive" : "Dropbox"}</span><button class="btn btn-ghost btn-sm" onclick={() => quitarNube(n.nombre)}>Quitar</button></li>{:else}<li class="v-mini">Ninguna.</li>{/each}
      </ul>
      <div class="v-fila">
        <select class="input corto" bind:value={nubeTipo} aria-label="Tipo de nube"><option value="dropbox">Dropbox</option><option value="drive">Google Drive</option></select>
        <input class="input" bind:value={nubeNombre} placeholder="Nombre (p. ej. Dropbox de la oficina)" aria-label="Nombre de la nube" />
        <button class="btn btn-sm" disabled={ocupado || !nubeNombre.trim()} onclick={conectarNube}>Conectar</button>
      </div>
    </section>
  {:else if abierto === "vincular"}
    <form class="v-tarjeta v-pila" onsubmit={vincular}>
      <p class="v-sub">
        El equipo conserva sus copias, repositorios y ajustes. Queda pendiente hasta que la consola lo dé de alta con <b>esta misma clave de administración</b> (un código solo no basta para quedarse con el equipo).
      </p>
      <label class="field"><span class="field-label">Dirección de la consola</span><input class="input mono" bind:value={url} placeholder="https://192.168.1.20:8443" /></label>
      <label class="field"><span class="field-label">Código de vinculación</span><input class="input mono" bind:value={codigo} placeholder="ABCD-EFGH-JK" /></label>
      <button class="btn btn-primary" disabled={ocupado || !codigo.trim() || url.length < 10}>{ocupado ? "Vinculando…" : "Vincular"}</button>
      {#if vinculado}
        <div class="notice notice-info">
          <p>Código de comprobación: <b class="mono">{vinculado.sas}</b>. Comprueba que la consola enseña el mismo antes de confirmar.</p>
          {#if !vinculado.sas_v3}<p class="v-mini">Compara también la huella de la autoridad: <span class="mono">{vinculado.huella_ca}</span>.</p>{/if}
          {#if vinculado.espera_alta}<p class="v-mini">Pendiente del alta en la consola con la misma clave de administración.</p>{/if}
        </div>
      {/if}
    </form>
  {/if}
  {#if hecho}<p class="v-ok" role="status">{hecho}</p>{/if}
  {#if error}<p class="v-error" role="alert">{error}</p>{/if}
</div>

<style>
  .botones {
    flex-wrap: wrap;
  }
  .historial,
  .nubes {
    display: grid;
    gap: 6px;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .historial li {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    min-width: 0;
    font-size: var(--fs-sm);
  }
  .cuando {
    margin-left: auto;
    flex: none;
  }
  .hora {
    width: 120px;
  }
  .corto {
    width: auto;
  }
</style>
