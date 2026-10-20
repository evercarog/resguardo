<script lang="ts">
  // 0.8.1 (H1, 15mm): «Recuperar un archivo». La persona del equipo recupera las versiones
  // anteriores de lo suyo, sin clave: el servicio comprueba con su cuenta lo que puede leer,
  // nunca sobrescribe (lo deja al lado, con la fecha en el nombre) y no enseña ni contraseñas
  // ni el repositorio. Desde el Explorador («Versiones anteriores con Resguardo») llega la ruta.
  import { untrack } from "svelte";
  import { ArchiveRestore, File, Folder, FolderOpen, LoaderCircle, Search } from "@lucide/svelte";
  import { relativo } from "$lib/formato";
  import { pedir, vivo } from "./puente.svelte";

  interface Version {
    version: string;
    cuando: string;
    carpeta: boolean;
    bytes?: number | null;
    modificado?: string | null;
  }
  interface Estado {
    activo: boolean;
    max_mb: number;
    motivo_obligatorio: boolean;
  }

  const MOTIVOS = [
    { id: "borrado", texto: "Archivo borrado por error" },
    { id: "dano", texto: "Daño" },
    { id: "prueba", texto: "Prueba" },
    { id: "otro", texto: "Otro" },
  ];

  let estado = $state<Estado | null>(null);
  let ruta = $state("");
  let versiones = $state<Version[] | null>(null);
  let elegida = $state("");
  let motivo = $state("");
  let notas = $state("");
  let otraCarpeta = $state(false);
  let carpeta = $state("");
  let ocupado = $state(false);
  let error = $state("");
  let hecho = $state<{ donde: string; archivos: number; sin_permiso: number } | null>(null);

  async function cargarEstado() {
    try {
      estado = await pedir<Estado>("recuperar_estado");
    } catch (e) {
      error = (e as Error).message;
    }
  }
  void cargarEstado();

  async function buscar(e?: SubmitEvent) {
    e?.preventDefault();
    if (!ruta.trim()) return;
    error = "";
    hecho = null;
    versiones = null;
    elegida = "";
    ocupado = true;
    try {
      const r = await pedir<{ versiones: Version[] }>("recuperar_versiones", { ruta: ruta.trim() });
      versiones = r.versiones;
      elegida = r.versiones[0]?.version ?? "";
    } catch (x) {
      error = (x as Error).message;
    } finally {
      ocupado = false;
    }
  }

  // Lo pedido desde el Explorador: la ruta, y se buscan sus versiones (una vez por petición).
  let pedidoVisto = "";
  $effect(() => {
    const p = vivo.datos?.recuperar ?? null;
    if (!p?.ruta || p.hora === pedidoVisto) return;
    untrack(() => {
      pedidoVisto = p.hora ?? "";
      ruta = p.ruta;
      void pedir("recuperar_visto");
      void buscar();
    });
  });

  const falta = $derived(!elegida ? "Elige una versión." : estado?.motivo_obligatorio && !motivo ? "Di por qué (un toque basta)." : otraCarpeta && !carpeta.trim() ? "Escribe la carpeta donde dejarlo." : "");

  async function recuperar() {
    if (falta || ocupado) return;
    error = "";
    ocupado = true;
    try {
      hecho = await pedir("recuperar", {
        ruta: ruta.trim(),
        version: elegida,
        motivo: motivo ? { tipo: motivo, ...(notas.trim() ? { notas: notas.trim().slice(0, 500) } : {}) } : null,
        carpeta: otraCarpeta ? carpeta.trim() : null,
      });
    } catch (x) {
      error = (x as Error).message;
    } finally {
      ocupado = false;
    }
  }

  const tamano = (b?: number | null) => (b == null ? "" : b < 1024 ? `${b} B` : b < 1024 ** 2 ? `${Math.round(b / 1024)} KB` : b < 1024 ** 3 ? `${(b / 1024 ** 2).toFixed(1).replace(".", ",")} MB` : `${(b / 1024 ** 3).toFixed(1).replace(".", ",")} GB`);
  const fecha = (t: string) => new Date(t).toLocaleString("es", { day: "numeric", month: "short", year: "numeric", hour: "2-digit", minute: "2-digit" });
</script>

<div class="recuperar">
  <div class="v-tarjeta cab">
    <ArchiveRestore size={20} aria-hidden="true" />
    <div>
      <p class="titulo">Recuperar un archivo</p>
      <p class="v-mini">Las versiones anteriores de lo tuyo. Se deja al lado del original, con la fecha en el nombre: nunca se sobrescribe nada.</p>
    </div>
  </div>

  {#if estado && !estado.activo}
    <p class="v-sub aviso">Recuperar desde el equipo no está activado aquí. Pide a quien administra las copias que lo active o que te lo restaure.</p>
  {:else}
    <form class="v-tarjeta campo" onsubmit={buscar}>
      <label class="v-mini" for="rec-ruta">Archivo o carpeta (la ruta entera, p. ej. C:\Users\tu-nombre\Documentos\informe.xlsx)</label>
      <div class="fila">
        <input id="rec-ruta" class="input" bind:value={ruta} spellcheck="false" autocomplete="off" placeholder="C:\Users\…" />
        <button class="btn btn-sm" disabled={ocupado || !ruta.trim()}><Search size={14} aria-hidden="true" />Ver versiones</button>
      </div>
      <p class="v-mini">También desde el Explorador: clic derecho en el archivo o la carpeta → «Versiones anteriores con Resguardo».</p>
    </form>

    {#if ocupado && !versiones}<p class="v-sub cargando" role="status"><LoaderCircle size={14} class="spin" aria-hidden="true" />Buscando sus versiones…</p>{/if}

    {#if versiones && !hecho}
      {#if !versiones.length}
        <p class="v-sub">No hay versiones anteriores de esa ruta en las copias.</p>
      {:else}
        <fieldset class="v-tarjeta lista">
          <legend class="v-mini">¿De qué momento? ({versiones.length})</legend>
          {#each versiones as v (v.version)}
            <label class="ver" class:on={elegida === v.version}>
              <input type="radio" name="version" value={v.version} bind:group={elegida} />
              {#if v.carpeta}<Folder size={15} aria-hidden="true" />{:else}<File size={15} aria-hidden="true" />{/if}
              <span class="cuando">{fecha(v.cuando)}<span class="v-mini"> · {relativo(v.cuando)}</span></span>
              {#if !v.carpeta}<span class="v-mini">{tamano(v.bytes)}</span>{/if}
            </label>
          {/each}
        </fieldset>

        <fieldset class="v-tarjeta motivo">
          <legend class="v-mini">¿Por qué? {estado?.motivo_obligatorio ? "(un toque basta)" : "(opcional)"}</legend>
          <div class="chips" role="radiogroup" aria-label="Motivo">
            {#each MOTIVOS as m (m.id)}
              <label class="chip" class:on={motivo === m.id}><input type="radio" name="motivo" value={m.id} bind:group={motivo} />{m.texto}</label>
            {/each}
          </div>
          {#if motivo}<input class="input" bind:value={notas} maxlength={500} placeholder="Notas (opcional). Sin contraseñas." aria-label="Notas" />{/if}
          <label class="otra"><input type="checkbox" bind:checked={otraCarpeta} />Dejarlo en otra carpeta mía</label>
          {#if otraCarpeta}<input class="input" bind:value={carpeta} spellcheck="false" placeholder="C:\Users\…\Recuperado" aria-label="Carpeta donde dejarlo" />{/if}
        </fieldset>

        <div class="pie">
          {#if falta}<span class="v-mini">{falta}</span>{/if}
          <button class="btn btn-primary" disabled={!!falta || ocupado} onclick={recuperar}>
            {#if ocupado}<LoaderCircle size={15} class="spin" aria-hidden="true" />Recuperando…{:else}<ArchiveRestore size={15} aria-hidden="true" />Recuperar{/if}
          </button>
        </div>
        <p class="v-mini">Hasta {estado?.max_mb ?? 1024} MB cada vez. Solo se recupera lo que tu cuenta puede abrir.</p>
      {/if}
    {/if}

    {#if hecho}
      <div class="v-tarjeta listo" role="status">
        <p class="v-ok">Listo: {hecho.archivos === 1 ? "1 archivo recuperado" : `${hecho.archivos} archivos recuperados`}.</p>
        <p class="v-mini donde">{hecho.donde}</p>
        {#if hecho.sin_permiso}<p class="v-mini">Sin {hecho.sin_permiso === 1 ? "1 cosa" : `${hecho.sin_permiso} cosas`} que tu cuenta no puede abrir.</p>{/if}
        <div class="pie">
          <button class="btn btn-sm" onclick={() => pedir("abrir_carpeta", { ruta: hecho!.donde })}><FolderOpen size={14} aria-hidden="true" />Abrir la carpeta</button>
          <button class="btn btn-sm btn-ghost" onclick={() => ((hecho = null), (versiones = null), (motivo = ""), (notas = ""))}>Recuperar otra cosa</button>
        </div>
      </div>
    {/if}
  {/if}
  {#if error}<p class="v-error" role="alert">{error}</p>{/if}
</div>

<style>
  .recuperar {
    display: grid;
    gap: var(--sp-3);
  }
  .cab {
    display: flex;
    gap: var(--sp-3);
    align-items: flex-start;
    padding: var(--sp-3);
  }
  .titulo {
    margin: 0;
    font-weight: 600;
  }
  .campo,
  .lista,
  .motivo,
  .listo {
    display: grid;
    gap: var(--sp-2);
    margin: 0;
    padding: var(--sp-3);
    min-width: 0;
  }
  .lista,
  .motivo {
    border: 1px solid var(--border);
  }
  .fila {
    display: flex;
    gap: var(--sp-2);
  }
  .fila .input {
    flex: 1;
    min-width: 0;
  }
  .ver {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    padding: 6px;
    border-radius: 8px;
    cursor: pointer;
  }
  .ver.on {
    background: var(--accent-soft);
  }
  .cuando {
    flex: 1;
    min-width: 0;
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .chip {
    display: inline-flex;
    padding: 4px 10px;
    border: 1px solid var(--border-strong);
    border-radius: 999px;
    font-size: var(--fs-sm);
    cursor: pointer;
  }
  .chip input {
    position: absolute;
    opacity: 0;
    pointer-events: none;
  }
  .chip.on {
    border-color: var(--accent);
    background: var(--accent-soft);
    color: var(--accent-text);
  }
  .chip:focus-within {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }
  .otra {
    display: flex;
    gap: 6px;
    align-items: center;
    font-size: var(--fs-sm);
  }
  .pie {
    display: flex;
    flex-wrap: wrap;
    gap: var(--sp-2);
    align-items: center;
    justify-content: flex-end;
  }
  .pie .v-mini {
    margin-right: auto;
  }
  .donde {
    overflow-wrap: anywhere;
  }
  .cargando {
    display: flex;
    gap: 6px;
    align-items: center;
  }
  .aviso {
    padding: var(--sp-3);
  }
</style>
