<script lang="ts">
  // «Traer historial» en modo local: el servicio copia (`restic copy`) a este
  // repositorio las versiones de otro (p. ej. el de la app de escritorio, u
  // otro de este equipo). Solo añade: no borra nada, ni aquí ni en el otro, y
  // lo ya traído no se repite. Sigue en el equipo aunque se cierre la ventana.
  import { onDestroy, onMount } from "svelte";
  import { FlaskConical, History, LoaderCircle } from "@lucide/svelte";
  import FormRepoExistente from "$lib/componentes/FormRepoExistente.svelte";
  import { origenCuerpo, repoExistenteCompleto, repoExistenteVacio } from "$lib/direccion";
  import { esBloqueo, servicio } from "../../puente.svelte";
  import type { EstadoLocal, RepoLocal } from "./comun";
  import { estadoHistorial, probarExistente, type EstadoHistorial, type Prueba } from "./existente";

  let { repo, estado, alBloquear, alCerrar }: { repo: RepoLocal; estado: EstadoLocal; alBloquear: () => void; alCerrar: () => void } = $props();

  const otros = $derived(estado.repositorios.filter((r) => r.id !== repo.id));
  let modo = $state<"otro" | "equipo">("otro");
  let origenId = $state("");
  let origen = $state(repoExistenteVacio());
  let ocupado = $state(false);
  let error = $state("");
  let probado = $state<Prueba | null>(null);
  /** Equipos elegidos (de los que copiaron en el origen); vacío: todos. */
  let elegidos = $state<string[]>([]);
  let va = $state<EstadoHistorial | null>(null);
  let vivo = true;
  onDestroy(() => {
    vivo = false;
    origen.contrasena = origen.secreto = "";
  });

  const huella = $derived(JSON.stringify([origen.tipo, origen.direccion, origen.usuario, origen.secreto, origen.ca, origen.contrasena]));
  let probadoCon = "";
  $effect(() => {
    if (huella !== probadoCon) {
      probado = null;
      elegidos = [];
    }
  });

  const enCurso = $derived(va?.estado === "en_marcha");
  const listo = $derived(!enCurso && (modo === "equipo" ? !!origenId : !!probado));

  /** Mientras el panel esté abierto: cómo va (el servicio lo apunta al momento). */
  async function seguir() {
    while (vivo) {
      try {
        va = await estadoHistorial(repo.id);
      } catch (e) {
        if (esBloqueo(e)) return alBloquear();
      }
      if (va?.estado !== "en_marcha") return;
      await new Promise((r) => setTimeout(r, 4000));
    }
  }
  // Si ya se estaba trayendo (o terminó hace poco), se enseña.
  onMount(() => void seguir());

  async function probar() {
    error = "";
    ocupado = true;
    try {
      const h = huella;
      probado = await probarExistente(origen);
      probadoCon = h;
    } catch (e) {
      if (esBloqueo(e)) alBloquear();
      error = (e as Error).message;
    } finally {
      ocupado = false;
    }
  }

  async function traer(e: SubmitEvent) {
    e.preventDefault();
    if (!listo) return;
    error = "";
    ocupado = true;
    try {
      await servicio<{ mensaje: string }>("copiar_historial", {
        historial: {
          repo: repo.id,
          origen: modo === "equipo" ? { repo: origenId } : origenCuerpo(origen),
          ...(modo === "otro" && elegidos.length ? { filtro: { equipos: elegidos } } : {}),
        },
      });
      origen.contrasena = origen.secreto = "";
      va = { estado: "en_marcha", mensaje: "Preparando…" };
      void seguir();
    } catch (err) {
      if (esBloqueo(err)) alBloquear();
      error = (err as Error).message;
    } finally {
      ocupado = false;
    }
  }
</script>

<form class="v-pila sub" onsubmit={traer} aria-label="Traer historial a {repo.nombre}">
  <div class="v-fila">
    <History size={16} aria-hidden="true" />
    <strong>Traer historial a «{repo.nombre}»</strong>
  </div>
  <p class="v-mini">Este equipo copiará aquí las versiones de otro repositorio (por ejemplo, el de la app de escritorio). Solo añade: no borra nada, ni aquí ni en el otro, y lo que ya se trajo no se repite.</p>

  {#if va}
    <p class={va.estado === "fallida" ? "v-error" : va.estado === "hecha" ? "v-ok" : "v-mini en-curso"} role="status">
      {#if va.estado === "en_marcha"}<LoaderCircle size={14} class="spin" aria-hidden="true" />{/if}{va.mensaje}
    </p>
    {#if va.estado === "en_marcha"}<p class="v-mini">Sigue aunque cierres la ventana. Puede tardar: depende de cuánto haya que copiar.</p>{/if}
  {/if}

  {#if !enCurso}
    {#if otros.length}
      <div class="v-fila modos" role="radiogroup" aria-label="De dónde">
        <label class="v-fila"><input type="radio" bind:group={modo} value="otro" />Otro repositorio</label>
        <label class="v-fila"><input type="radio" bind:group={modo} value="equipo" />Uno de este equipo</label>
      </div>
    {/if}
    {#if modo === "equipo"}
      <label class="field">
        <span class="field-label">De</span>
        <select class="input" bind:value={origenId}>
          <option value="" disabled>Elige…</option>
          {#each otros as r (r.id)}<option value={r.id}>{r.nombre}</option>{/each}
        </select>
      </label>
    {:else}
      <FormRepoExistente bind:repo={origen} id="v-historial-{repo.id}" etiquetaTipo="De dónde" nombreEquipo={estado.nombre_equipo} local />
      <div class="v-fila fin">
        <button type="button" class="btn btn-sm" disabled={ocupado || !repoExistenteCompleto(origen)} onclick={probar}><FlaskConical size={14} aria-hidden="true" />Probar</button>
      </div>
      {#if probado}
        <div class="v-pila" role="status">
          <p class="v-ok">{probado.mensaje}</p>
          {#if probado.equipos.length > 1}
            <fieldset class="v-pila equipos">
              <legend class="v-mini">Traer solo las de (sin marcar ninguno: todas)</legend>
              {#each probado.equipos as q (q)}
                <label class="v-fila"><input type="checkbox" checked={elegidos.includes(q)} onchange={(ev) => (elegidos = ev.currentTarget.checked ? [...elegidos, q] : elegidos.filter((x) => x !== q))} />{q}</label>
              {/each}
            </fieldset>
          {/if}
        </div>
      {/if}
    {/if}
  {/if}
  {#if error}<p class="v-error" role="alert">{error}</p>{/if}
  <div class="v-fila fin">
    <button type="button" class="btn btn-ghost btn-sm" onclick={alCerrar}>{enCurso || va?.estado === "hecha" ? "Cerrar" : "Cancelar"}</button>
    {#if !enCurso}<button class="btn btn-primary btn-sm" disabled={ocupado || !listo}>{ocupado ? "Un momento…" : "Traer historial"}</button>{/if}
  </div>
</form>

<style>
  .sub {
    padding-top: var(--sp-3);
    border-top: 1px solid var(--border);
  }
  .fin {
    justify-content: flex-end;
    flex-wrap: wrap;
  }
  .modos {
    gap: var(--sp-4);
    flex-wrap: wrap;
  }
  .equipos {
    margin: 0;
    padding: var(--sp-2) var(--sp-3);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
  }
  .en-curso {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
</style>
