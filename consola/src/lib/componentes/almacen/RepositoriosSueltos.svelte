<script lang="ts">
  // 0.8.1 (D6, 15gg): «Repositorios sueltos» en la ficha del almacén: los que hay en su
  // disco fuera de la carpeta de cualquier equipo (p. ej. los de un rest-server viejo), sin
  // abrirlos. «Dar a un equipo»: el almacén lo mueve a la carpeta de ese equipo (mismo disco,
  // sin copiar) y después el equipo lo adopta con su contraseña («Usar un repositorio que ya
  // existe», que se abre ya puesto). Todo con la clave de administración.
  import { ArrowRight, Check, Database, FolderInput, FolderSearch, TriangleAlert } from "@lucide/svelte";
  import { goto } from "$app/navigation";
  import Modal from "$ui/componentes/Modal.svelte";
  import { pedirAlEquipo } from "$lib/pedirAlEquipo";
  import { avisar } from "$lib/avisos.svelte";
  import { cargarCliente } from "$lib/estado.svelte";
  import { ErrorLlavesCambiadas } from "$lib/ordenar";
  import { plural, relativo } from "$lib/formato";
  import { zonasDe, nombreZonaPorDefecto } from "$lib/destinos";
  import { ADMITE_SUELTOS, carpetaPropuesta, cuerpoDar, equiposConAcceso, sueltosDe, type Suelto } from "$lib/sueltos";
  import { carpetaValida } from "$lib/nombres";
  import type { Cliente, Equipo } from "$lib/tipos";
  import BotonFalta from "../BotonFalta.svelte";
  import CampoClave from "../CampoClave.svelte";
  import AlertaLlaves from "../AlertaLlaves.svelte";

  let { cliente, almacen, equipos, administra, ahora }: { cliente: Cliente; almacen: Equipo; equipos: Equipo[]; administra: boolean; ahora: number } = $props();

  const admite = $derived(!!almacen.resumen?.admite?.includes(ADMITE_SUELTOS));
  const sueltos = $derived(sueltosDe(almacen));
  const nombreZona = (z: string | null | undefined) => {
    if (!z) return null;
    const x = zonasDe(almacen).find((y) => y.id === z);
    return x ? nombreZonaPorDefecto(x) : z;
  };

  // --- Dar a un equipo ---
  let dar = $state<Suelto | null>(null);
  let paraQuien = $state("");
  let carpeta = $state("");
  let clave = $state("");
  let ocupado = $state(false);
  let error = $state("");
  let llaves = $state(false);
  let hecho = $state<{ equipo: Equipo; destino: string; carpeta: string; mensaje: string } | null>(null);
  const conAcceso = $derived(dar ? equiposConAcceso(almacen, equipos, dar.zona) : []);
  const elegido = $derived(conAcceso.find((x) => x.equipo.id === paraQuien));
  /** Las carpetas que ya tiene ese equipo en esa zona (para no pisar ninguna). */
  const ocupadas = $derived.by(() => {
    if (!elegido || !dar) return [];
    const g = almacen.resumen?.guarda_copias;
    const lista = dar.zona ? g?.zonas?.find((z) => z.id === dar!.zona)?.repositorios : g?.repositorios;
    return (lista?.find((u) => u.usuario === elegido.usuario)?.repos ?? []).map((r) => r.split("/")[0]);
  });
  function abrir(s: Suelto) {
    dar = s;
    hecho = null;
    error = "";
    clave = "";
    paraQuien = equiposConAcceso(almacen, equipos, s.zona)[0]?.equipo.id ?? "";
    carpeta = carpetaPropuesta(s.ruta, []);
  }
  $effect(() => {
    // Al cambiar de equipo, una carpeta que no tenga ya.
    const o = ocupadas;
    if (dar && o.includes(carpeta)) carpeta = carpetaPropuesta(dar.ruta, o);
  });
  const falta = $derived(
    !elegido
      ? { texto: "Elige el equipo" }
      : !carpetaValida(carpeta)
        ? { texto: "La carpeta: minúsculas, cifras y guiones", campo: "ds-carpeta" }
        : ocupadas.includes(carpeta)
          ? { texto: "Ya tiene una carpeta con ese nombre", campo: "ds-carpeta" }
          : !clave
            ? { texto: "Falta la clave de administración", campo: "ds-clave" }
            : null,
  );
  async function enviar(e: SubmitEvent) {
    e.preventDefault();
    if (falta || !dar || !elegido || ocupado) return;
    ocupado = true;
    error = "";
    try {
      const r = await pedirAlEquipo(cliente, almacen, "guarda_copias", cuerpoDar($state.snapshot(dar) as Suelto, elegido.usuario, carpeta), 30, { claveAdmin: clave });
      clave = "";
      if (!r.hecha) {
        avisar(r.texto);
        dar = null;
        return;
      }
      hecho = { equipo: elegido.equipo, destino: elegido.destino, carpeta, mensaje: r.texto };
      void cargarCliente(cliente.id, { silencioso: true });
    } catch (err) {
      if (err instanceof ErrorLlavesCambiadas) llaves = true;
      error = (err as Error).message;
    } finally {
      ocupado = false;
    }
  }
  function adoptar() {
    if (!hecho) return;
    const h = hecho;
    dar = null;
    void goto(`/c/${cliente.id}/equipos/${h.equipo.id}?${new URLSearchParams({ adoptar: "1", donde: h.destino, ruta: h.carpeta })}`);
  }

  // --- Buscar también fuera de la carpeta del almacén ---
  let buscar = $state(false);
  let rutas = $state("");
  let claveB = $state("");
  let errorB = $state("");
  function abrirBuscar() {
    rutas = (almacen.resumen?.guarda_copias?.sueltos_en ?? []).join("\n");
    claveB = errorB = "";
    buscar = true;
  }
  async function guardarBusqueda(e: SubmitEvent) {
    e.preventDefault();
    if (!claveB || ocupado) return;
    ocupado = true;
    errorB = "";
    try {
      const lista = rutas.split(/\r?\n/).map((x) => x.trim()).filter(Boolean);
      const r = await pedirAlEquipo(cliente, almacen, "guarda_copias", { sueltos_en: lista }, 20, { claveAdmin: claveB });
      claveB = "";
      avisar(r.texto);
      buscar = false;
      void cargarCliente(cliente.id, { silencioso: true });
    } catch (err) {
      errorB = (err as Error).message;
    } finally {
      ocupado = false;
    }
  }
</script>

{#if !admite}
  <p class="faint vacio-sub">Actualiza el agente de {almacen.nombre} para ver aquí los repositorios de su disco que no son de ningún equipo (p. ej. los de un servidor de copias antiguo) y dárselos a uno sin usar la terminal.</p>
{:else if !sueltos.length}
  <p class="faint vacio-sub">Ninguno: todos los repositorios de su disco están en la carpeta de algún equipo.{#if administra}{" "}<button type="button" class="link-btn" onclick={abrirBuscar}>Buscar también en otras carpetas…</button>{/if}</p>
{:else}
  <ul class="sueltos">
    {#each sueltos as s (s.zona + s.ruta)}
      <li>
        <span class="txt">
          <strong class="mono">{s.ruta}</strong>
          <span class="faint">{plural(s.versiones, "versión", "versiones")}{#if s.ultima}{" · "}la última {relativo(s.ultima, ahora)}{/if}{#if nombreZona(s.zona)}{" · "}{nombreZona(s.zona)}{/if}{#if s.fuera}{" · "}fuera de la carpeta del almacén{/if}</span>
        </span>
        {#if administra}
          {#if s.fuera}
            <span class="faint nota-fuera">Muévela a mano a la carpeta de un equipo para dársela.</span>
          {:else}
            <button type="button" class="btn btn-sm" onclick={() => abrir(s)}><FolderInput size={14} />Dar a un equipo</button>
          {/if}
        {/if}
      </li>
    {/each}
  </ul>
  {#if administra}<button type="button" class="link-btn" onclick={abrirBuscar}><FolderSearch size={13} />Buscar también en otras carpetas…</button>{/if}
{/if}

{#if dar}
  <Modal labelledby="t-dar-suelto" onclose={() => (dar = null)} width={520}>
    <div class="dlg-title">
      <span class="ticon"><Database size={18} /></span>
      <div>
        <h2 id="t-dar-suelto">Dar «{dar.ruta}» a un equipo</h2>
        <p>{almacen.nombre} lo mueve a la carpeta del equipo, en el mismo disco y sin copiar nada. Después, el equipo lo adopta con su contraseña y sigue con todo su historial.</p>
      </div>
    </div>
    {#if llaves}
      <AlertaLlaves equipo={almacen} cliente={cliente.id} />
    {:else if hecho}
      <div class="notice notice-success" role="status"><Check size={16} /><p>{hecho.mensaje}</p></div>
      <p class="faint">Falta el último paso: que {hecho.equipo.nombre} lo adopte con la contraseña del repositorio (la del servidor de copias antiguo o la de su kit).</p>
      <footer>
        <button type="button" class="btn btn-ghost" onclick={() => (dar = null)}>Más tarde</button>
        <button type="button" class="btn btn-primary" onclick={adoptar}>Que {hecho.equipo.nombre} lo adopte<ArrowRight size={14} /></button>
      </footer>
    {:else}
      <form class="form" onsubmit={enviar}>
        <fieldset class="equipos">
          <legend class="field-label">A qué equipo</legend>
          {#each conAcceso as x (x.equipo.id)}
            <label class="opcion" class:on={paraQuien === x.equipo.id}><input type="radio" name="ds-equipo" value={x.equipo.id} bind:group={paraQuien} /><span>{x.equipo.nombre}<span class="faint mono">{x.usuario}/</span></span></label>
          {:else}
            <p class="faint">Ningún equipo tiene aún acceso a {nombreZona(dar.zona) ?? "este almacén"}.</p>
          {/each}
          <p class="faint nota">¿Falta el equipo? En su ficha, «Usar uno que ya existe → Dar acceso a … en este almacén».</p>
        </fieldset>
        <div class="field">
          <label class="field-label" for="ds-carpeta">Nombre de la carpeta en la del equipo</label>
          <input id="ds-carpeta" class="input mono" bind:value={carpeta} maxlength="64" spellcheck="false" autocomplete="off" />
          {#if elegido}<span class="field-hint">Quedará en <span class="mono">{elegido.usuario}/{carpeta || "…"}</span>.</span>{/if}
        </div>
        <CampoClave id="ds-clave" etiqueta="Clave de administración" bind:value={clave} requerido />
        {#if error}<div class="notice notice-danger" role="alert"><TriangleAlert size={16} /><p>{error}</p></div>{/if}
        <footer>
          <button type="button" class="btn btn-ghost" disabled={ocupado} onclick={() => (dar = null)}>Cancelar</button>
          <BotonFalta disabled={ocupado} {falta}>{ocupado ? "Moviendo…" : "Moverlo a su carpeta"}</BotonFalta>
        </footer>
      </form>
    {/if}
  </Modal>
{/if}

{#if buscar}
  <Modal labelledby="t-buscar-sueltos" onclose={() => (buscar = false)} width={520}>
    <form class="form" onsubmit={guardarBusqueda}>
      <div class="dlg-title">
        <span class="ticon"><FolderSearch size={18} /></span>
        <div>
          <h2 id="t-buscar-sueltos">Buscar repositorios en otras carpetas</h2>
          <p>Por ejemplo, la de un servidor de copias antiguo de {almacen.nombre}. Lo que encuentre fuera de la carpeta del almacén solo se enseña: para dárselo a un equipo hay que moverlo antes a mano.</p>
        </div>
      </div>
      <div class="field">
        <label class="field-label" for="bs-rutas">Carpetas de {almacen.nombre} <span class="faint">(una por línea; vacío: ninguna)</span></label>
        <textarea id="bs-rutas" class="input mono" rows="3" bind:value={rutas} spellcheck="false" placeholder={/windows/i.test(almacen.so) ? "D:\\RestServerViejo" : "/srv/rest-viejo"}></textarea>
      </div>
      <CampoClave id="bs-clave" etiqueta="Clave de administración" bind:value={claveB} requerido error={errorB} />
      <footer>
        <button type="button" class="btn btn-ghost" disabled={ocupado} onclick={() => (buscar = false)}>Cancelar</button>
        <BotonFalta disabled={ocupado} falta={claveB ? null : { texto: "Falta la clave de administración", campo: "bs-clave" }}>{ocupado ? "Guardando…" : "Guardar"}</BotonFalta>
      </footer>
    </form>
  </Modal>
{/if}

<style>
  .sueltos {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin: 0 0 var(--sp-2);
    padding: 0;
    list-style: none;
  }
  .sueltos li {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: 6px 12px;
    padding: 8px 10px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    font-size: var(--fs-sm);
  }
  .txt {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
    overflow-wrap: anywhere;
  }
  .mono {
    font-family: var(--mono);
    font-size: 12.5px;
  }
  .nota-fuera,
  .nota {
    margin: 0;
    font-size: var(--fs-xs);
  }
  .link-btn {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 0;
    border: none;
    background: none;
    font: inherit;
    font-size: var(--fs-xs);
    color: var(--accent-text);
    cursor: pointer;
  }
  .link-btn:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }
  .equipos {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin: 0;
    padding: 0;
    border: none;
  }
  .opcion {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 10px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    font-size: var(--fs-sm);
    cursor: pointer;
  }
  .opcion.on {
    border-color: var(--accent);
    background: var(--surface-2);
  }
  .opcion span {
    display: flex;
    flex-wrap: wrap;
    gap: 2px 8px;
  }
</style>
