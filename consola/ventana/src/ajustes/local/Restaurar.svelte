<script lang="ts">
  // Restaurar (modo local): elegir el repositorio y la versión, recorrerla,
  // ver «Qué cambió» respecto a la anterior y restaurar archivos o carpetas
  // junto al original, en su sitio o en otra carpeta.
  import { untrack } from "svelte";
  import { ArrowLeft, ChevronRight, File, Folder, GitCompare, HardDriveDownload, LoaderCircle } from "@lucide/svelte";
  import { bytes, fechaLarga, numero, relativo } from "$lib/formato";
  import { esBloqueo, servicio } from "../../puente.svelte";
  import type { PropsParte } from "./comun";
  import ElegirCarpeta from "./ElegirCarpeta.svelte";

  let { estado, alBloquear }: PropsParte = $props();
  interface Version {
    id: string;
    cuando: string;
    etiquetas?: string[];
    archivos?: number | null;
    bytes?: number | null;
  }
  interface Entrada {
    nombre: string;
    tipo: "dir" | "archivo";
    bytes?: number | null;
    modificado?: string | null;
  }
  interface Cambios {
    primera?: boolean;
    resumen?: { nuevos: number; cambiados: number; borrados: number };
    cambios?: { ruta: string; tipo: string; bytes?: number }[];
    total?: number;
    siguiente?: number | null;
  }
  let repo = $state(untrack(() => estado.repositorios[0]?.id ?? ""));
  let versiones = $state<Version[] | null>(null);
  let version = $state<Version | null>(null);
  let ruta = $state("/");
  let entradas = $state<Entrada[]>([]);
  let elegidas = $state<string[]>([]);
  let cambios = $state<Cambios | null>(null);
  let destino = $state<"junto" | "original" | "carpeta">("junto");
  let reemplazar = $state(false);
  let carpeta = $state("");
  let elegir = $state(false);
  let cargando = $state(false);
  let error = $state("");
  let hecho = $state("");

  async function op<T>(que: string, cuerpo: Record<string, unknown>): Promise<T | null> {
    cargando = true;
    error = "";
    try {
      return await servicio<T>(que, cuerpo);
    } catch (e) {
      if (esBloqueo(e)) alBloquear();
      error = (e as Error).message;
      return null;
    } finally {
      cargando = false;
    }
  }
  async function cargarVersiones() {
    version = null;
    versiones = null;
    const r = await op<{ versiones: Version[] }>("explorar", { repo, que: "versiones", p: {} });
    versiones = r?.versiones ?? [];
  }
  async function abrir(v: Version) {
    version = v;
    cambios = null;
    elegidas = [];
    hecho = "";
    await ir("/");
  }
  async function ir(r: string) {
    if (!version) return;
    const x = await op<{ entradas: Entrada[] }>("explorar", { repo, que: "listar", p: { version: version.id, ruta: r } });
    if (x) {
      ruta = r;
      entradas = x.entradas.sort((a, b) => (a.tipo === b.tipo ? a.nombre.localeCompare(b.nombre) : a.tipo === "dir" ? -1 : 1));
    }
  }
  const unir = (r: string, n: string) => (r.endsWith("/") ? r + n : `${r}/${n}`);
  const padre = (r: string) => r.replace(/\/[^/]*\/?$/, "") || "/";
  /** «/C/Users/Ana» → «C:\Users\Ana» (cómo lo ve el usuario en Windows). */
  const legible = (r: string) => (/^\/[A-Za-z](\/|$)/.test(r) ? `${r[1]}:\\${r.slice(3).replaceAll("/", "\\")}` : r);
  function alternar(r: string) {
    elegidas = elegidas.includes(r) ? elegidas.filter((x) => x !== r) : [...elegidas, r];
  }
  async function verCambios() {
    if (!version) return;
    cambios = await op<Cambios>("explorar", { repo, que: "diferencias", p: { hasta: version.id } });
  }
  async function restaurar() {
    if (!version || !elegidas.length) return;
    hecho = "";
    const r = await op<{ mensaje: string }>("restaurar", { repo, version: version.id, rutas: elegidas, destino, carpeta, reemplazar });
    if (r) {
      hecho = r.mensaje;
      elegidas = [];
    }
  }
  $effect(() => {
    void repo;
    if (repo) void cargarVersiones();
  });
  const TIPO: Record<string, string> = { nuevo: "nuevo", cambiado: "cambiado", borrado: "borrado", metadatos: "permisos o fechas", otro: "otro" };
</script>

{#if elegir}
  <ElegirCarpeta titulo="Restaurar en…" crear alCerrar={() => (elegir = false)} alElegir={(rs) => ((carpeta = rs[0]), (elegir = false))} />
{/if}

<div class="v-pila">
  {#if !estado.repositorios.length}
    <div class="v-tarjeta"><p class="v-sub">Aún no hay copias que restaurar.</p></div>
  {:else}
    <label class="field">
      <span class="field-label">De</span>
      <select class="input" bind:value={repo}>
        {#each estado.repositorios as r (r.id)}<option value={r.id}>{r.nombre}</option>{/each}
      </select>
    </label>

    {#if !version}
      {#if versiones === null}<p class="v-sub"><LoaderCircle size={14} class="spin" /> Leyendo las versiones…</p>{/if}
      <ul class="lista">
        {#each versiones ?? [] as v (v.id)}
          <li>
            <button class="v-tarjeta fila" onclick={() => abrir(v)}>
              <span class="v-cortar"><b>{fechaLarga(v.cuando)}</b> <span class="v-mini">({relativo(v.cuando)})</span></span>
              <span class="v-mini">{v.archivos ? `${numero(v.archivos)} archivos` : ""}{v.bytes ? ` · ${bytes(v.bytes)}` : ""}</span>
              <ChevronRight size={15} aria-hidden="true" />
            </button>
          </li>
        {:else}
          {#if versiones}<li class="v-sub">Este repositorio aún no tiene versiones.</li>{/if}
        {/each}
      </ul>
    {:else}
      <div class="v-fila">
        <button class="btn btn-ghost btn-sm" onclick={() => (version = null)}><ArrowLeft size={14} />Versiones</button>
        <span class="v-sub v-cortar">{fechaLarga(version.cuando)}</span>
        <button class="btn btn-sm cambios" onclick={verCambios} disabled={cargando}><GitCompare size={14} aria-hidden="true" />Qué cambió</button>
      </div>

      {#if cambios}
        <section class="v-tarjeta v-pila">
          {#if cambios.primera}
            <p class="v-sub">Es la primera versión de esta copia: todo lo que tiene es nuevo.</p>
          {:else}
            <p class="v-sub">
              Respecto a la anterior: {numero(cambios.resumen?.nuevos ?? 0)} nuevos, {numero(cambios.resumen?.cambiados ?? 0)} cambiados y {numero(cambios.resumen?.borrados ?? 0)} borrados.
            </p>
            <ul class="cambios-lista">
              {#each cambios.cambios ?? [] as c (c.ruta)}
                <li><span class="badge badge-sm" class:tone-ok={c.tipo === "nuevo"} class:tone-warn={c.tipo === "cambiado"} class:tone-danger={c.tipo === "borrado"}>{TIPO[c.tipo] ?? c.tipo}</span><span class="mono v-cortar">{legible(c.ruta)}</span></li>
              {/each}
            </ul>
            {#if (cambios.total ?? 0) > (cambios.cambios?.length ?? 0)}<p class="v-mini">Y {numero((cambios.total ?? 0) - (cambios.cambios?.length ?? 0))} más.</p>{/if}
          {/if}
        </section>
      {/if}

      <nav class="migas v-mini" aria-label="Carpeta de la versión">
        <button class="link" onclick={() => ir("/")}>Inicio</button>
        {#each ruta.split("/").filter(Boolean) as parte, i (i)}
          <ChevronRight size={12} aria-hidden="true" /><button class="link" onclick={() => ir("/" + ruta.split("/").filter(Boolean).slice(0, i + 1).join("/"))}>{i === 0 && parte.length === 1 ? `${parte}:` : parte}</button>
        {/each}
      </nav>
      <ul class="explorador" aria-busy={cargando}>
        {#if ruta !== "/"}<li><button class="entrada" onclick={() => ir(padre(ruta))}><ArrowLeft size={15} />..</button></li>{/if}
        {#each entradas as e (e.nombre)}
          {@const r = unir(ruta, e.nombre)}
          <li class="v-fila">
            <input type="checkbox" checked={elegidas.includes(r)} onchange={() => alternar(r)} aria-label={`Elegir ${e.nombre}`} />
            {#if e.tipo === "dir"}
              <button class="entrada" onclick={() => ir(r)}><Folder size={15} aria-hidden="true" /><span class="v-cortar">{ruta === "/" && e.nombre.length === 1 ? `${e.nombre}:` : e.nombre}</span></button>
            {:else}
              <span class="entrada"><File size={15} aria-hidden="true" /><span class="v-cortar">{e.nombre}</span><span class="v-mini tam">{bytes(e.bytes)}</span></span>
            {/if}
          </li>
        {/each}
      </ul>

      {#if elegidas.length}
        <section class="v-tarjeta v-pila">
          <h3 class="v-titulo">Restaurar {elegidas.length === 1 ? `«${elegidas[0].split("/").pop()}»` : `${elegidas.length} elementos`}</h3>
          <label class="radio"><input type="radio" bind:group={destino} value="junto" /> Junto al original (en una carpeta «Restaurado …»)</label>
          <label class="radio"><input type="radio" bind:group={destino} value="original" /> En su sitio</label>
          {#if destino === "original"}
            <label class="switch-row sangria"><input type="checkbox" class="switch" bind:checked={reemplazar} /><span>Reemplazar lo que haya (si no, solo lo que falte)</span></label>
          {/if}
          <label class="radio"><input type="radio" bind:group={destino} value="carpeta" /> En otra carpeta</label>
          {#if destino === "carpeta"}
            <div class="v-fila sangria"><input class="input mono" bind:value={carpeta} placeholder="D:\Recuperado" aria-label="Carpeta" /><button class="btn btn-sm" onclick={() => (elegir = true)}>Elegir</button></div>
          {/if}
          <button class="btn btn-primary" disabled={cargando || (destino === "carpeta" && !carpeta)} onclick={restaurar}>
            <HardDriveDownload size={15} aria-hidden="true" />{cargando ? "Restaurando…" : "Restaurar"}
          </button>
        </section>
      {/if}
    {/if}
  {/if}
  {#if hecho}<p class="v-ok" role="status">{hecho}</p>{/if}
  {#if error}<p class="v-error" role="alert">{error}</p>{/if}
</div>

<style>
  .lista,
  .explorador,
  .cambios-lista {
    display: grid;
    gap: 4px;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .fila {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    width: 100%;
    padding: var(--sp-3);
    color: var(--text-1);
    font: inherit;
    text-align: left;
    cursor: pointer;
  }
  .fila > span:first-child {
    flex: 1;
  }
  .cambios {
    margin-left: auto;
  }
  .migas {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 2px;
  }
  .explorador {
    max-height: 320px;
    overflow-y: auto;
    padding: 4px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
  }
  .entrada {
    display: flex;
    flex: 1;
    align-items: center;
    gap: var(--sp-2);
    min-width: 0;
    padding: 4px 6px;
    border: 0;
    border-radius: var(--radius-sm);
    background: none;
    color: var(--text-1);
    font: inherit;
    text-align: left;
  }
  button.entrada {
    cursor: pointer;
  }
  button.entrada:hover {
    background: var(--surface-2);
  }
  .tam {
    margin-left: auto;
    flex: none;
  }
  .cambios-lista li {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    font-size: var(--fs-sm);
  }
  .sangria {
    margin-left: 24px;
  }
</style>
