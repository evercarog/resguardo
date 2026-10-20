<script lang="ts">
  // Plan 0.8.1, C4 (15d): crear o cambiar un servidor físico (nombre, marca,
  // modelo, sede, hipervisor y notas) con sus conjuntos de discos. Pocos campos:
  // lo que el agente vio ya viene relleno (la persona lo confirma).
  import { Plus, Server, Trash2, TriangleAlert } from "@lucide/svelte";
  import Modal from "$ui/componentes/Modal.svelte";
  import { ESTADOS_CONJUNTO, HIPERVISORES, idDe, TIPOS_CONJUNTO, type ConjuntoDiscos, type ServidorFisico } from "$lib/infra";
  import type { Sede } from "$lib/sedes";
  import BotonFalta from "../BotonFalta.svelte";

  let {
    servidor,
    usados,
    sedes,
    alGuardar,
    onclose,
  }: {
    /** El que se cambia, o uno nuevo (quizá con lo que vio el agente). */
    servidor: ServidorFisico;
    /** Los ids de los demás servidores (para no repetirlos). */
    usados: string[];
    sedes: Sede[];
    alGuardar: (s: ServidorFisico) => Promise<void>;
    onclose: () => void;
  } = $props();

  const copia = (): ServidorFisico => JSON.parse(JSON.stringify(servidor));
  let s = $state<ServidorFisico>(copia());
  /** La capacidad se escribe en TB (decimales con coma o punto). */
  let teras = $state<string[]>(copia().conjuntos.map((c) => (c.capacidad ? String(Math.round((c.capacidad / 1e12) * 100) / 100).replace(".", ",") : "")));
  let ocupado = $state(false);
  let error = $state("");

  const falta = $derived(
    !s.nombre.trim()
      ? { texto: "Falta el nombre", campo: "es-nombre" }
      : s.conjuntos.some((c) => !c.nombre.trim())
        ? { texto: "Cada conjunto necesita un nombre" }
        : teras.some((t) => t.trim() && !(Number(t.replace(",", ".")) > 0))
          ? { texto: "Una capacidad no es un número" }
          : null,
  );

  function anadirConjunto() {
    const ids = s.conjuntos.map((c) => c.id);
    s.conjuntos = [...s.conjuntos, { id: idDe(`conjunto-${s.conjuntos.length + 1}`, ids), nombre: "", tipo: "raid1", discos: 2, capacidad: null, estado: null }];
    teras = [...teras, ""];
  }
  function quitarConjunto(i: number) {
    s.conjuntos = s.conjuntos.filter((_, j) => j !== i);
    teras = teras.filter((_, j) => j !== i);
  }

  async function guardar(e: SubmitEvent) {
    e.preventDefault();
    if (falta) return;
    ocupado = true;
    error = "";
    try {
      const conjuntos: ConjuntoDiscos[] = s.conjuntos.map((c, i) => ({
        ...c,
        nombre: c.nombre.trim(),
        discos: c.discos && c.discos > 0 ? Math.min(256, Math.floor(c.discos)) : null,
        capacidad: teras[i]?.trim() ? Math.round(Number(teras[i].replace(",", ".")) * 1e12) : null,
      }));
      await alGuardar({
        ...s,
        id: s.id || idDe(s.nombre, usados),
        nombre: s.nombre.trim(),
        marca: s.marca?.trim() || null,
        modelo: s.modelo?.trim() || null,
        notas: s.notas?.trim() || null,
        conjuntos,
      });
      onclose();
    } catch (err) {
      error = (err as Error).message;
    } finally {
      ocupado = false;
    }
  }
</script>

<Modal labelledby="t-servidor" {onclose} width={760} dismissible={false} {ocupado}>
  <div class="dlg-title">
    <span class="ticon"><Server size={18} /></span>
    <div>
      <h2 id="t-servidor">{servidor.nombre ? `Servidor físico «${servidor.nombre}»` : "Servidor físico nuevo"}</h2>
      <p>Solo para saber dónde están de verdad tus datos. Resguardo no gestiona los discos ni el hipervisor: tú lo anotas, él avisa.</p>
    </div>
  </div>
  <form class="form" onsubmit={guardar}>
    <div class="fila2">
      <div class="field">
        <label class="field-label" for="es-nombre">Nombre</label>
        <input id="es-nombre" class="input" bind:value={s.nombre} maxlength="80" placeholder="hv-01" disabled={ocupado} />
      </div>
      <div class="field">
        <label class="field-label" for="es-hiper">Hipervisor</label>
        <select id="es-hiper" class="input" bind:value={s.hipervisor} disabled={ocupado}>
          <option value={null}>Ninguno (uso directo)</option>
          {#each HIPERVISORES as h (h.id)}<option value={h.id}>{h.nombre}</option>{/each}
        </select>
      </div>
      <div class="field">
        <label class="field-label" for="es-marca">Marca <span class="faint">(opcional)</span></label>
        <input id="es-marca" class="input" bind:value={s.marca} maxlength="80" disabled={ocupado} />
      </div>
      <div class="field">
        <label class="field-label" for="es-modelo">Modelo <span class="faint">(opcional)</span></label>
        <input id="es-modelo" class="input" bind:value={s.modelo} maxlength="120" disabled={ocupado} />
      </div>
      {#if sedes.length}
        <div class="field">
          <label class="field-label" for="es-sede">Sede</label>
          <select id="es-sede" class="input" bind:value={s.sede} disabled={ocupado}>
            <option value={null}>Sin sede</option>
            {#each sedes as x (x.id)}<option value={x.id}>{x.nombre}</option>{/each}
          </select>
        </div>
      {/if}
    </div>

    <fieldset class="conjuntos">
      <legend class="field-label">Conjuntos de discos</legend>
      <p class="field-hint">Un RAID, un grupo de ZFS, Espacios de almacenamiento o un disco suelto. El estado es el que anotas tú (Resguardo no lo comprueba).</p>
      {#each s.conjuntos as c, i (c.id)}
        <div class="conjunto">
          <div class="field">
            <label class="field-label" for="es-c-n-{i}">Nombre</label>
            <input id="es-c-n-{i}" class="input" bind:value={c.nombre} maxlength="80" placeholder="tank" disabled={ocupado} />
          </div>
          <div class="field">
            <label class="field-label" for="es-c-t-{i}">Tipo</label>
            <select id="es-c-t-{i}" class="input" bind:value={c.tipo} disabled={ocupado}>
              {#each TIPOS_CONJUNTO as t (t.id)}<option value={t.id}>{t.nombre}</option>{/each}
            </select>
          </div>
          <div class="field corto">
            <label class="field-label" for="es-c-d-{i}">Discos</label>
            <input id="es-c-d-{i}" class="input" type="number" min="1" max="256" bind:value={c.discos} disabled={ocupado} />
          </div>
          <div class="field corto">
            <label class="field-label" for="es-c-c-{i}">Capacidad (TB)</label>
            <input id="es-c-c-{i}" class="input" inputmode="decimal" bind:value={teras[i]} placeholder="1,2" disabled={ocupado} />
          </div>
          <div class="field">
            <label class="field-label" for="es-c-e-{i}">Estado</label>
            <select id="es-c-e-{i}" class="input" bind:value={c.estado} disabled={ocupado}>
              <option value={null}>Sin anotar</option>
              {#each ESTADOS_CONJUNTO as x (x.id)}<option value={x.id}>{x.nombre}</option>{/each}
            </select>
          </div>
          <button type="button" class="icon-btn quitar" aria-label="Quitar el conjunto {c.nombre || i + 1}" disabled={ocupado} onclick={() => quitarConjunto(i)}><Trash2 size={15} /></button>
        </div>
      {/each}
      <button type="button" class="btn btn-sm" disabled={ocupado || s.conjuntos.length >= 32} onclick={anadirConjunto}><Plus size={14} />Añadir un conjunto</button>
    </fieldset>

    <div class="field">
      <label class="field-label" for="es-notas">Notas <span class="faint">(opcional)</span></label>
      <textarea id="es-notas" class="input" rows="2" maxlength="1000" bind:value={s.notas} placeholder="Rack 1, abajo" disabled={ocupado}></textarea>
      <span class="field-hint">No pongas contraseñas aquí.</span>
    </div>
    {#if error}<div class="notice notice-danger" role="alert"><TriangleAlert size={16} /><p>{error}</p></div>{/if}
    <footer>
      <button type="button" class="btn btn-ghost" disabled={ocupado} onclick={onclose}>Cancelar</button>
      <BotonFalta disabled={ocupado} {falta}>{ocupado ? "Guardando…" : "Guardar"}</BotonFalta>
    </footer>
  </form>
</Modal>

<style>
  .fila2 {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
    gap: var(--sp-3) var(--sp-4);
  }
  .conjuntos {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
    margin: 0;
    padding: 0;
    border: 0;
  }
  .conjunto {
    display: grid;
    grid-template-columns: minmax(110px, 1.3fr) minmax(130px, 1.4fr) 80px 110px minmax(110px, 1fr) 32px;
    align-items: end;
    gap: var(--sp-2);
    padding: var(--sp-3);
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: var(--radius);
  }
  .quitar {
    margin-bottom: 1px;
  }
  @media (max-width: 760px) {
    .conjunto {
      grid-template-columns: 1fr 1fr;
    }
    .quitar {
      justify-self: end;
    }
  }
</style>
