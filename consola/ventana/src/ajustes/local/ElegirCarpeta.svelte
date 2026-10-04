<script lang="ts">
  // Elegir una carpeta de este equipo (la lista la da el servicio, con la
  // clave: ve las carpetas como las verá al copiar). Con sugerencias (las
  // carpetas de los usuarios) y «Nueva carpeta» si se pide.
  import Modal from "$ui/componentes/Modal.svelte";
  import { ChevronRight, Folder, FolderPlus, HardDrive, Sparkles } from "@lucide/svelte";
  import { onMount } from "svelte";
  import { servicio } from "../../puente.svelte";

  let {
    titulo = "Elegir una carpeta",
    alElegir,
    alCerrar,
    crear = false,
  }: { titulo?: string; alElegir: (rutas: string[]) => void; alCerrar: () => void; crear?: boolean } = $props();

  interface Entrada {
    nombre: string;
    tipo: "dir" | "archivo";
    sistema?: boolean;
  }
  let ruta = $state("");
  let entradas = $state<Entrada[]>([]);
  let sugerencias = $state<{ id: string; nombre: string; rutas: string[] }[]>([]);
  let error = $state("");
  let cargando = $state(false);
  let nueva = $state("");
  const id = $props.id();

  const sep = (r: string) => (r.includes("\\") || /^[A-Za-z]:/.test(r) ? "\\" : "/");
  const unir = (r: string, n: string) => (!r ? n : r.endsWith("\\") || r.endsWith("/") ? r + n : r + sep(r) + n);
  function padre(r: string): string {
    if (!r || /^[A-Za-z]:\\?$/.test(r) || r === "/") return "";
    const s = sep(r);
    const i = r.replace(/[\\/]+$/, "").lastIndexOf(s);
    if (i < 0) return "";
    const p = r.slice(0, i);
    return /^[A-Za-z]:$/.test(p) ? p + "\\" : p || "/";
  }

  async function ir(r: string) {
    cargando = true;
    error = "";
    try {
      const x = await servicio<{ entradas: Entrada[] }>("carpetas", { que: "carpetas", p: { ruta: r } });
      entradas = x.entradas.filter((e) => e.tipo === "dir");
      ruta = r;
    } catch (e) {
      error = (e as Error).message;
    } finally {
      cargando = false;
    }
  }

  async function crearCarpeta() {
    try {
      const x = await servicio<{ ruta: string }>("carpetas", { que: "crear_carpeta", p: { ruta, nombre: nueva.trim() } });
      nueva = "";
      await ir(x.ruta);
    } catch (e) {
      error = (e as Error).message;
    }
  }

  onMount(async () => {
    void ir("");
    try {
      sugerencias = (await servicio<{ sugerencias: typeof sugerencias }>("carpetas", { que: "sugerencias", p: {} })).sugerencias;
    } catch {
      /* sin sugerencias */
    }
  });
  const migas = $derived.by(() => {
    const out: { texto: string; ruta: string }[] = [];
    let r = ruta;
    while (r) {
      out.unshift({ texto: r.replace(/[\\/]+$/, "").split(/[\\/]/).pop() || r, ruta: r });
      r = padre(r);
    }
    return out;
  });
</script>

<Modal onclose={alCerrar} labelledby={`${id}-t`} width={420}>
  <div class="dlg-head"><h2 class="dlg-title" id={`${id}-t`}>{titulo}</h2></div>
  <div class="v-pila cuerpo">
    {#if sugerencias.length && !ruta}
      <div class="sug">
        {#each sugerencias as s (s.id)}
          <button class="btn btn-sm" onclick={() => alElegir(s.rutas)}><Sparkles size={14} aria-hidden="true" />{s.nombre}</button>
        {/each}
      </div>
    {/if}
    <nav class="migas v-mini" aria-label="Carpeta actual">
      <button class="link" onclick={() => ir("")}>Este equipo</button>
      {#each migas as m (m.ruta)}<ChevronRight size={12} aria-hidden="true" /><button class="link" onclick={() => ir(m.ruta)}>{m.texto}</button>{/each}
    </nav>
    <ul class="lista" aria-busy={cargando}>
      {#each entradas as e (e.nombre)}
        <li>
          <button class="fila" class:sistema={e.sistema} onclick={() => ir(unir(ruta, e.nombre))}>
            {#if !ruta}<HardDrive size={16} aria-hidden="true" />{:else}<Folder size={16} aria-hidden="true" />{/if}
            <span class="v-cortar">{e.nombre}</span>
          </button>
        </li>
      {:else}
        <li class="v-mini vacia">{cargando ? "Leyendo…" : "Sin carpetas dentro."}</li>
      {/each}
    </ul>
    {#if crear && ruta}
      <div class="v-fila">
        <input class="input" placeholder="Nueva carpeta aquí" bind:value={nueva} aria-label="Nombre de la carpeta nueva" />
        <button class="btn btn-sm" disabled={!nueva.trim()} onclick={crearCarpeta}><FolderPlus size={14} aria-hidden="true" />Crear</button>
      </div>
    {/if}
    {#if error}<p class="v-error" role="alert">{error}</p>{/if}
    <div class="v-fila acciones">
      <button class="btn btn-ghost" onclick={alCerrar}>Cancelar</button>
      <button class="btn btn-primary" disabled={!ruta} onclick={() => alElegir([ruta])}>Elegir {migas.at(-1)?.texto ? `«${migas.at(-1)?.texto}»` : "esta carpeta"}</button>
    </div>
  </div>
</Modal>

<style>
  .cuerpo {
    padding: 0 var(--sp-5) var(--sp-5);
  }
  .sug {
    display: flex;
    flex-wrap: wrap;
    gap: var(--sp-2);
  }
  .migas {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 2px;
  }
  .lista {
    max-height: 280px;
    overflow-y: auto;
    margin: 0;
    padding: 0;
    list-style: none;
    border: 1px solid var(--border);
    border-radius: var(--radius);
  }
  .fila {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    width: 100%;
    padding: 8px 10px;
    border: 0;
    background: none;
    color: var(--text-1);
    font: inherit;
    text-align: left;
    cursor: pointer;
  }
  .fila:hover {
    background: var(--surface-2);
  }
  .fila.sistema {
    color: var(--text-3);
  }
  .vacia {
    padding: 10px;
  }
  .acciones {
    justify-content: flex-end;
  }
</style>
