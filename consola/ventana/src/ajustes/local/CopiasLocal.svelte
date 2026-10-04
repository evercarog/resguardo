<script lang="ts">
  // Las copias del equipo (modo local): qué carpetas, sin qué, dónde, cuándo
  // (las reglas del horario de la consola: horas, cada N minutos, cada N días,
  // un día al mes), «solo si hay cambios», «Antes de copiar» y la verificación
  // automática. Se guarda como la `config` de la consola (mismo validador).
  import { untrack } from "svelte";
  import { CalendarClock, FolderOpen, Pencil, Plus, Trash2, X } from "@lucide/svelte";
  import EditorHorario from "$lib/componentes/EditorHorario.svelte";
  import { resumenReglas } from "$lib/formato";
  import { errorRegla, reglasDe } from "$lib/horario";
  import { errorGancho, ganchosDe, MAX_GANCHOS, NOMBRE_GANCHO, paraConfig } from "$lib/ganchos";
  import { errorVerificacion, fraseVerificacion, PORCENTAJES, VERIFICACION_POR_DEFECTO } from "$lib/verificacion";
  import type { CopiaConfig, Gancho, VerificacionAuto } from "$lib/tipos";
  import { esBloqueo, servicio } from "../../puente.svelte";
  import { configParaEnviar, idDe, type ConfigLocal, type PropsParte } from "./comun";
  import ElegirCarpeta from "./ElegirCarpeta.svelte";

  let { estado, recargar, alBloquear }: PropsParte = $props();
  const repos = $derived(estado.repositorios);
  let editando = $state<CopiaConfig | null>(null);
  let ganchos = $state<Gancho[]>([]);
  let exclusiones = $state("");
  let elegir = $state<null | "carpetas" | number>(null);
  let guardando = $state(false);
  let error = $state("");
  let hecho = $state("");

  const nueva = (): CopiaConfig => ({
    id: "",
    nombre: "",
    repo: repos[0]?.id ?? "",
    carpetas: [],
    exclusiones: [],
    horario: { dias: [1, 2, 3, 4, 5, 6, 7], horas: ["13:00"] },
    activa: true,
    solo_si_cambios: true,
    gancho: null,
  });
  function editar(c: CopiaConfig | null) {
    const x: CopiaConfig = JSON.parse(JSON.stringify(c ?? nueva()));
    editando = x;
    ganchos = ganchosDe(x.gancho);
    exclusiones = x.exclusiones.join("\n");
    error = hecho = "";
  }

  async function guardar(copias: CopiaConfig[], verificaciones = untrack(() => estado.config.verificaciones)) {
    guardando = true;
    error = hecho = "";
    try {
      const config: ConfigLocal = configParaEnviar({ ...estado.config, copias, ...(verificaciones ? { verificaciones } : {}) });
      const r = await servicio<{ mensaje: string }>("config", { config });
      hecho = r.mensaje;
      editando = null;
      await recargar();
    } catch (e) {
      if (esBloqueo(e)) return alBloquear();
      error = (e as Error).message;
    } finally {
      guardando = false;
    }
  }

  const problema = $derived.by(() => {
    if (!editando) return null;
    if (!editando.nombre.trim()) return "Ponle un nombre a la copia.";
    if (!editando.repo) return "Elige dónde se guarda (crea antes un destino en «Dónde»).";
    if (!editando.carpetas.length) return "Elige al menos una carpeta.";
    for (const r of reglasDe(editando.horario, true)) {
      const e = errorRegla(r);
      if (e) return e;
    }
    for (const g of ganchos) {
      const e = errorGancho(g);
      if (e) return e;
    }
    return null;
  });

  function aceptar() {
    if (!editando || problema) return;
    const c: CopiaConfig = {
      ...editando,
      id: editando.id || idDe(editando.nombre),
      nombre: editando.nombre.trim(),
      exclusiones: exclusiones
        .split("\n")
        .map((x) => x.trim())
        .filter(Boolean),
      gancho: paraConfig(ganchos),
    };
    const otras = estado.config.copias.filter((k) => k.id !== c.id);
    void guardar([...otras, c]);
  }
  function borrar(c: CopiaConfig) {
    if (!confirm(`¿Quitar la copia «${c.nombre}»? Lo ya copiado se queda en su destino.`)) return;
    void guardar(estado.config.copias.filter((k) => k.id !== c.id));
  }
  function activar(c: CopiaConfig, activa: boolean) {
    void guardar(estado.config.copias.map((k) => (k.id === c.id ? { ...k, activa } : k)));
  }

  // ---- Verificación automática (por repositorio) ----
  function verificacionDe(repo: string): VerificacionAuto | null {
    return estado.config.verificaciones?.[repo] ?? null;
  }
  function ponerVerificacion(repo: string, v: VerificacionAuto | null) {
    const mapa = { ...(estado.config.verificaciones ?? {}) };
    if (v) mapa[repo] = v;
    else delete mapa[repo];
    void guardar(estado.config.copias, mapa);
  }
  const nombreRepo = (id: string) => repos.find((r) => r.id === id)?.nombre ?? id;
</script>

{#if elegir !== null && editando}
  <ElegirCarpeta
    titulo={elegir === "carpetas" ? "Carpetas que se copian" : "Carpeta"}
    crear={elegir !== "carpetas"}
    alCerrar={() => (elegir = null)}
    alElegir={(rs) => {
      if (elegir === "carpetas" && editando) editando.carpetas = [...new Set([...editando.carpetas, ...rs])];
      else if (typeof elegir === "number") ganchos[elegir].carpeta = rs[0];
      elegir = null;
    }}
  />
{/if}

{#if editando}
  <form class="v-tarjeta v-pila" onsubmit={(e) => (e.preventDefault(), aceptar())}>
    <h3 class="v-titulo">{editando.id ? `Cambiar «${editando.nombre}»` : "Copia nueva"}</h3>
    <label class="field"><span class="field-label">Nombre</span><input class="input" bind:value={editando.nombre} maxlength="80" placeholder="Documentos" /></label>
    <label class="field">
      <span class="field-label">Se guarda en</span>
      <select class="input" bind:value={editando.repo}>
        {#each repos as r (r.id)}<option value={r.id}>{r.nombre}</option>{/each}
      </select>
    </label>
    <div class="field">
      <span class="field-label">Carpetas</span>
      <ul class="carpetas">
        {#each editando.carpetas as c, i (c)}
          <li><span class="mono v-cortar">{c}</span><button type="button" class="icon-btn" aria-label={`Quitar ${c}`} onclick={() => editando!.carpetas.splice(i, 1)}><X size={14} /></button></li>
        {/each}
      </ul>
      <button type="button" class="btn btn-sm" onclick={() => (elegir = "carpetas")}><FolderOpen size={14} aria-hidden="true" />Añadir carpetas</button>
    </div>
    <label class="field">
      <span class="field-label">Sin copiar (una por línea)</span>
      <textarea class="input mono" rows="3" bind:value={exclusiones} placeholder={"*.tmp\nC:\\Users\\Ana\\Downloads"}></textarea>
      <span class="field-hint">Patrones como «*.tmp» o carpetas completas.</span>
    </label>
    <div class="field">
      <span class="field-label"><CalendarClock size={14} aria-hidden="true" /> Cuándo</span>
      <EditorHorario id="horario-local" bind:horario={editando.horario} admiteReglas={true} version={window.__resguardoVersion ?? null} />
    </div>
    <label class="switch-row">
      <input type="checkbox" class="switch" bind:checked={editando.solo_si_cambios} />
      <span>Solo guardar una versión si hay cambios</span>
    </label>
    <label class="switch-row">
      <input type="checkbox" class="switch" bind:checked={editando.activa} />
      <span>Activa</span>
    </label>

    <div class="field">
      <span class="field-label">Antes de copiar</span>
      {#each ganchos as g, i (i)}
        <div class="gancho v-pila">
          <div class="v-fila"><b>{NOMBRE_GANCHO[g.tipo]}</b><button type="button" class="icon-btn quitar" aria-label="Quitar este paso" onclick={() => ganchos.splice(i, 1)}><Trash2 size={14} /></button></div>
          {#if g.tipo === "sqlserver"}
            <label class="field"><span class="field-hint">Instancia («.» la predeterminada, o EQUIPO\SQLEXPRESS)</span><input class="input mono" bind:value={g.instancia} placeholder="." /></label>
            <label class="field">
              <span class="field-hint">Bases de datos (separadas por comas)</span>
              <input class="input mono" value={g.bases.join(", ")} oninput={(e) => (g.bases = e.currentTarget.value.split(",").map((x) => x.trim()).filter(Boolean))} />
            </label>
            <div class="v-fila"><input class="input mono" bind:value={g.carpeta} placeholder="C:\ResguardoVolcados" aria-label="Carpeta de los volcados" /><button type="button" class="btn btn-sm" onclick={() => (elegir = i)}>Elegir</button></div>
          {:else}
            <div class="v-fila"><input class="input mono" bind:value={g.carpeta} placeholder="D:\Contabilidad\Copias" aria-label="Carpeta a vigilar" /><button type="button" class="btn btn-sm" onclick={() => (elegir = i)}>Elegir</button></div>
            <label class="v-fila"><span class="field-hint">Avisar si lo más nuevo tiene más de</span><input class="input horas" type="number" min="1" max="720" bind:value={g.horas} /><span class="field-hint">horas</span></label>
          {/if}
          {#if errorGancho(g)}<p class="v-mini aviso">{errorGancho(g)}</p>{/if}
        </div>
      {/each}
      {#if ganchos.length < MAX_GANCHOS}
        <div class="v-fila">
          <button type="button" class="btn btn-sm" onclick={() => ganchos.push({ tipo: "sqlserver", bases: [], carpeta: "C:\\ResguardoVolcados" })}><Plus size={14} />Volcar SQL Server</button>
          <button type="button" class="btn btn-sm" onclick={() => ganchos.push({ tipo: "carpeta_reciente", carpeta: "", horas: 26 })}><Plus size={14} />Vigilar copias de una aplicación</button>
        </div>
      {/if}
    </div>

    {#if problema}<p class="v-mini aviso">{problema}</p>{/if}
    {#if error}<p class="v-error" role="alert">{error}</p>{/if}
    <div class="v-fila fin">
      <button type="button" class="btn btn-ghost" onclick={() => (editando = null)}>Cancelar</button>
      <button class="btn btn-primary" disabled={!!problema || guardando}>{guardando ? "Guardando…" : "Guardar"}</button>
    </div>
  </form>
{:else}
  <div class="v-pila">
    {#if !repos.length}
      <div class="v-tarjeta"><p class="v-sub">Primero, dónde se guardan las copias: crea un destino en «Dónde».</p></div>
    {/if}
    {#each estado.config.copias as c (c.id)}
      <div class="v-tarjeta fila">
        <div class="texto">
          <p class="nombre v-cortar">{c.nombre}{#if !c.activa}<span class="badge badge-sm tone-muted">en pausa</span>{/if}</p>
          <p class="v-mini v-cortar">{c.carpetas.length === 1 ? c.carpetas[0] : `${c.carpetas.length} carpetas`} → {nombreRepo(c.repo)}</p>
          <p class="v-mini">{resumenReglas(reglasDe(c.horario, true))}{c.solo_si_cambios !== false ? " · solo si hay cambios" : ""}{ganchosDe(c.gancho).length ? " · con «Antes de copiar»" : ""}</p>
        </div>
        <input type="checkbox" class="switch" checked={c.activa} aria-label={`Activa «${c.nombre}»`} onchange={(e) => activar(c, e.currentTarget.checked)} />
        <button class="icon-btn" aria-label={`Cambiar «${c.nombre}»`} onclick={() => editar(c)}><Pencil size={15} /></button>
        <button class="icon-btn" aria-label={`Quitar «${c.nombre}»`} onclick={() => borrar(c)}><Trash2 size={15} /></button>
      </div>
    {/each}
    {#if repos.length}<button class="btn btn-primary" onclick={() => editar(null)}><Plus size={15} aria-hidden="true" />Nueva copia</button>{/if}

    {#if repos.length}
      <section class="v-tarjeta v-pila">
        <h3 class="v-titulo">Verificación automática</h3>
        <p class="v-mini">Comprueba que las copias se pueden leer: cada cierto tiempo, una parte de los datos (rotativa: al final se lee todo).</p>
        {#each repos as r (r.id)}
          {@const v = verificacionDe(r.id)}
          <div class="v-fila verif">
            <span class="v-cortar">{r.nombre}</span>
            {#if v}
              <select class="input corto" value={v.cada_dias} onchange={(e) => ponerVerificacion(r.id, { ...v, cada_dias: Number(e.currentTarget.value) })} aria-label="Cada cuántos días">
                {#each [1, 3, 7, 14, 30] as d (d)}<option value={d}>cada {d} {d === 1 ? "día" : "días"}</option>{/each}
              </select>
              <select class="input corto" value={v.porcentaje} onchange={(e) => ponerVerificacion(r.id, { ...v, porcentaje: Number(e.currentTarget.value) })} aria-label="Qué parte de los datos">
                {#each PORCENTAJES as p (p)}<option value={p}>{p === 0 ? "solo la estructura" : `${p} %`}</option>{/each}
              </select>
              <button class="icon-btn" aria-label="Quitar la verificación" onclick={() => ponerVerificacion(r.id, null)}><X size={14} /></button>
            {:else}
              <button class="btn btn-sm" onclick={() => ponerVerificacion(r.id, VERIFICACION_POR_DEFECTO)}>Activar</button>
            {/if}
          </div>
          {#if v}<p class="v-mini">{errorVerificacion(v) ?? fraseVerificacion(v)}</p>{/if}
        {/each}
      </section>
    {/if}
    {#if hecho}<p class="v-ok" role="status">{hecho}</p>{/if}
    {#if error}<p class="v-error" role="alert">{error}</p>{/if}
  </div>
{/if}

<style>
  .fila {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    padding: var(--sp-3);
  }
  .texto {
    flex: 1;
    min-width: 0;
  }
  .nombre {
    display: flex;
    gap: var(--sp-2);
    align-items: center;
    margin: 0;
    font-weight: 600;
  }
  .carpetas {
    display: grid;
    gap: 4px;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .carpetas li {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    padding: 4px 8px;
    border-radius: var(--radius-sm);
    background: var(--surface-2);
    font-size: var(--fs-sm);
  }
  .carpetas li span {
    flex: 1;
  }
  .gancho {
    padding: var(--sp-3);
    border: 1px solid var(--border);
    border-radius: var(--radius);
  }
  .quitar {
    margin-left: auto;
  }
  .horas {
    width: 80px;
  }
  .corto {
    width: auto;
  }
  .verif span {
    flex: 1;
  }
  .aviso {
    color: var(--warn);
    margin: 0;
  }
  .fin {
    justify-content: flex-end;
  }
</style>
