<script lang="ts">
  // Plan 0.8.1, C4 (15d): crear o cambiar un servidor físico (nombre, marca,
  // modelo, sede, hipervisor y notas) con sus conjuntos de discos. Pocos campos:
  // lo que el agente vio ya viene relleno (la persona lo confirma).
  //
  // Plan 0.8.2, A2 (15vv): sus discos físicos uno a uno (nombre, tipo, modelo,
  // número de serie escrito a mano, tamaño, bahía, estado y notas) y cada conjunto
  // con los discos que lo forman. Un conjunto de antes, sin discos elegidos, sigue
  // con su «cuántos discos».
  import { ChevronDown, ChevronRight, HardDrive, Layers, Plus, Server, Trash2, TriangleAlert } from "@lucide/svelte";
  import Modal from "$ui/componentes/Modal.svelte";
  import { bytesInfra, detalleDisco, ESTADOS_CONJUNTO, ESTADOS_DISCO, HIPERVISORES, idDe, MAX_BYTES_INFRA, TIPOS_CONJUNTO, TIPOS_DISCO, type ConjuntoDiscos, type DiscoFisico, type ServidorFisico } from "$lib/infra";
  import { actual } from "$lib/estado.svelte";
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

  const copia = (): ServidorFisico => {
    const x = JSON.parse(JSON.stringify(servidor)) as ServidorFisico;
    return { ...x, discos: x.discos ?? [], conjuntos: (x.conjuntos ?? []).map((c) => ({ ...c, miembros: c.miembros ?? [] })) };
  };
  let s = $state<ServidorFisico>(copia());
  const enTB = (b: number | null | undefined) => (b ? String(Math.round((b / 1e12) * 100) / 100).replace(".", ",") : "");
  /** La capacidad y el tamaño se escriben en TB (decimales con coma o punto). */
  let teras = $state<string[]>(copia().conjuntos.map((c) => enTB(c.capacidad)));
  let terasDisco = $state<string[]>(copia().discos.map((d) => enTB(d.tamano)));
  /** Los discos con todos sus campos a la vista (en estrecho, uno a la vez). */
  let abiertos = $state<Set<string>>(new Set(copia().discos.filter((d) => !d.serie).map((d) => d.id).slice(0, 2)));
  let ocupado = $state(false);
  let error = $state("");
  const numeroTB = (t: string) => Number(t.replace(",", "."));

  const falta = $derived(
    !s.nombre.trim()
      ? { texto: "Falta el nombre", campo: "es-nombre" }
      : s.discos.some((d) => !d.nombre.trim())
        ? { texto: "Cada disco necesita un nombre" }
        : s.conjuntos.some((c) => !c.nombre.trim())
          ? { texto: "Cada conjunto necesita un nombre" }
          : [...teras, ...terasDisco].some((t) => t.trim() && !(numeroTB(t) > 0))
            ? { texto: "Un tamaño no es un número" }
            : null,
  );

  function anadirDisco() {
    const ids = s.discos.map((d) => d.id);
    const id = idDe(`disco-${s.discos.length + 1}`, ids);
    s.discos = [...s.discos, { id, nombre: `Bahía ${s.discos.length + 1}`, tipo: null, modelo: null, serie: null, tamano: null, bahia: null, estado: null, notas: null }];
    terasDisco = [...terasDisco, ""];
    abiertos = new Set([...abiertos, id]);
  }
  function quitarDisco(i: number) {
    const id = s.discos[i].id;
    s.discos = s.discos.filter((_, j) => j !== i);
    terasDisco = terasDisco.filter((_, j) => j !== i);
    s.conjuntos = s.conjuntos.map((c) => ({ ...c, miembros: (c.miembros ?? []).filter((m) => m !== id) }));
  }
  function alternarDisco(id: string) {
    const x = new Set(abiertos);
    if (x.has(id)) x.delete(id);
    else x.add(id);
    abiertos = x;
  }
  function anadirConjunto() {
    const ids = s.conjuntos.map((c) => c.id);
    s.conjuntos = [...s.conjuntos, { id: idDe(`conjunto-${s.conjuntos.length + 1}`, ids), nombre: "", tipo: "raid1", discos: s.discos.length ? null : 2, miembros: [], capacidad: null, estado: null }];
    teras = [...teras, ""];
  }
  function quitarConjunto(i: number) {
    s.conjuntos = s.conjuntos.filter((_, j) => j !== i);
    teras = teras.filter((_, j) => j !== i);
  }
  function alternarMiembro(c: ConjuntoDiscos, d: string, on: boolean) {
    const m = (c.miembros ?? []).filter((x) => x !== d);
    c.miembros = on ? [...m, d] : m;
  }

  /** Lo que se guarda: limpio, con los tamaños en bytes. */
  function armar(): ServidorFisico {
    const discos: DiscoFisico[] = s.discos.map((d, i) => ({
      ...d,
      nombre: d.nombre.trim(),
      modelo: d.modelo?.trim() || null,
      serie: d.serie?.trim() || null,
      bahia: d.bahia?.trim() || null,
      notas: d.notas?.trim() || null,
      tamano: terasDisco[i]?.trim() ? Math.round(numeroTB(terasDisco[i]) * 1e12) : null,
    }));
    const conjuntos: ConjuntoDiscos[] = s.conjuntos.map((c, i) => ({
      ...c,
      nombre: c.nombre.trim(),
      miembros: (c.miembros ?? []).filter((m) => discos.some((d) => d.id === m)),
      discos: c.miembros?.length ? null : c.discos && c.discos > 0 ? Math.min(256, Math.floor(c.discos)) : null,
      capacidad: teras[i]?.trim() ? Math.round(numeroTB(teras[i]) * 1e12) : null,
    }));
    return { ...s, id: s.id || idDe(s.nombre, usados), nombre: s.nombre.trim(), marca: s.marca?.trim() || null, modelo: s.modelo?.trim() || null, notas: s.notas?.trim() || null, discos, conjuntos };
  }
  /** A5: lo que ocupará toda la infraestructura en cada equipo con este servidor así. */
  const ocupara = $derived.by(() => {
    if (falta) return null;
    const nuevo = armar();
    const otros = actual.infra.servidores.filter((x) => x.id !== servidor.id && x.id !== nuevo.id);
    return bytesInfra({ ...actual.infra, servidores: [...otros, nuevo] });
  });

  async function guardar(e: SubmitEvent) {
    e.preventDefault();
    if (falta) return;
    ocupado = true;
    error = "";
    try {
      await alGuardar(armar());
      onclose();
    } catch (err) {
      error = (err as Error).message;
    } finally {
      ocupado = false;
    }
  }
</script>

<Modal labelledby="t-servidor" {onclose} width={820} dismissible={false} {ocupado}>
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

    <fieldset class="bloque">
      <legend class="field-label"><HardDrive size={14} />Discos físicos <span class="faint">(opcional)</span></legend>
      <p class="field-hint">Uno a uno, para saber qué disco cambiar cuando uno falla. El número de serie lo escribes tú (Resguardo no lo lee de los equipos).</p>
      {#each s.discos as d, i (d.id)}
        {@const abierto = abiertos.has(d.id)}
        <div class="disco">
          <div class="disco-cab">
            <button type="button" class="icon-btn" aria-expanded={abierto} aria-label="{abierto ? 'Plegar' : 'Desplegar'} {d.nombre || `el disco ${i + 1}`}" onclick={() => alternarDisco(d.id)}>
              {#if abierto}<ChevronDown size={15} />{:else}<ChevronRight size={15} />{/if}
            </button>
            <div class="field crece">
              <label class="field-label sr-only" for="es-d-n-{i}">Nombre del disco</label>
              <input id="es-d-n-{i}" class="input" bind:value={d.nombre} maxlength="80" placeholder="Bahía 2" disabled={ocupado} />
            </div>
            {#if !abierto}<span class="faint resumen-d">{[detalleDisco({ ...d, tamano: terasDisco[i] ? numeroTB(terasDisco[i]) * 1e12 : null }), d.serie ? `S/N ${d.serie}` : null].filter(Boolean).join(" · ")}</span>{/if}
            <button type="button" class="icon-btn quitar" aria-label="Quitar el disco {d.nombre || i + 1}" disabled={ocupado} onclick={() => quitarDisco(i)}><Trash2 size={15} /></button>
          </div>
          {#if abierto}
            <div class="disco-campos">
              <div class="field">
                <label class="field-label" for="es-d-t-{i}">Tipo</label>
                <select id="es-d-t-{i}" class="input" bind:value={d.tipo} disabled={ocupado}>
                  <option value={null}>Sin decir</option>
                  {#each TIPOS_DISCO as t (t.id)}<option value={t.id}>{t.nombre}</option>{/each}
                </select>
              </div>
              <div class="field">
                <label class="field-label" for="es-d-m-{i}">Modelo</label>
                <input id="es-d-m-{i}" class="input" bind:value={d.modelo} maxlength="120" disabled={ocupado} />
              </div>
              <div class="field">
                <label class="field-label" for="es-d-s-{i}">Número de serie</label>
                <input id="es-d-s-{i}" class="input mono" bind:value={d.serie} maxlength="64" autocomplete="off" spellcheck="false" disabled={ocupado} />
              </div>
              <div class="field corto">
                <label class="field-label" for="es-d-c-{i}">Tamaño (TB)</label>
                <input id="es-d-c-{i}" class="input" inputmode="decimal" bind:value={terasDisco[i]} placeholder="4" disabled={ocupado} />
              </div>
              <div class="field corto">
                <label class="field-label" for="es-d-b-{i}">Bahía o ranura</label>
                <input id="es-d-b-{i}" class="input" bind:value={d.bahia} maxlength="40" placeholder="2" disabled={ocupado} />
              </div>
              <div class="field">
                <label class="field-label" for="es-d-e-{i}">Estado</label>
                <select id="es-d-e-{i}" class="input" bind:value={d.estado} disabled={ocupado}>
                  <option value={null}>Sin anotar</option>
                  {#each ESTADOS_DISCO as x (x.id)}<option value={x.id}>{x.nombre}</option>{/each}
                </select>
              </div>
              <div class="field ancho">
                <label class="field-label" for="es-d-o-{i}">Notas</label>
                <input id="es-d-o-{i}" class="input" bind:value={d.notas} maxlength="500" placeholder="Cambiado en marzo" disabled={ocupado} />
              </div>
              {#if d.visto}<p class="faint visto">Lo ve el propio equipo de este servidor (su nombre en la ficha del equipo sale de aquí).</p>{/if}
            </div>
          {/if}
        </div>
      {/each}
      <button type="button" class="btn btn-sm" disabled={ocupado || s.discos.length >= 64} onclick={anadirDisco}><Plus size={14} />Añadir un disco</button>
    </fieldset>

    <fieldset class="bloque">
      <legend class="field-label"><Layers size={14} />Conjuntos de discos</legend>
      <p class="field-hint">Un RAID, un grupo de ZFS, Espacios de almacenamiento, el disco virtual de una controladora o un disco suelto. El estado es el que anotas tú (Resguardo no lo comprueba).</p>
      {#each s.conjuntos as c, i (c.id)}
        <div class="conjunto">
          <div class="conjunto-campos">
            <div class="field">
              <label class="field-label" for="es-c-n-{i}">Nombre</label>
              <input id="es-c-n-{i}" class="input" bind:value={c.nombre} maxlength="80" placeholder="Disco virtual 2" disabled={ocupado} />
            </div>
            <div class="field">
              <label class="field-label" for="es-c-t-{i}">Tipo</label>
              <select id="es-c-t-{i}" class="input" bind:value={c.tipo} disabled={ocupado}>
                {#each TIPOS_CONJUNTO as t (t.id)}<option value={t.id}>{t.nombre}</option>{/each}
              </select>
            </div>
            {#if !c.miembros?.length}
              <div class="field corto">
                <label class="field-label" for="es-c-d-{i}">Discos</label>
                <input id="es-c-d-{i}" class="input" type="number" min="1" max="256" bind:value={c.discos} disabled={ocupado} />
              </div>
            {/if}
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
          {#if s.discos.length}
            <div class="miembros" role="group" aria-label="Discos que forman {c.nombre || `el conjunto ${i + 1}`}">
              <span class="faint">Hecho de:</span>
              {#each s.discos as d (d.id)}
                <label class="miembro">
                  <input type="checkbox" checked={c.miembros?.includes(d.id)} disabled={ocupado} onchange={(ev) => alternarMiembro(c, d.id, ev.currentTarget.checked)} />
                  <!-- Con el número de serie (o la bahía): dos discos del mismo modelo se distinguen. -->
                  <span>{d.nombre || "Disco sin nombre"}{#if d.serie || d.bahia}<span class="faint ident"> · {d.serie ? `S/N ${d.serie}` : `bahía ${d.bahia}`}</span>{/if}</span>
                </label>
              {/each}
            </div>
          {/if}
        </div>
      {/each}
      <button type="button" class="btn btn-sm" disabled={ocupado || s.conjuntos.length >= 32} onclick={anadirConjunto}><Plus size={14} />Añadir un conjunto</button>
    </fieldset>

    <div class="field">
      <label class="field-label" for="es-notas">Notas <span class="faint">(opcional)</span></label>
      <textarea id="es-notas" class="input" rows="2" maxlength="1000" bind:value={s.notas} placeholder="Rack 1, abajo" disabled={ocupado}></textarea>
      <span class="field-hint">No pongas contraseñas aquí: lo ven todas las consolas del cliente.</span>
    </div>
    {#if ocupara !== null && ocupara > MAX_BYTES_INFRA * 0.8}
      <div class="notice notice-warn" role="note"><TriangleAlert size={16} /><p>Lo anotado ya ocupa {Math.round((ocupara / MAX_BYTES_INFRA) * 100)} % de lo que cabe en cada equipo para todas las consolas. Acorta las notas si no hacen falta.</p></div>
    {/if}
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
  .bloque {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
    margin: 0;
    padding: 0;
    border: 0;
    min-width: 0;
  }
  .bloque legend {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .disco,
  .conjunto {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
    padding: var(--sp-3);
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    min-width: 0;
  }
  .disco-cab {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--sp-2);
  }
  .crece {
    flex: 1 1 160px;
    min-width: 0;
  }
  .resumen-d {
    flex: 1 1 100%;
    order: 3;
    font-size: var(--fs-xs);
    overflow-wrap: anywhere;
  }
  .disco-campos {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(150px, 1fr));
    gap: var(--sp-2) var(--sp-3);
  }
  .disco-campos .ancho {
    grid-column: 1 / -1;
  }
  .visto {
    grid-column: 1 / -1;
    margin: 0;
    font-size: var(--fs-xs);
  }
  .conjunto-campos {
    display: grid;
    grid-template-columns: minmax(110px, 1.3fr) minmax(130px, 1.4fr) 80px 110px minmax(110px, 1fr) 32px;
    align-items: end;
    gap: var(--sp-2);
  }
  .miembros {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 4px var(--sp-3);
    font-size: var(--fs-sm);
  }
  .miembro {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
  .quitar {
    margin-bottom: 1px;
  }
  @media (max-width: 760px) {
    .conjunto-campos {
      grid-template-columns: 1fr 1fr;
    }
    .quitar {
      justify-self: end;
    }
  }
</style>
