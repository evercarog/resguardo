<script lang="ts">
  import { tip } from "$lib/tooltip";
  // «Primeros pasos» de un cliente nuevo, en Estado. Cada paso se marca solo,
  // a partir de lo que ya hay (no de lo que se ha pulsado): equipos, quién
  // guarda copias, las copias hechas y el kit de cada repositorio (según la
  // protección que informa el agente). Se puede ocultar en este navegador.
  import { Check, ChevronRight, Copy, X } from "@lucide/svelte";
  import type { Cliente, Equipo, Informe, RespaldoConsola } from "$lib/tipos";
  import * as api from "$lib/api";
  import { avisar } from "$lib/avisos.svelte";
  import { app, puede, urlAgentes } from "$lib/estado.svelte";
  import { primeraCopiaFrase } from "$lib/salud";
  import Ilustracion from "$ui/componentes/Ilustracion.svelte";

  let { cliente, equipos, informes }: { cliente: Cliente; equipos: Equipo[]; informes: Record<string, Informe | null> } = $props();

  const c = $derived(cliente.id);
  const confirmados = $derived(equipos.filter((e) => e.confirmado && e.modo !== "trasladado"));
  const repos = $derived(confirmados.flatMap((e) => (e.resumen?.repositorios ?? []).map((r) => ({ e, r }))));
  const almacen = $derived(confirmados.find((e) => e.resumen?.guarda_copias?.activo));
  const fuera = $derived(confirmados.some((e) => (e.resumen?.destinos ?? []).some((d) => ["b2", "s3", "sftp", "rest"].includes(d.tipo) && !d.equipo_almacen)));
  const copiaHecha = $derived(confirmados.some((e) => (e.resumen?.copias ?? []).some((k) => k.ultima && k.ultima.estado !== "fallo")) || repos.some((x) => (x.r.versiones ?? 0) > 0));
  /** El kit de cada repositorio, según la protección del último informe de su equipo. */
  const kits = $derived.by(() => {
    if (!repos.length) return false;
    return repos.every(({ e, r }) => informes[e.id]?.datos.repos?.find((x) => x.id === r.id)?.proteccion?.items.find((i) => i.id === "kit")?.estado === "ok");
  });

  /** Un equipo que ya tiene copias programadas (para el paso «La primera copia»). */
  const conCopias = $derived(confirmados.find((e) => (e.resumen?.copias ?? []).length));
  const comando = $derived(`resguardo-agente vincular CÓDIGO --servidor ${urlAgentes()}`);
  // v1.23: al propietario del servidor, también la copia de la consola (null: servidor anterior).
  let respaldo = $state<RespaldoConsola | null | undefined>(undefined);
  $effect(() => {
    if (!app.cuenta?.superusuario) return;
    api
      .respaldoConsola()
      .then((x) => (respaldo = x))
      .catch(() => (respaldo = null));
  });
  const pasoConsola = $derived(
    app.cuenta?.superusuario && respaldo
      ? [
          {
            id: "consola",
            hecho: !!respaldo.clave_puesta && respaldo.activo,
            titulo: "Copia de la consola",
            texto: "Pon la clave de respaldo de la consola e imprime su kit: cada noche este servidor guardará una copia cifrada de sí mismo, para restaurarlo en otra máquina sin volver a vincular los equipos.",
            accion: { texto: "Ponerla", href: `/c/${c}/servidor#copia-consola` },
          },
        ]
      : [],
  );
  const pasos = $derived([
    {
      id: "cliente",
      hecho: true,
      titulo: "Crear el cliente",
      texto: `«${cliente.nombre}» ya existe. Su clave de administración se elige al emparejar el primer equipo: guárdala en un gestor de contraseñas o imprímela.`,
    },
    {
      id: "equipo",
      hecho: confirmados.length > 0,
      titulo: "Emparejar el primer equipo",
      texto: "Instala Resguardo Agente en el equipo, como administrador, y escribe el código que te da la consola. En Linux o Mac, con esta línea:",
      comando: true,
      accion: { texto: "Añadir equipo", href: `/c/${c}/emparejar` },
    },
    app.servidor?.agente_local && !almacen && !fuera
      ? {
          // v1.19: el servidor se instaló con «Este equipo también guarda copias».
          id: "destino",
          hecho: false,
          titulo: "Usar este servidor como almacén",
          texto: "Resguardo Agente ya está instalado en el equipo del servidor. Vincúlalo con la clave de administración y elige la carpeta donde guardará las copias de los demás.",
          accion: { texto: "Vincular este servidor", href: `/c/${c}/emparejar?local=1` },
        }
      : {
          id: "destino",
          hecho: !!almacen || fuera,
          titulo: "Dónde guardar las copias",
          texto: "Un equipo de la oficina que guarde las copias de los demás («Este equipo guarda copias», con su espejo en otro disco o en la nube), o un destino en la nube.",
          accion: confirmados[0] ? { texto: "Ir al equipo", href: `/c/${c}/equipos/${confirmados[0].id}` } : undefined,
        },
    {
      id: "copia",
      hecho: copiaHecha,
      titulo: "La primera copia",
      texto: conCopias
        ? `${primeraCopiaFrase(conCopias.resumen?.copias ?? [])} Al terminar, aquí se marcará como hecha.`
        : "Crea un repositorio, elige las carpetas y el horario, y pulsa «Copiar ahora» para no esperar a la hora.",
      accion: conCopias
        ? { texto: `Ver ${conCopias.nombre}`, href: `/c/${c}/equipos/${conCopias.id}` }
        : confirmados[0]
          ? { texto: "Crear la copia", href: `/c/${c}/equipos/${confirmados[0].id}/copias` }
          : undefined,
    },
    {
      id: "kits",
      hecho: kits,
      titulo: "Imprimir los kits de recuperación",
      texto: "Uno por repositorio: con él se abren las copias en cualquier equipo si se pierde este. Se muestra al crear el repositorio; en el equipo también se puede ver con «resguardo-agente kit <repositorio>».",
      accion: { texto: "Qué es el kit", href: "/ayuda#kit" },
    },
    ...pasoConsola,
  ]);
  const hechos = $derived(pasos.filter((p) => p.hecho).length);
  const siguiente = $derived(pasos.find((p) => !p.hecho)?.id);

  const CLAVE = $derived(`resguardo-primeros-pasos-oculto|${c}`);
  let oculto = $state(false);
  $effect(() => {
    try {
      oculto = localStorage.getItem(CLAVE) === "1";
    } catch {
      oculto = false;
    }
  });
  // «Primera copia hecha»: se celebra una vez, solo si este navegador vio el
  // cliente sin copias (así no salta en clientes que ya las tenían).
  const CLAVE_SIN_COPIA = $derived(`resguardo-sin-primera-copia|${c}`);
  let celebrar = $state(false);
  $effect(() => {
    try {
      if (!copiaHecha) localStorage.setItem(CLAVE_SIN_COPIA, "1");
      else celebrar = localStorage.getItem(CLAVE_SIN_COPIA) === "1";
    } catch {
      celebrar = false;
    }
  });
  function cerrarCelebracion() {
    celebrar = false;
    try {
      localStorage.removeItem(CLAVE_SIN_COPIA);
    } catch {
      /* sin almacenamiento */
    }
  }

  function ocultar() {
    oculto = true;
    try {
      localStorage.setItem(CLAVE, "1");
    } catch {
      /* sin almacenamiento */
    }
  }
</script>

{#if celebrar}
  <section class="card logro" aria-labelledby="t-logro">
    <Ilustracion nombre="primera-copia" ancho={112} />
    <div>
      <h2 id="t-logro">Primera copia hecha</h2>
      <p>Los datos de {cliente.nombre} ya están guardados y cifrados. A partir de ahora se copiarán solos, a su hora.</p>
    </div>
    <button class="btn btn-sm" onclick={cerrarCelebracion}>Entendido</button>
  </section>
{/if}

{#if hechos < pasos.length && !oculto && puede.administrar(cliente.rol)}
  <section class="card pasos" aria-labelledby="t-primeros">
    <header>
      {#if hechos <= 1}<Ilustracion nombre="bienvenida" ancho={88} />{/if}
      <div>
        <h2 id="t-primeros">Primeros pasos</h2>
        <p class="faint">{hechos} de {pasos.length} hechos. Cada paso se marca solo cuando está listo.</p>
      </div>
      <div class="barra" role="progressbar" aria-valuemin="0" aria-valuemax={pasos.length} aria-valuenow={hechos} aria-label="Progreso de los primeros pasos">
        <span style:width="{(hechos / pasos.length) * 100}%"></span>
      </div>
      <button class="icon-btn" aria-label="Ocultar los primeros pasos" use:tip={"Ocultar"} onclick={ocultar}><X size={15} /></button>
    </header>
    <ol>
      {#each pasos as p, i (p.id)}
        <li class:hecho={p.hecho} class:actual={p.id === siguiente}>
          <span class="n" aria-hidden="true">{#if p.hecho}<Check size={13} />{:else}{i + 1}{/if}</span>
          <div class="cuerpo">
            <strong>{p.titulo}<span class="sr-only">{p.hecho ? ": hecho" : ": pendiente"}</span></strong>
            {#if p.id === siguiente}
              <p>{p.texto}</p>
              {#if p.comando}
                <div class="cmd">
                  <code class="selectable">{comando}</code>
                  <button
                    class="btn btn-sm"
                    onclick={async () => {
                      await navigator.clipboard.writeText(comando);
                      avisar("Línea copiada.");
                    }}><Copy size={14} />Copiar</button
                  >
                </div>
              {/if}
              {#if p.accion}<a class="btn btn-sm btn-primary" href={p.accion.href}>{p.accion.texto}<ChevronRight size={14} /></a>{/if}
            {/if}
          </div>
        </li>
      {/each}
    </ol>
  </section>
{/if}

<style>
  .pasos {
    display: flex;
    flex-direction: column;
    gap: var(--sp-4);
    padding: var(--sp-5);
  }
  header {
    display: flex;
    align-items: center;
    gap: var(--sp-4);
  }
  header > div:first-child {
    flex: 1;
    min-width: 0;
  }
  h2 {
    margin: 0;
    font-size: var(--fs-h2);
    line-height: var(--lh-h2);
    font-weight: 600;
  }
  header p {
    margin: 2px 0 0;
    font-size: var(--fs-sm);
  }
  .barra {
    flex: none;
    width: 120px;
    height: 6px;
    overflow: hidden;
    background: var(--surface-3);
    border-radius: 999px;
  }
  .barra span {
    display: block;
    height: 100%;
    background: var(--accent);
    border-radius: 999px;
    transition: width var(--dur-slow) var(--ease-out);
  }
  ol {
    display: flex;
    flex-direction: column;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  li {
    display: flex;
    gap: var(--sp-3);
    padding: 10px 0;
    border-top: 1px solid var(--border);
  }
  li:first-child {
    border-top: none;
  }
  .n {
    display: grid;
    flex: none;
    place-items: center;
    width: 24px;
    height: 24px;
    font-size: var(--fs-xs);
    font-weight: 600;
    color: var(--text-2);
    background: var(--surface-3);
    border-radius: 999px;
  }
  .hecho .n {
    color: var(--accent-contrast);
    background: var(--ok);
  }
  .actual .n {
    color: var(--accent-contrast);
    background: var(--accent);
  }
  .cuerpo {
    display: flex;
    flex: 1;
    flex-direction: column;
    align-items: flex-start;
    gap: 8px;
    min-width: 0;
    padding-top: 2px;
  }
  .cuerpo strong {
    font-weight: 500;
  }
  .hecho .cuerpo strong {
    color: var(--text-3);
    text-decoration: line-through;
    text-decoration-color: var(--border-strong);
  }
  .cuerpo p {
    margin: 0;
    font-size: var(--fs-sm);
    line-height: var(--lh-sm);
    color: var(--text-2);
  }
  .cmd {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
    width: 100%;
  }
  .cmd code {
    flex: 1;
    min-width: 0;
    padding: 6px 8px;
    font-size: 12px;
    word-break: break-all;
    background: var(--surface-2);
    border-radius: var(--radius-sm);
  }
  .logro {
    display: flex;
    align-items: center;
    gap: var(--sp-4);
    padding: var(--sp-4) var(--sp-5);
  }
  .logro > div {
    flex: 1;
    min-width: 0;
  }
  .logro p {
    margin: 2px 0 0;
    font-size: var(--fs-sm);
    line-height: var(--lh-sm);
    color: var(--text-2);
  }
  @media (max-width: 560px) {
    .barra {
      display: none;
    }
    .logro {
      flex-wrap: wrap;
    }
    header :global(.il) {
      display: none;
    }
  }
</style>
