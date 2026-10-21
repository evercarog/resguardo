<script lang="ts">
  // La página de una nube conectada en un equipo (plan 0.8.1, B3; plan-mejoras 15s): lo que
  // guarda por espejo y por repositorio, con su ruta allí cuando se sabe; el estado de la
  // conexión (por la última subida de sus espejos: el agente no tiene otra forma de probarla);
  // «Probar» = hacer ahora uno de sus espejos; y «Desconectar» si nada la usa (la misma orden
  // que en la ficha del equipo, con la clave de administración).
  import { page } from "$app/state";
  import { untrack } from "svelte";
  import { ArrowRight, Cloud, Database, HardDrive, Lock, Play, Unplug } from "@lucide/svelte";
  import CabeceraPagina from "$lib/componentes/CabeceraPagina.svelte";
  import Cargando from "$lib/componentes/Cargando.svelte";
  import Chip from "$lib/componentes/Chip.svelte";
  import Tiempo from "$lib/componentes/Tiempo.svelte";
  import Ayuda from "$lib/componentes/Ayuda.svelte";
  import OrdenDialog from "$lib/componentes/OrdenDialog.svelte";
  import * as api from "$lib/api";
  import { actual, cargarCliente, puede } from "$lib/estado.svelte";
  import { seguirCambios, tocaEquipo } from "$lib/vivo.svelte";
  import { catalogoDe, cargarCatalogo } from "$lib/catalogoDestinos.svelte";
  import { hrefNubes, nubesVista } from "$lib/nubesAlmacen";
  import { admiteRevocar, queHaceAlDesconectar } from "$lib/nubesEquipo";
  import { hrefDestino } from "$lib/fichaDestino";
  import { bytes, plural } from "$lib/formato";
  import type { EquipoDetalle } from "$lib/tipos";

  const c = $derived(page.params.c ?? "");
  const id = $derived(page.params.e ?? "");
  const nombre = $derived(page.params.n ?? "");
  let equipo = $state<EquipoDetalle | null>(null);
  let error = $state("");

  $effect(() => {
    const [cc, ee] = [c, id];
    untrack(() => cargar(cc, ee));
  });
  function cargar(cc = c, ee = id) {
    api.equipo(cc, ee).then((e) => ee === id && (equipo = e), (e) => (error = e.message));
  }
  $effect(() => {
    const [cc, ee] = [c, id];
    return untrack(() =>
      seguirCambios(() => api.equipo(cc, ee).then((e) => ee === id && (equipo = e), () => {}), {
        ms: 0,
        toca: (x) => (x.t === "informe" || x.t === "equipo" || x.t === "orden") && tocaEquipo(x, ee),
      }),
    );
  });
  $effect(() => {
    const cc = c;
    if (cc) untrack(() => void cargarCatalogo(cc));
  });

  const equipos = $derived(equipo ? [...actual.equipos.filter((x) => x.id !== equipo!.id), equipo] : actual.equipos);
  const n = $derived(equipo ? (nubesVista(equipo, equipos, catalogoDe(c), c).find((x) => x.nombre === nombre) ?? null) : null);
  const admin = $derived(!!actual.cliente && puede.administrar(actual.cliente.rol));
  const ordena = $derived(!!actual.cliente && puede.ordenar(actual.cliente.rol));
  const TONO = { ok: "ok", bad: "bad", neutral: "neutral" } as const;
  const ESTADO = { ok: ["ok", "Al día"], error: ["bad", "Falló"], aviso: ["warn", "Por confirmar"], pausado: ["paused", "En pausa"], nuevo: ["neutral", "Todavía no"] } as const;

  let orden = $state<{ tipo: string; cuerpo: Record<string, unknown>; titulo: string; descripcion: string; accion?: string } | null>(null);
  function probar(t: { id: string; nombre: string }, quien: "almacen" | "equipo") {
    orden = {
      tipo: "guarda_copias",
      cuerpo: { espejo_ahora: { trabajo: t.id, ...(quien === "equipo" ? { quien: "equipo" } : {}) } },
      titulo: `Probar «${nombre}» haciendo ahora «${t.nombre}»`,
      descripcion: "Empieza en cuanto pueda, sin esperar a su hora. Solo sube lo que falta: si termina bien, la conexión con la nube funciona. Se ve en vivo aquí y en todas las consolas.",
    };
  }
  function desconectar() {
    if (!equipo || !n) return;
    orden = {
      tipo: "quitar_nube",
      cuerpo: { nombre: n.nombre },
      titulo: `Desconectar «${n.nombre}»`,
      descripcion: `${equipo.nombre} olvidará el permiso de «${n.nombre}». Lo ya subido se queda allí. ${queHaceAlDesconectar(equipo, n.tipo)}`,
      accion: "Desconectar",
    };
  }
</script>

<svelte:head><title>{nombre} · Nubes de {equipo?.nombre ?? "equipo"} · Resguardo Server</title></svelte:head>

<div class="page">
  {#if !equipo || !actual.cliente}
    {#if error}<div class="notice notice-danger"><p>{error}</p></div>{:else}<Cargando />{/if}
  {:else if !n}
    <CabeceraPagina titulo="Esta nube ya no está conectada" icono={Cloud} migas={[{ texto: "Equipos", href: `/c/${c}/equipos` }, { texto: equipo.nombre, href: `/c/${c}/equipos/${id}` }, { texto: "Nubes conectadas", href: hrefNubes(c, id) }]} />
    <div class="notice notice-info"><p>Puede que se desconectara. Las demás están en <a href={hrefNubes(c, id)}>las nubes de {equipo.nombre}</a>.</p></div>
  {:else}
    <CabeceraPagina titulo={n.nombre} icono={Cloud} migas={[{ texto: "Equipos", href: `/c/${c}/equipos` }, { texto: equipo.nombre, href: `/c/${c}/equipos/${id}` }, { texto: "Nubes conectadas", href: hrefNubes(c, id) }, { texto: n.nombre }]}>
      {#snippet detalle()}
        <p class="linea"><Chip tono={TONO[n.conexion.tono]} texto={n.conexion.texto} pequeno /><span>{n.tipoTexto} conectada en {equipo?.nombre}{#if n.conexion.cuando}{" · última subida "}<Tiempo iso={n.conexion.cuando} />{/if}</span></p>
      {/snippet}
    </CabeceraPagina>

    {#if n.conexion.tono === "bad" && n.conexion.error}<div class="notice notice-danger"><p><strong>La última subida falló:</strong> {n.conexion.error} <a href="/ayuda#{/permis|token|auth|401|403|expir|revoc/i.test(n.conexion.error) ? 'si-token' : 'si-espejo'}">Qué hacer</a></p></div>{/if}

    <section aria-labelledby="t-datos">
      <div class="section-head"><h2 id="t-datos">Cómo es</h2></div>
      <div class="card p">
        <dl class="datos">
          <div><dt>Tipo</dt><dd>{n.tipoTexto}</dd></div>
          {#if n.bucket || ["b2", "s3"].includes(n.tipo)}<div><dt>Bucket</dt><dd>{#if n.bucket}<span class="pastilla mono">{n.bucket}</span>{:else}<span class="faint">no lo sé (no está en la lista de destinos de esta consola)</span>{/if}</dd></div>{/if}
          <div><dt>Carpeta base</dt><dd>{#if n.carpetaBase}<span class="pastilla mono">{n.carpetaBase}</span>{:else}<span class="faint">la de cada espejo (abajo)</span>{/if}</dd></div>
          <div><dt>Inmutable</dt><dd>{#if n.inmutable}<Lock size={12} /> Sí: lo subido no se puede borrar ni cambiar durante su plazo{:else}No{/if}</dd></div>
          <div><dt>Espacio</dt><dd>{#if n.ocupa != null}{bytes(n.ocupa)} ocupados por sus espejos{:else}<span class="faint">sin datos</span>{/if}{#if n.espacio}{" · "}{bytes(n.espacio.libre)} libres de {bytes(n.espacio.total)}{/if}</dd></div>
          <div><dt>Qué la usa</dt><dd>{n.usos.length ? n.usos.join(", ") : "Nada ahora"}</dd></div>
        </dl>
        {#if n.vista}<p class="faint pie"><a class="link-suave" href={hrefDestino(c, n.vista.clave)}>Su ficha en «Destinos»</a>: tipo y marcas para la regla 3-2-1-1-0.</p>{/if}
      </div>
    </section>

    <section aria-labelledby="t-guarda">
      <div class="section-head"><h2 id="t-guarda">Qué guarda <span class="count">· {plural(n.espejos.length, "espejo", "espejos")}{n.directos.length ? ` y ${plural(n.directos.length, "repositorio", "repositorios")}` : ""}</span> <Ayuda id="espejo" /></h2></div>
      {#each n.espejos as x (x.trabajo.id)}
        <div class="card p-0 espejo">
          <div class="cab">
            <a class="enlace" href={x.href}>
              <HardDrive size={15} class="faint" />
              <span class="t"><strong>{x.nombre}</strong><span class="faint">Lo hace {equipo.nombre}{x.quien === "equipo" ? " (sus discos)" : ""} · en la carpeta <span class="mono">{x.carpeta}</span>{#if x.trabajo.ultima}{" · "}<Tiempo iso={x.trabajo.ultima} />{/if}</span></span>
              <Chip pequeno tono={ESTADO[x.estado][0]} texto={ESTADO[x.estado][1]} />
              <ArrowRight size={14} class="faint" />
            </a>
            {#if ordena && x.trabajo.activo}<button class="btn btn-sm" onclick={() => probar({ id: x.trabajo.id, nombre: x.nombre }, x.quien)}><Play size={13} />Probar ahora</button>{/if}
          </div>
          {#if x.repos.length}
            <ul class="repos">
              {#each x.repos as r (r.valor)}
                <li>
                  <Database size={14} class="faint" aria-hidden="true" />
                  <span class="t">
                    {#if r.equipo && r.repo}<a class="link-suave" href="/c/{c}/equipos/{r.equipo.id}/repositorios/{encodeURIComponent(r.repo.id)}">{r.nombre}</a>{:else}<span class:mono={!r.equipo}>{r.nombre}</span>{/if}
                    <span class="faint">{r.de}</span>
                    {#if r.ruta}<span class="ruta mono">{r.ruta}</span>{/if}
                  </span>
                  <span class="num tam">{r.bytes != null ? bytes(r.bytes) : "—"}</span>
                </li>
              {/each}
            </ul>
          {:else}
            <p class="faint vacio">Ningún repositorio todavía.</p>
          {/if}
        </div>
      {/each}
      {#if n.directos.length}
        <div class="card p-0">
          <ul class="repos">
            {#each n.directos as r (r.repo.id + (r.derivada ?? ""))}
              <li>
                <Database size={14} class="faint" aria-hidden="true" />
                <span class="t"><a class="link-suave" href="/c/{c}/equipos/{id}/repositorios/{encodeURIComponent(r.repo.id)}">{r.nombre}</a><span class="faint">{equipo.nombre} la guarda aquí directamente</span>{#if r.ruta}<span class="ruta mono">{r.ruta}</span>{/if}</span>
                <span class="num tam">{!r.derivada && r.repo.bytes ? bytes(r.repo.bytes) : "—"}</span>
              </li>
            {/each}
          </ul>
        </div>
      {/if}
      {#if !n.espejos.length && !n.directos.length}<p class="faint">Nada todavía: úsala como destino de un espejo o de una copia.</p>{/if}
    </section>

    {#if admin}
      <section aria-labelledby="t-desc">
        <div class="section-head"><h2 id="t-desc">Desconectar</h2></div>
        <div class="card p desc">
          {#if n.motivo}
            <p class="faint">{n.motivo}</p>
          {:else}
            <p class="faint">{equipo.nombre} olvidará su permiso. Lo ya subido se queda allí.{#if !admiteRevocar(equipo)}{" "}Con este agente, el permiso sigue vivo en el proveedor: quítalo también allí.{/if}</p>
            <div><button class="btn btn-ghost" onclick={desconectar}><Unplug size={14} />Desconectar de {equipo.nombre}</button></div>
          {/if}
        </div>
      </section>
    {/if}
  {/if}
</div>

{#if orden && equipo && actual.cliente}
  <OrdenDialog cliente={actual.cliente} {equipo} tipo={orden.tipo} cuerpo={orden.cuerpo} titulo={orden.titulo} descripcion={orden.descripcion} accion={orden.accion} onclose={() => (orden = null)} alTerminar={() => (cargar(), void cargarCliente(c, { silencioso: true }))} />
{/if}

<style>
  .linea {
    margin: 0;
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px 8px;
  }
  section {
    display: grid;
    gap: 10px;
    margin-top: var(--sp-6);
  }
  .datos {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
    gap: var(--sp-3) var(--sp-5);
    margin: 0;
  }
  .datos dt {
    font-size: var(--fs-xs);
    color: var(--text-3);
  }
  .datos dd {
    margin: 2px 0 0;
    font-size: var(--fs-sm);
    overflow-wrap: anywhere;
  }
  .pie {
    margin: var(--sp-4) 0 0;
    font-size: var(--fs-sm);
  }
  .cab {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
    padding: 10px 16px;
  }
  .enlace {
    display: flex;
    flex: 1 1 260px;
    align-items: center;
    gap: 8px;
    min-width: 0;
    color: inherit;
    text-decoration: none;
  }
  .t {
    display: flex;
    flex: 1;
    flex-direction: column;
    gap: 1px;
    min-width: 0;
    font-size: var(--fs-sm);
    overflow-wrap: anywhere;
  }
  .t .faint,
  .ruta {
    font-size: var(--fs-xs);
  }
  .ruta {
    color: var(--text-2);
  }
  .repos {
    list-style: none;
    margin: 0;
    padding: 0;
    border-top: 1px solid var(--border);
  }
  .repos li {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 16px;
  }
  .repos li + li {
    border-top: 1px solid var(--border);
  }
  .tam {
    flex: none;
    font-size: var(--fs-sm);
  }
  .vacio {
    margin: 0;
    padding: 8px 16px 12px;
    font-size: var(--fs-sm);
  }
  .desc {
    display: grid;
    gap: 8px;
  }
  .desc p {
    margin: 0;
  }
</style>
