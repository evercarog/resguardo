<script lang="ts">
  // Centro de ayuda: buscar, las tres llaves, guías cortas de lo que se puede
  // hacer, «¿Qué hago si…?», qué ve el servidor, el certificado y el glosario
  // (las mismas entradas que los «?» de la consola). Cada bloque tiene su
  // ancla: /ayuda#guia-espejo, /ayuda#si-llaves, /ayuda#espejo…
  import { onMount, tick } from "svelte";
  import { Check, ChevronRight, Copy, Eye, EyeOff, KeyRound, LifeBuoy, LockKeyhole, Search, ShieldCheck, X } from "@lucide/svelte";
  import { GLOSARIO } from "$lib/glosario";
  import { GUIAS, PREGUNTAS } from "$lib/guias";

  let buscar = $state("");
  let abiertas = $state<Record<string, boolean>>({});
  let copiado = $state("");

  /** Sin tildes y en minúsculas, para buscar «cancelacion» y encontrar «cancelación». */
  const plano = (t: string) => t.normalize("NFD").replace(/[̀-ͯ]/g, "").toLowerCase();
  const q = $derived(plano(buscar.trim()));
  const casa = (...t: (string | undefined)[]) => !q || q.split(/\s+/).every((p) => plano(t.filter(Boolean).join(" ")).includes(p));
  const guias = $derived(GUIAS.filter((g) => casa(g.titulo, g.resumen, ...g.texto, ...(g.pasos ?? []))));
  const preguntas = $derived(PREGUNTAS.filter((p) => casa(p.si, ...p.respuesta)));
  const terminos = $derived(Object.entries(GLOSARIO).filter(([, e]) => casa(e.title, e.text, e.todo)));
  const nada = $derived(!!q && !guias.length && !preguntas.length && !terminos.length);

  onMount(async () => {
    await tick();
    const id = location.hash.slice(1);
    if (!id) return;
    if (GUIAS.some((g) => g.id === id) || PREGUNTAS.some((p) => p.id === id)) abiertas[id] = true;
    await tick();
    document.getElementById(id)?.scrollIntoView({ block: "center" });
  });

  async function copiar(t: string) {
    try {
      await navigator.clipboard.writeText(t);
      copiado = t;
      setTimeout(() => copiado === t && (copiado = ""), 1500);
    } catch {
      /* sin portapapeles */
    }
  }
</script>

<svelte:head><title>Ayuda · Resguardo Server</title></svelte:head>

<div class="page estrecha">
  <div class="page-top">
    <div>
      <h1>Ayuda</h1>
      <p>Cómo protege Resguardo Server las copias, qué se puede hacer y qué hacer si algo va mal.</p>
    </div>
  </div>

  <label class="buscar">
    <Search size={16} />
    <input class="input" type="search" placeholder="Buscar en la ayuda: espejo, clave, restaurar…" bind:value={buscar} aria-label="Buscar en la ayuda" />
    {#if buscar}<button class="icon-btn" aria-label="Borrar la búsqueda" onclick={() => (buscar = "")}><X size={14} /></button>{/if}
  </label>

  {#if nada}
    <div class="card empty-state">
      <LifeBuoy size={28} strokeWidth={1.5} />
      <strong>Nada sobre «{buscar}»</strong>
      <span class="faint">Prueba con otra palabra, como «copia», «clave» o «nube».</span>
    </div>
  {/if}

  {#if !q}
    <section class="card p" id="llaves">
      <h2 class="section-title">Las tres llaves</h2>
      <div class="tres">
        <div><span class="card-icon on"><ShieldCheck size={18} /></span><strong>Tu sesión</strong><p>Cuenta y verificación en dos pasos. Para ver el estado y pedir cosas que no ponen nada en riesgo, como copiar ahora.</p></div>
        <div><span class="card-icon on"><LockKeyhole size={18} /></span><strong>La contraseña del repositorio</strong><p>Para ver archivos, restaurar, descargar o borrar versiones. Va sellada solo para el equipo, que la comprueba.</p></div>
        <div><span class="card-icon on"><KeyRound size={18} /></span><strong>La clave de administración</strong><p>Para cambiar qué se copia, dónde y cuándo, dar de alta equipos, pausar o mover el cliente. El servidor nunca la ve.</p></div>
      </div>
    </section>
  {/if}

  {#if guias.length}
    <section>
      <div class="section-head"><h2>Guías <span class="count">· {guias.length}</span></h2></div>
      <div class="card p-0 lista">
        {#each guias as g (g.id)}
          <details class="bloque" id={g.id} bind:open={() => abiertas[g.id] || !!q, (v) => (abiertas[g.id] = v)}>
            <summary>
              <span class="sum-texto"><strong>{g.titulo}</strong><span class="faint">{g.resumen}</span></span>
              <ChevronRight size={16} />
            </summary>
            <div class="cuerpo">
              {#each g.texto as t, i (i)}<p>{t}</p>{/each}
              {#if g.pasos}
                <ol>{#each g.pasos as p, i (i)}<li>{p}</li>{/each}</ol>
              {/if}
              {#if g.comando}{@render comando(g.comando)}{/if}
              {#if g.ver?.length}
                <p class="ver">Ver también: {#each g.ver.filter((v) => GLOSARIO[v]) as v, i (v)}{i ? ", " : ""}<a href="#{v}">{GLOSARIO[v].title}</a>{/each}</p>
              {/if}
            </div>
          </details>
        {/each}
      </div>
    </section>
  {/if}

  {#if preguntas.length}
    <section>
      <div class="section-head"><h2>¿Qué hago si…? <span class="count">· {preguntas.length}</span></h2></div>
      <div class="card p-0 lista">
        {#each preguntas as p (p.id)}
          <details class="bloque" id={p.id} bind:open={() => abiertas[p.id] || !!q, (v) => (abiertas[p.id] = v)}>
            <summary>
              <span class="sum-texto"><strong>{p.si}</strong></span>
              <ChevronRight size={16} />
            </summary>
            <div class="cuerpo">
              {#each p.respuesta as t, i (i)}<p>{t}</p>{/each}
              {#if p.comando}{@render comando(p.comando)}{/if}
            </div>
          </details>
        {/each}
      </div>
    </section>
  {/if}

  {#if !q}
    <section class="card p">
      <h2 class="section-title">Qué ve el servidor</h2>
      <div class="dos">
        <div>
          <strong class="si"><Eye size={15} />Sí ve</strong>
          <ul>
            <li>Nombres de equipos, copias y repositorios.</li>
            <li>Horarios, estados, fechas, tamaños y número de versiones.</li>
            <li>El tipo de cada orden y quién la pidió.</li>
          </ul>
        </div>
        <div>
          <strong class="no"><EyeOff size={15} />No ve</strong>
          <ul>
            <li>Rutas ni nombres de archivos.</li>
            <li>Contraseñas, claves ni credenciales de la nube.</li>
            <li>El contenido de lo que restauras o descargas.</li>
          </ul>
        </div>
      </div>
      <p class="faint nota">Si alguien controlara el servidor podría servir una consola manipulada y capturar una clave al escribirla. Por eso: lo destructivo espera y se puede cancelar, los destinos son de solo añadir o inmutables, y la app de escritorio puede comprobar que la consola es la publicada.</p>
    </section>

    <section class="card p" id="certificado">
      <h2 class="section-title">El aviso del certificado</h2>
      <p>El servidor usa su propio certificado, por eso el navegador avisa la primera vez. Para quitar el aviso en los equipos de la oficina, instala la autoridad del servidor (<code>ca.crt</code>, en su carpeta de datos) como «entidad de certificación raíz de confianza». Antes, compara su huella con la de <a href="/ajustes">Mi cuenta y servidor</a>. Si el servidor tiene un nombre público, puede usar un certificado de Let's Encrypt y no hace falta nada de esto.</p>
    </section>
  {/if}

  {#if terminos.length}
    <section>
      <div class="section-head"><h2>Glosario <span class="count">· {terminos.length}</span></h2></div>
      <div class="card p-0 lista">
        {#each terminos as [id, e] (id)}
          <article class="termino" {id}>
            <h3>{e.title}</h3>
            <p>{e.text}</p>
            {#if e.todo}<p class="todo"><strong>Qué hacer:</strong> {e.todo}</p>{/if}
          </article>
        {/each}
      </div>
    </section>
  {/if}
</div>

{#snippet comando(c: string)}
  <div class="cmd">
    <code class="selectable">{c}</code>
    <button type="button" class="btn btn-sm" onclick={() => copiar(c)}>{#if copiado === c}<Check size={14} />Copiado{:else}<Copy size={14} />Copiar{/if}</button>
  </div>
  <p class="faint cmd-nota">En ese equipo, en una consola abierta como administrador (en Linux, con sudo).</p>
{/snippet}

<style>
  .estrecha {
    max-width: 820px;
  }
  .section-title {
    margin-bottom: var(--sp-4);
  }
  .buscar {
    position: relative;
    display: flex;
    align-items: center;
  }
  .buscar > :global(svg) {
    position: absolute;
    left: 12px;
    color: var(--text-3);
    pointer-events: none;
  }
  .buscar .input {
    padding-left: 38px;
    padding-right: 38px;
  }
  .buscar .icon-btn {
    position: absolute;
    right: 6px;
  }
  .tres {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: var(--sp-5);
  }
  .tres > div {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .tres p,
  .termino p,
  .cuerpo p,
  section > p {
    margin: 0;
    font-size: var(--fs-sm);
    color: var(--text-2);
    line-height: 1.55;
  }
  .bloque {
    border-top: 1px solid var(--border);
    scroll-margin-top: 80px;
  }
  .bloque:first-child {
    border-top: none;
  }
  summary {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    min-height: 52px;
    padding: var(--sp-3) var(--sp-5);
    cursor: pointer;
    list-style: none;
  }
  summary::-webkit-details-marker {
    display: none;
  }
  summary:hover {
    background: var(--surface-2);
  }
  summary:focus-visible {
    outline: 2px solid var(--focus);
    outline-offset: -2px;
  }
  summary > :global(svg) {
    flex: none;
    margin-left: auto;
    color: var(--text-3);
    transition: transform var(--dur-fast) var(--ease);
  }
  details[open] > summary > :global(svg) {
    transform: rotate(90deg);
  }
  .sum-texto {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }
  .sum-texto strong {
    font-weight: 500;
  }
  .sum-texto .faint {
    font-size: var(--fs-sm);
    line-height: var(--lh-sm);
  }
  .cuerpo {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
    padding: 0 var(--sp-5) var(--sp-5);
  }
  .bloque:target > summary,
  .termino:target {
    background: var(--accent-soft);
  }
  ol {
    margin: 0;
    padding-left: 20px;
    font-size: var(--fs-sm);
    color: var(--text-2);
    line-height: 1.6;
  }
  .ver {
    font-size: var(--fs-xs) !important;
  }
  .cmd {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
  }
  .cmd code {
    flex: 1;
    min-width: 0;
    padding: 8px 10px;
    font-size: 12.5px;
    word-break: break-all;
    background: var(--surface-2);
    border-radius: var(--radius-sm);
  }
  .cmd-nota {
    margin-top: -6px !important;
    font-size: var(--fs-xs) !important;
  }
  .dos {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: var(--sp-5);
  }
  .dos strong {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
  .si {
    color: var(--warn);
  }
  .no {
    color: var(--ok);
  }
  ul {
    margin: 8px 0 0;
    padding-left: 18px;
    font-size: var(--fs-sm);
    color: var(--text-2);
    line-height: 1.6;
  }
  .nota {
    margin-top: var(--sp-4) !important;
    font-size: var(--fs-xs) !important;
  }
  .termino {
    padding: var(--sp-4) var(--sp-5);
    border-top: 1px solid var(--border);
    scroll-margin-top: 80px;
  }
  .termino:first-child {
    border-top: none;
  }
  .termino h3 {
    margin-bottom: 4px;
    font-size: var(--fs-h2);
  }
  .todo {
    margin-top: 6px !important;
  }
  .todo strong {
    color: var(--text-1);
  }
  @media (max-width: 760px) {
    .tres,
    .dos {
      grid-template-columns: 1fr;
    }
    summary,
    .termino {
      padding-left: var(--sp-4);
      padding-right: var(--sp-4);
    }
    .cuerpo {
      padding: 0 var(--sp-4) var(--sp-4);
    }
  }
</style>
