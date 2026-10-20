<script lang="ts">
  // Plan 0.8.1, C4 (plan-mejoras 15d; docs/infraestructura.md): infraestructura,
  // fase 1, para cualquier entorno. Dónde están de verdad los datos y las copias:
  // los servidores físicos, sus conjuntos de discos y su hipervisor, qué equipos
  // (máquinas virtuales, contenedores o el propio servidor) y qué destinos viven
  // en cada uno. **La interfaz propone** con lo que ven los equipos («El almacén
  // guarda en ZFS «tank» (1,1 TB, 65 % usado). ¿En qué servidor está?») **y la
  // persona confirma**, con pocos campos. Todo opcional: sin nada anotado, todo
  // funciona como antes. Sin conectar ningún hipervisor (15e, aparcada).
  import { untrack } from "svelte";
  import { Check, Cloud, HardDrive, Layers, Lightbulb, Monitor, Pencil, Plus, Server, ServerCog, Trash2, TriangleAlert, X } from "@lucide/svelte";
  import CabeceraPagina from "$lib/componentes/CabeceraPagina.svelte";
  import Esqueleto from "$lib/componentes/Esqueleto.svelte";
  import Vacio from "$lib/componentes/Vacio.svelte";
  import EditorServidor from "$lib/componentes/infra/EditorServidor.svelte";
  import Modal from "$ui/componentes/Modal.svelte";
  import { actual, app, puede } from "$lib/estado.svelte";
  import { avisar, fallo } from "$lib/avisos.svelte";
  import { guardarInfra } from "$lib/infra.svelte";
  import { formaConjunto, HIPERVISORES, propuestas, TIPOS_CONJUNTO, type Infraestructura, type Propuesta, type ServidorFisico } from "$lib/infra";
  import { destinosParaPasos, detalleDestino } from "$lib/cadenas";
  import { catalogoDe, cargarCatalogo } from "$lib/catalogoDestinos.svelte";
  import { textoTipo, tipoEquipo } from "$lib/tipoEquipo";
  import { sedePorId } from "$lib/sedes";
  import { bytes, plural } from "$lib/formato";
  import { guardar as guardarTexto, leerTexto } from "$lib/recordar";
  import { hrefDestino } from "$lib/fichaDestino";

  const c = $derived(actual.id);
  $effect(() => {
    const cc = c;
    if (cc) untrack(() => void cargarCatalogo(cc));
  });
  const infra = $derived(actual.infra);
  const admin = $derived(!!actual.cliente && puede.administrar(actual.cliente.rol));
  const equipos = $derived(actual.equipos.filter((e) => e.modo !== "trasladado" && e.confirmado));
  const destinos = $derived(destinosParaPasos(actual.equipos, catalogoDe(c)));

  // Lo que la persona dijo «Ahora no» se recuerda por persona (en este navegador).
  const claveDescartes = $derived(`infra.descartes.${app.cuenta?.id ?? ""}.${c}`);
  let descartes = $state<string[]>([]);
  $effect(() => {
    const k = claveDescartes;
    untrack(() => {
      try {
        descartes = JSON.parse(leerTexto(k) || "[]");
      } catch {
        descartes = [];
      }
    });
  });
  function descartar(id: string) {
    descartes = [...descartes, id].slice(-300);
    guardarTexto(claveDescartes, JSON.stringify(descartes));
  }
  const sugerencias = $derived(propuestas(equipos, infra).filter((p) => !descartes.includes(p.id)));

  let guardando = $state(false);
  async function guardar(nueva: Infraestructura, hecho: string) {
    guardando = true;
    try {
      await guardarInfra(c, nueva);
      avisar(hecho);
    } catch (e) {
      fallo(e);
      throw e;
    } finally {
      guardando = false;
    }
  }

  // --- Servidores ---
  let editando = $state<{ s: ServidorFisico; nuevo: boolean; asignar?: string } | null>(null);
  const vacio = (): ServidorFisico => ({ id: "", nombre: "", marca: null, modelo: null, sede: null, hipervisor: null, notas: null, conjuntos: [] });
  async function guardarServidor(s: ServidorFisico, asignar?: string) {
    const otros = infra.servidores.filter((x) => x.id !== s.id);
    const equiposN = { ...infra.equipos };
    if (asignar) equiposN[asignar] = { servidor: s.id };
    // Las asignaciones a conjuntos que ya no existen se quedan solo con el servidor.
    const ajustar = (m: Infraestructura["equipos"]) =>
      Object.fromEntries(Object.entries(m).map(([k, v]) => [k, v.servidor === s.id && v.conjunto && !s.conjuntos.some((x) => x.id === v.conjunto) ? { servidor: s.id } : v]));
    const pos = infra.servidores.findIndex((x) => x.id === s.id);
    const servidores = pos >= 0 ? infra.servidores.map((x) => (x.id === s.id ? s : x)) : [...otros, s];
    await guardar({ ...infra, servidores, equipos: ajustar(equiposN), destinos: ajustar(infra.destinos) }, `Servidor «${s.nombre}» guardado.`);
  }
  let quitando = $state<ServidorFisico | null>(null);
  async function quitarServidor(s: ServidorFisico) {
    const sin = (m: Infraestructura["equipos"]) => Object.fromEntries(Object.entries(m).filter(([, v]) => v.servidor !== s.id));
    await guardar({ ...infra, servidores: infra.servidores.filter((x) => x.id !== s.id), equipos: sin(infra.equipos), destinos: sin(infra.destinos) }, `Servidor «${s.nombre}» quitado.`).catch(() => {});
    quitando = null;
  }

  // --- Asignar un equipo o un destino a un servidor (y conjunto) ---
  type Asignacion = { servidor: string; conjunto: string };
  let eleccion = $state<Record<string, Asignacion>>({});
  const elegido = (k: string, actualS?: { servidor: string; conjunto?: string | null }): Asignacion => eleccion[k] ?? { servidor: actualS?.servidor ?? "", conjunto: actualS?.conjunto ?? "" };
  function elegir(k: string, x: Partial<Asignacion>, actualS?: { servidor: string; conjunto?: string | null }) {
    const prev = elegido(k, actualS);
    const sig = { ...prev, ...x };
    if (x.servidor !== undefined && x.servidor !== prev.servidor) sig.conjunto = "";
    eleccion = { ...eleccion, [k]: sig };
  }
  async function asignar(clase: "equipos" | "destinos", k: string, a: Asignacion, nombre: string) {
    const m = { ...infra[clase] };
    if (!a.servidor) delete m[k];
    else m[k] = { servidor: a.servidor, ...(a.conjunto ? { conjunto: a.conjunto } : {}) };
    await guardar({ ...infra, [clase]: m }, a.servidor ? `${nombre}: anotado en «${infra.servidores.find((s) => s.id === a.servidor)?.nombre}».` : `${nombre}: sin servidor.`).catch(() => {});
    const { [`${clase}:${k}`]: _q, ...resto } = eleccion;
    eleccion = resto;
  }

  /** Aceptar una propuesta «es un servidor físico»: el servidor con lo que vio el agente, para revisarlo. */
  function aceptarServidor(p: Propuesta) {
    if (p.servidor) editando = { s: p.servidor, nuevo: true, asignar: p.equipo.id };
  }

  const nombreHiper = (id: string | null | undefined) => HIPERVISORES.find((h) => h.id === id)?.nombre ?? null;
  const enServidor = (id: string) => ({
    equipos: equipos.filter((e) => infra.equipos[e.id]?.servidor === id),
    destinos: destinos.filter((d) => infra.destinos[d.clave]?.servidor === id),
  });
  const minusc = (t: string) => t.charAt(0).toLowerCase() + t.slice(1);
  const ICONO_ESTADO = { bien: Check, degradado: TriangleAlert, reconstruyendo: Layers, sin_revisar: Layers } as const;
</script>

<svelte:head><title>Infraestructura · {actual.cliente?.nombre ?? "Resguardo"}</title></svelte:head>

<div class="page">
  <CabeceraPagina
    titulo="Infraestructura"
    icono={ServerCog}
    migas={[{ texto: actual.cliente?.nombre ?? "Cliente", href: `/c/${c}` }, { texto: "Infraestructura" }]}
    resumen="Dónde están de verdad tus datos: los servidores físicos, sus discos y qué vive en cada uno. Es opcional: sirve para la regla 3-2-1-1-0 (los soportes reales y el aviso «servidor físico compartido»). Resguardo no gestiona los discos ni el hipervisor."
  >
    {#snippet acciones()}
      {#if admin}<button class="btn btn-primary" disabled={guardando} onclick={() => (editando = { s: vacio(), nuevo: true })}><Plus size={16} />Añadir un servidor</button>{/if}
    {/snippet}
  </CabeceraPagina>

  {#if !actual.cargado}
    <Esqueleto forma="filas" n={4} etiqueta="Cargando la infraestructura…" />
  {:else}
    {#if admin && sugerencias.length}
      <section class="card p" aria-labelledby="t-propuestas">
        <h2 class="section-title" id="t-propuestas"><Lightbulb size={16} />Lo que ven tus equipos <span class="faint cuenta">· {sugerencias.length}</span></h2>
        <p class="faint intro">Confírmalo con un clic. Desde dentro de una máquina virtual o un contenedor no se ven los discos del anfitrión: eso lo anotas tú.</p>
        <ul class="propuestas">
          {#each sugerencias as p (p.id)}
            {@const k = p.clase === "zona" ? `destinos:${p.claveDestino}` : `equipos:${p.equipo.id}`}
            {@const a = elegido(k)}
            {@const srv = infra.servidores.find((s) => s.id === a.servidor)}
            <li class="propuesta">
              <span class="p-ic" aria-hidden="true">{#if p.clase === "zona"}<HardDrive size={16} />{:else if p.clase === "servidor"}<Server size={16} />{:else}<Monitor size={16} />{/if}</span>
              <p class="p-frase">{p.frase}</p>
              <div class="p-acciones">
                {#if p.clase === "servidor"}
                  <button class="btn btn-sm btn-primary" disabled={guardando} onclick={() => aceptarServidor(p)}><Check size={14} />Anotarlo como servidor</button>
                  {#if infra.servidores.length}
                    <select class="input input-sm" aria-label="O es parte de" value={a.servidor} onchange={(ev) => elegir(k, { servidor: ev.currentTarget.value })}>
                      <option value="">O está en…</option>
                      {#each infra.servidores as s (s.id)}<option value={s.id}>{s.nombre}</option>{/each}
                    </select>
                    {#if a.servidor}<button class="btn btn-sm" disabled={guardando} onclick={() => asignar("equipos", p.equipo.id, a, p.equipo.nombre)}>Guardar</button>{/if}
                  {/if}
                {:else if infra.servidores.length}
                  <select class="input input-sm" aria-label="Servidor físico" value={a.servidor} onchange={(ev) => elegir(k, { servidor: ev.currentTarget.value })}>
                    <option value="">Elige el servidor…</option>
                    {#each infra.servidores as s (s.id)}<option value={s.id}>{s.nombre}</option>{/each}
                  </select>
                  {#if srv?.conjuntos.length}
                    <select class="input input-sm" aria-label="Conjunto de discos" value={a.conjunto} onchange={(ev) => elegir(k, { conjunto: ev.currentTarget.value })}>
                      <option value="">¿En qué discos? (opcional)</option>
                      {#each srv.conjuntos as x (x.id)}<option value={x.id}>{x.nombre} · {formaConjunto(x)}</option>{/each}
                    </select>
                  {/if}
                  <button class="btn btn-sm btn-primary" disabled={guardando || !a.servidor} onclick={() => (p.clase === "zona" ? asignar("destinos", p.claveDestino!, a, p.equipo.nombre) : asignar("equipos", p.equipo.id, a, p.equipo.nombre))}><Check size={14} />Guardar</button>
                {:else}
                  <button class="btn btn-sm" disabled={guardando} onclick={() => (editando = { s: vacio(), nuevo: true })}><Plus size={14} />Anota primero el servidor</button>
                {/if}
                <button class="btn btn-sm btn-ghost" onclick={() => descartar(p.id)}><X size={14} />Ahora no</button>
              </div>
            </li>
          {/each}
        </ul>
      </section>
    {/if}

    <section aria-labelledby="t-servidores">
      <h2 class="section-title" id="t-servidores">Servidores físicos {#if infra.servidores.length}<span class="faint cuenta">· {infra.servidores.length}</span>{/if}</h2>
      {#if !infra.servidores.length}
        <div class="card">
          <Vacio icono={ServerCog} titulo="Nada anotado todavía" texto="Sin infraestructura, todo funciona igual. Anótala si quieres saber dónde están de verdad las copias: p. ej. una máquina virtual y su almacén en el mismo servidor físico.">
            {#if admin}<button class="btn btn-primary" onclick={() => (editando = { s: vacio(), nuevo: true })}><Plus size={16} />Añadir un servidor</button>{/if}
          </Vacio>
        </div>
      {:else}
        <div class="servidores">
          {#each infra.servidores as s (s.id)}
            {@const dentro = enServidor(s.id)}
            {@const sede = sedePorId(actual.organizacion, s.sede)}
            <article class="card p servidor" aria-labelledby="srv-{s.id}">
              <header class="srv-cab">
                <span class="tile-ic" aria-hidden="true"><Server size={16} /></span>
                <div class="srv-t">
                  <h3 id="srv-{s.id}">{s.nombre}</h3>
                  <p class="faint">{[nombreHiper(s.hipervisor) ?? "Sin hipervisor", [s.marca, s.modelo].filter(Boolean).join(" "), sede?.nombre].filter(Boolean).join(" · ")}</p>
                </div>
                {#if admin}
                  <div class="srv-acc">
                    <button class="icon-btn" aria-label="Cambiar {s.nombre}" disabled={guardando} onclick={() => (editando = { s, nuevo: false })}><Pencil size={15} /></button>
                    <button class="icon-btn" aria-label="Quitar {s.nombre}" disabled={guardando} onclick={() => (quitando = s)}><Trash2 size={15} /></button>
                  </div>
                {/if}
              </header>
              {#if s.notas}<p class="notas">{s.notas}</p>{/if}
              {#if s.conjuntos.length}
                <h4 class="etiqueta-seccion">Discos</h4>
                <ul class="conjuntos">
                  {#each s.conjuntos as x (x.id)}
                    {@const Ic = x.estado ? ICONO_ESTADO[x.estado] : Layers}
                    <li>
                      <HardDrive size={14} aria-hidden="true" />
                      <span class="cj-n">{x.nombre}</span>
                      <span class="faint">{TIPOS_CONJUNTO.find((t) => t.id === x.tipo)?.nombre ?? x.tipo}{x.discos ? ` · ${plural(x.discos, "disco", "discos")}` : ""}{x.capacidad ? ` · ${bytes(x.capacidad)}` : ""}</span>
                      {#if x.estado === "degradado"}<span class="badge tone-warn"><Ic size={12} />Degradado</span>{:else if x.estado === "reconstruyendo"}<span class="badge tone-info"><Ic size={12} />Reconstruyendo</span>{:else if x.estado === "bien"}<span class="badge tone-ok"><Ic size={12} />Bien</span>{/if}
                    </li>
                  {/each}
                </ul>
              {/if}
              <h4 class="etiqueta-seccion">Lo que vive aquí</h4>
              {#if dentro.equipos.length || dentro.destinos.length}
                <ul class="dentro">
                  {#each dentro.equipos as e (e.id)}
                    {@const cj = s.conjuntos.find((x) => x.id === infra.equipos[e.id]?.conjunto)}
                    <li><Monitor size={14} aria-hidden="true" /><a class="link" href="/c/{c}/equipos/{e.id}">{e.nombre}</a><span class="faint">{minusc(textoTipo(tipoEquipo(e)))}{cj ? ` · en ${cj.nombre}` : ""}</span></li>
                  {/each}
                  {#each dentro.destinos as d (d.clave)}
                    {@const cj = s.conjuntos.find((x) => x.id === infra.destinos[d.clave]?.conjunto)}
                    <li><HardDrive size={14} aria-hidden="true" /><a class="link" href={hrefDestino(c, d.clave)}>{d.nombre}</a><span class="faint">{detalleDestino(d)}{cj ? ` · en ${cj.nombre}` : ""}</span></li>
                  {/each}
                </ul>
              {:else}
                <p class="faint">Nada asignado todavía: elige el servidor de cada equipo o destino abajo.</p>
              {/if}
            </article>
          {/each}
        </div>
      {/if}
    </section>

    {#if infra.servidores.length}
      <section class="card lista" aria-labelledby="t-equipos-srv">
        <h2 class="section-title fila-t" id="t-equipos-srv">Equipos</h2>
        {#each equipos as e (e.id)}
          {@const k = `equipos:${e.id}`}
          {@const ya = infra.equipos[e.id]}
          {@const a = elegido(k, ya)}
          {@const srv = infra.servidores.find((s) => s.id === a.servidor)}
          {@const cambia = a.servidor !== (ya?.servidor ?? "") || a.conjunto !== (ya?.conjunto ?? "")}
          <div class="fila asignar">
            <span class="as-n"><Monitor size={14} aria-hidden="true" /><a class="link" href="/c/{c}/equipos/{e.id}">{e.nombre}</a><span class="faint">{textoTipo(tipoEquipo(e))}</span></span>
            {#if admin}
              <span class="as-c">
                <select class="input input-sm" aria-label="Servidor físico de {e.nombre}" value={a.servidor} onchange={(ev) => elegir(k, { servidor: ev.currentTarget.value }, ya)}>
                  <option value="">Sin anotar</option>
                  {#each infra.servidores as s (s.id)}<option value={s.id}>{s.nombre}</option>{/each}
                </select>
                {#if srv?.conjuntos.length}
                  <select class="input input-sm" aria-label="Discos de {e.nombre}" value={a.conjunto} onchange={(ev) => elegir(k, { conjunto: ev.currentTarget.value }, ya)}>
                    <option value="">Discos sin decir</option>
                    {#each srv.conjuntos as x (x.id)}<option value={x.id}>{x.nombre}</option>{/each}
                  </select>
                {/if}
                {#if cambia}<button class="btn btn-sm btn-primary" disabled={guardando} onclick={() => asignar("equipos", e.id, a, e.nombre)}>Guardar</button>{/if}
              </span>
            {:else}
              <span class="faint">{infra.servidores.find((s) => s.id === ya?.servidor)?.nombre ?? "Sin anotar"}</span>
            {/if}
          </div>
        {/each}
      </section>

      <section class="card lista" aria-labelledby="t-destinos-srv">
        <h2 class="section-title fila-t" id="t-destinos-srv">Destinos y zonas</h2>
        {#each destinos.filter((d) => d.tipo !== "nube" && d.tipo !== "b2" && d.tipo !== "s3") as d (d.clave)}
          {@const k = `destinos:${d.clave}`}
          {@const ya = infra.destinos[d.clave]}
          {@const a = elegido(k, ya)}
          {@const srv = infra.servidores.find((s) => s.id === a.servidor)}
          {@const cambia = a.servidor !== (ya?.servidor ?? "") || a.conjunto !== (ya?.conjunto ?? "")}
          <div class="fila asignar">
            <span class="as-n"><HardDrive size={14} aria-hidden="true" /><a class="link" href={hrefDestino(c, d.clave)}>{d.nombre}</a><span class="faint">{detalleDestino(d)}</span></span>
            {#if admin}
              <span class="as-c">
                <select class="input input-sm" aria-label="Servidor físico de {d.nombre}" value={a.servidor} onchange={(ev) => elegir(k, { servidor: ev.currentTarget.value }, ya)}>
                  <option value="">{d.zona ? "El de su almacén" : "Sin anotar"}</option>
                  {#each infra.servidores as s (s.id)}<option value={s.id}>{s.nombre}</option>{/each}
                </select>
                {#if srv?.conjuntos.length}
                  <select class="input input-sm" aria-label="Discos de {d.nombre}" value={a.conjunto} onchange={(ev) => elegir(k, { conjunto: ev.currentTarget.value }, ya)}>
                    <option value="">Discos sin decir</option>
                    {#each srv.conjuntos as x (x.id)}<option value={x.id}>{x.nombre} · {formaConjunto(x)}</option>{/each}
                  </select>
                {/if}
                {#if cambia}<button class="btn btn-sm btn-primary" disabled={guardando} onclick={() => asignar("destinos", d.clave, a, d.nombre)}>Guardar</button>{/if}
              </span>
            {:else}
              <span class="faint">{infra.servidores.find((s) => s.id === ya?.servidor)?.nombre ?? "Sin anotar"}</span>
            {/if}
          </div>
        {/each}
        <p class="faint pie"><Cloud size={13} aria-hidden="true" />La nube (Backblaze, S3, Dropbox…) no se anota: siempre está fuera.</p>
      </section>
    {/if}
    {#if infra.actualizada}<p class="faint pie">Cambiado {infra.por ? `por ${infra.por} ` : ""}el {new Date(infra.actualizada).toLocaleString("es")}. Es de esta consola.</p>{/if}
  {/if}
</div>

{#if editando}
  <EditorServidor
    servidor={editando.s}
    usados={infra.servidores.map((s) => s.id).filter((id) => id !== editando!.s.id)}
    sedes={actual.organizacion.sedes}
    alGuardar={(s) => guardarServidor(s, editando?.asignar)}
    onclose={() => (editando = null)}
  />
{/if}
{#if quitando}
  {@const q = quitando}
  <Modal labelledby="t-quitar-srv" onclose={() => (quitando = null)} width={440} ocupado={guardando}>
    <div class="dlg-title">
      <span class="ticon danger"><Trash2 size={18} /></span>
      <div>
        <h2 id="t-quitar-srv">¿Quitar «{q.nombre}»?</h2>
        <p>Se quitan también sus discos y lo que estaba anotado en él. No toca nada en los equipos ni en las copias.</p>
      </div>
    </div>
    <footer>
      <button type="button" class="btn btn-ghost" disabled={guardando} onclick={() => (quitando = null)}>Cancelar</button>
      <button type="button" class="btn btn-danger" disabled={guardando} onclick={() => quitarServidor(q)}>Quitar</button>
    </footer>
  </Modal>
{/if}

<style>
  .page {
    display: flex;
    flex-direction: column;
    gap: var(--sp-6);
  }
  .section-title {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .cuenta {
    font-weight: 400;
  }
  .intro {
    margin: 4px 0 var(--sp-3);
  }
  .propuestas {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .propuesta {
    display: grid;
    grid-template-columns: 32px 1fr;
    gap: 4px var(--sp-3);
    align-items: start;
    padding: var(--sp-3);
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: var(--radius);
  }
  .p-ic {
    display: grid;
    place-items: center;
    width: 32px;
    height: 32px;
    color: var(--text-2);
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 8px;
  }
  .p-frase {
    margin: 0;
    align-self: center;
  }
  .p-acciones :global(.input),
  .as-c :global(.input) {
    width: auto;
    max-width: 100%;
  }
  .p-acciones {
    grid-column: 2;
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--sp-2);
  }
  .servidores {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(320px, 1fr));
    gap: var(--sp-4);
    margin-top: var(--sp-3);
  }
  .srv-cab {
    display: flex;
    align-items: flex-start;
    gap: var(--sp-3);
  }
  .srv-t {
    flex: 1;
    min-width: 0;
  }
  .srv-t h3 {
    margin: 0;
    font-size: var(--fs-h2);
    overflow-wrap: anywhere;
  }
  .srv-t p {
    margin: 2px 0 0;
    font-size: var(--fs-sm);
  }
  .srv-acc {
    display: flex;
    gap: 2px;
  }
  .notas {
    margin: var(--sp-3) 0 0;
    font-size: var(--fs-sm);
    color: var(--text-2);
    white-space: pre-line;
  }
  .servidor h4 {
    margin: var(--sp-4) 0 var(--sp-2);
  }
  .conjuntos,
  .dentro {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin: 0;
    padding: 0;
    font-size: var(--fs-sm);
    list-style: none;
  }
  .conjuntos li,
  .dentro li {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
  }
  .conjuntos :global(svg),
  .dentro :global(svg) {
    color: var(--text-3);
  }
  .cj-n {
    font-weight: 500;
  }
  .fila-t {
    padding: var(--sp-4) var(--sp-4) var(--sp-2);
    margin: 0;
  }
  .asignar {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: var(--sp-2) var(--sp-4);
  }
  .as-n {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
    min-width: 0;
  }
  .as-n :global(svg) {
    color: var(--text-3);
  }
  .as-c {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--sp-2);
  }
  .pie {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 0;
    padding: var(--sp-3) var(--sp-4);
    font-size: var(--fs-sm);
  }
  @media (max-width: 640px) {
    .servidores {
      grid-template-columns: 1fr;
    }
    .p-acciones {
      grid-column: 1 / -1;
    }
  }
</style>
