<script lang="ts">
  // Los espejos de un equipo (plan 0.7.26, bloque 4): los que hace el almacén con lo
  // que guarda o los que hace el propio equipo con los repositorios de sus discos.
  // Cada uno en una tarjeta con su resumen y cómo fue; se ordenan, se pausan, se
  // cambian con el editor guiado y se quitan. Cada cambio manda la lista entera al
  // equipo con la clave de administración (lo que reduce la protección, con la espera).
  import { ArrowDown, ArrowUp, CornerDownRight, Plus, ShieldAlert } from "@lucide/svelte";
  import OrdenDialog from "../OrdenDialog.svelte";
  import MenuAcciones from "../MenuAcciones.svelte";
  import Chip from "../Chip.svelte";
  import Tiempo from "../Tiempo.svelte";
  import TipoDestino from "../TipoDestino.svelte";
  import EditorEspejo from "./EditorEspejo.svelte";
  import { tip } from "$lib/tooltip";
  import { puede } from "$lib/estado.svelte";
  import { bytes, plural } from "$lib/formato";
  import { catalogoDe, cargarCatalogo } from "$lib/catalogoDestinos.svelte";
  import { destinosParaPasos } from "$lib/cadenas";
  import { nombreZonaPorDefecto, PRINCIPAL, zonasDe } from "$lib/destinos";
  import { clasificacionDeVista } from "$lib/regla321";
  import { diaLegible } from "$lib/espejo";
  import { corta, tipoDeNube } from "$lib/tipoDestino";
  import {
    AVISO_IGUAL,
    conTrabajo,
    cuerpoAlmacen,
    cuerpoEquipo,
    equiposDelAlmacen,
    estadoTrabajo,
    mover,
    nombrePorDefecto,
    opcionesRepos,
    paraOrden,
    sinTrabajo,
    textoCuando,
    textoQue,
    textoRetencion,
    trabajoNuevo,
    trabajosDelAlmacen,
    trabajosDelEquipo,
    type TrabajoEspejo,
    type TrabajoEspejoResumen,
  } from "$lib/espejoTrabajos";
  import type { Cliente, Equipo } from "$lib/tipos";

  interface Props {
    cliente: Cliente;
    /** Quién los hace. */
    hace: Equipo;
    quien: "almacen" | "equipo";
    equipos: Equipo[];
    /** Abrir el editor ya al llegar: uno nuevo con esto elegido, o uno que ya está. */
    inicial?: { nuevo?: Partial<TrabajoEspejo> | null; editar?: string | null } | null;
    /** Cuando el equipo responde (para recargar su resumen). */
    alTerminar?: () => void;
  }
  let { cliente, hace, quien, equipos, inicial = null, alTerminar }: Props = $props();

  const lista = $derived<TrabajoEspejoResumen[]>([...(quien === "almacen" ? trabajosDelAlmacen(hace) : trabajosDelEquipo(hace))].sort((a, b) => a.orden - b.orden));
  const admin = $derived(puede.administrar(cliente.rol));

  // svelte-ignore state_referenced_locally
  void cargarCatalogo(cliente.id);
  const vistas = $derived(destinosParaPasos(equipos, catalogoDe(cliente.id)));
  const zonas = $derived(quien === "almacen" ? zonasDe(hace) : []);
  const nombreRepo = (t: TrabajoEspejo) => (r: string) => {
    const o = opcionesRepos(hace, quien, equipos, t.zona || PRINCIPAL).find((x) => x.valor === r);
    return o ? (o.equipo ? `${o.nombre} (${o.equipo})` : o.nombre) : r;
  };
  const nombreEquipo = (u: string) => equiposDelAlmacen(hace, equipos).find((x) => x.usuario === u)?.nombre ?? u;
  const nombreDe = (id: string) => {
    const x = lista.find((y) => y.id === id);
    return x ? x.nombre || nombrePorDefecto(x.adonde) : "otro espejo";
  };
  /** Adónde va, con su nombre del catálogo y su tipo (icono). */
  function destino(t: TrabajoEspejo) {
    if (t.adonde.tipo === "zona") {
      const z = zonas.find((x) => x.id === t.adonde.carpeta);
      const v = vistas.find((x) => x.zona && x.zona.almacen.id === hace.id && x.zona.id === t.adonde.carpeta);
      return { nombre: v?.nombre ?? (z ? nombreZonaPorDefecto(z) : "Otra zona"), tipo: v ? corta(clasificacionDeVista(v, equipos)) : null };
    }
    if (t.adonde.tipo === "nube") {
      const v = vistas.find((x) => x.nube && x.nube.equipo.id === hace.id && x.nube.nombre === t.adonde.nube);
      const tipo = [...(hace.resumen?.nubes ?? []), ...(hace.resumen?.guarda_copias?.nubes ?? [])].find((n) => n.nombre === t.adonde.nube)?.tipo;
      return { nombre: `${v?.nombre ?? t.adonde.nube} · ${t.adonde.carpeta}`, tipo: v ? corta(clasificacionDeVista(v, equipos)) : tipo ? tipoDeNube(tipo) : null };
    }
    return { nombre: t.adonde.carpeta, tipo: { tipo: "local" as const, inmutable: false, aislado: false, bloqueoDias: null } };
  }
  const TONO = { ok: "ok", error: "bad", pausado: "paused", nuevo: "info" } as const;
  const TEXTO = { ok: "Al día", error: "Con error", pausado: "Pausado", nuevo: "Aún no se ha hecho" } as const;

  // --- Editor y órdenes ---
  let editor = $state<{ trabajo: TrabajoEspejo; nuevo: boolean } | null>(null);
  let orden = $state<{ cuerpo: Record<string, unknown>; titulo: string; descripcion: string } | null>(null);
  const cuerpo = (ts: TrabajoEspejo[]) => (quien === "almacen" ? cuerpoAlmacen(ts) : cuerpoEquipo(ts));
  const base = $derived(lista.map(paraOrden));

  function abrirNuevo(pre?: Partial<TrabajoEspejo> | null) {
    const t = trabajoNuevo(quien, lista.length);
    editor = { trabajo: { ...t, ...(pre ?? {}), id: t.id, orden: lista.length }, nuevo: true };
  }
  function abrirEditar(t: TrabajoEspejo) {
    editor = { trabajo: paraOrden(t), nuevo: false };
  }
  // Lo pedido al llegar (una vez).
  // svelte-ignore state_referenced_locally
  if (inicial?.nuevo && admin) abrirNuevo(inicial.nuevo);
  // svelte-ignore state_referenced_locally
  else if (inicial?.editar && admin) {
    const t = lista.find((x) => x.id === inicial.editar);
    if (t) abrirEditar(t);
  }

  function guardado(t: TrabajoEspejo) {
    const nuevo = editor?.nuevo;
    editor = null;
    const nombre = t.nombre || nombrePorDefecto(t.adonde);
    orden = {
      cuerpo: cuerpo(conTrabajo(base, t)),
      titulo: nuevo ? "Añadir un espejo" : `Cambiar «${nombre}»`,
      descripcion: `${quien === "almacen" ? hace.nombre : `${hace.nombre} (el propio equipo)`} hará «${nombre}». Qué: ${textoQue(t, nombreRepo(t), nombreEquipo)} → ${destino(t).nombre}. Cuándo: ${textoCuando(t, nombreDe)}. ${textoRetencion(t)}.${t.retencion.modo === "igual" ? ` ${AVISO_IGUAL}` : ""}`,
    };
  }
  function moverUno(t: TrabajoEspejo, d: -1 | 1) {
    orden = { cuerpo: cuerpo(mover(base, t.id, d)), titulo: "Cambiar el orden de los espejos", descripcion: "Cuando tocan varios a la vez, se hacen en este orden." };
  }
  function pausar(t: TrabajoEspejo) {
    const pausa = t.activo;
    orden = {
      cuerpo: cuerpo(conTrabajo(base, { ...paraOrden(t), activo: !pausa })),
      titulo: pausa ? `Pausar «${t.nombre}»` : `Activar «${t.nombre}»`,
      descripcion: pausa ? "Deja de hacerse hasta que lo actives (lo ya copiado se queda). Reduce la protección: espera su turno." : "Vuelve a hacerse cuando toque.",
    };
  }
  function quitar(t: TrabajoEspejo) {
    orden = {
      cuerpo: cuerpo(sinTrabajo(base, t.id)),
      titulo: `Quitar «${t.nombre}»`,
      descripcion: `Deja de hacerse. Lo ya copiado se queda en su destino.${lista.some((x) => x.cuando.cadena === t.id || x.cuando.despues === t.id) ? " Los que iban después de él pasan a hacerse después de cada copia nueva." : ""}`,
    };
  }
  function duplicar(t: TrabajoEspejo) {
    const c = trabajoNuevo(quien, lista.length);
    editor = { trabajo: { ...paraOrden(t), id: c.id, orden: lista.length, nombre: `${t.nombre} (copia)` }, nuevo: true };
  }
  function ahora(t: TrabajoEspejo) {
    orden = {
      cuerpo: { espejo_ahora: { trabajo: t.id, ...(quien === "equipo" ? { quien: "equipo" } : {}) } },
      titulo: `Hacer ahora «${t.nombre}»`,
      descripcion: "Empieza en cuanto pueda, sin esperar a su hora. Solo copia lo que falta: no cambia nada más.",
    };
  }
  function confirmarFreno(t: TrabajoEspejoResumen) {
    orden = {
      cuerpo: { espejo_freno: { trabajo: t.id, ...(quien === "equipo" ? { quien: "equipo" } : {}) } },
      titulo: `Confirmar el freno de «${t.nombre}»`,
      descripcion: `Hazlo solo si sabes por qué falta (una poda grande o un repositorio que quitaste). La próxima vez que se haga se anota y se borrará del espejo ${t.retencion.modo === "retraso" ? `pasados ${t.retencion.dias} días` : "la vez siguiente"}. Si no lo sabes, revisa antes el original.`,
    };
  }
</script>

<div class="lista-espejos">
  {#each lista as t, j (t.id)}
    {@const e = estadoTrabajo(t)}
    {@const d = destino(t)}
    {@const despues = t.cuando.cadena || t.cuando.despues}
    <article class="card espejo" class:pausado={!t.activo} class:tras={!!despues}>
      {#if despues}<p class="tras-de faint"><CornerDownRight size={13} />{t.cuando.cadena ? "En cadena tras" : "Después de"} «{nombreDe(despues)}»</p>{/if}
      <div class="cab">
        <h3>{t.nombre || nombrePorDefecto(t.adonde)}</h3>
        <Chip tono={TONO[e]} texto={TEXTO[e]} pequeno />
        {#if admin}
          <div class="acciones">
            <button class="icon-btn" use:tip={"Subir"} aria-label="Subir «{t.nombre}»" disabled={j === 0} onclick={() => moverUno(t, -1)}><ArrowUp size={14} /></button>
            <button class="icon-btn" use:tip={"Bajar"} aria-label="Bajar «{t.nombre}»" disabled={j === lista.length - 1} onclick={() => moverUno(t, 1)}><ArrowDown size={14} /></button>
            <button class="btn btn-sm" onclick={() => abrirEditar(t)}>Cambiar</button>
            <MenuAcciones
              etiqueta="Más acciones de «{t.nombre}»"
              grupos={[
                [
                  ...(t.activo ? [{ texto: "Hacer ahora", onclick: () => ahora(t) }] : []),
                  { texto: t.activo ? "Pausar" : "Activar", onclick: () => pausar(t) },
                  { texto: "Duplicar", onclick: () => duplicar(t) },
                ],
                [{ texto: "Quitar", peligro: true, onclick: () => quitar(t) }],
              ]}
            />
          </div>
        {/if}
      </div>
      <p class="linea">
        <span>{textoQue(t, nombreRepo(t), nombreEquipo)}</span>
        <span class="flecha" aria-hidden="true">→</span>
        <span class="dest">{#if d.tipo}<TipoDestino {...d.tipo} />{/if}{d.nombre}</span>
      </p>
      <p class="linea faint">
        <span>{textoCuando(t, nombreDe)}</span><span aria-hidden="true">·</span><span class:igual={t.retencion.modo === "igual"}>{textoRetencion(t)}</span>
        {#if t.limite_kib}<span aria-hidden="true">·</span><span>hasta {t.limite_kib} KiB/s</span>{/if}
      </p>
      {#if t.retencion.modo === "igual"}<p class="aviso-igual"><ShieldAlert size={13} />{AVISO_IGUAL}</p>{/if}
      <p class="estado faint">
        {#if t.ultima}La última, <Tiempo iso={t.ultima} />{:else}Todavía no se ha hecho{/if}{#if t.proxima && t.activo}{" · "}la próxima por horario <Tiempo iso={t.proxima} />{/if}
        {#if t.verificacion}{" · "}comprobados {plural(t.verificacion.archivos, "archivo", "archivos")}{t.verificacion.mal ? `, ${t.verificacion.mal} mal` : ""}{/if}
        {#if t.por_borrar?.archivos}{" · "}{plural(t.por_borrar.archivos, "archivo", "archivos")} ({bytes(t.por_borrar.bytes)}) por borrar{t.por_borrar.primero ? ` desde el ${diaLegible(t.por_borrar.primero)}` : ""}{/if}
      </p>
      {#if e === "error" && t.resultado}<p class="msg-fallo">{t.resultado}</p>{/if}
      {#if t.freno_aviso || t.retenidos}
        <div class="notice notice-warn freno">
          <ShieldAlert size={16} />
          <div>
            <p><strong>Freno:</strong> {t.freno_aviso ?? `se conservan ${plural(t.retenidos?.archivos ?? 0, "archivo", "archivos")} que faltan en el original.`}</p>
            {#if admin && t.retencion.modo !== "nunca"}<button class="btn btn-sm" onclick={() => confirmarFreno(t)}>Confirmar…</button>{/if}
          </div>
        </div>
      {/if}
    </article>
  {:else}
    <p class="faint vacio">{quien === "almacen" ? "Ningún espejo todavía. Un espejo copia lo que guarda este almacén a otro disco o a una nube, tal cual (sin contraseñas)." : "Ningún espejo todavía. Este equipo puede copiar los repositorios de sus discos a otro disco o a una nube conectada aquí."}</p>
  {/each}
  {#if admin}
    <div class="anadir"><button class="btn btn-primary" onclick={() => abrirNuevo()}><Plus size={16} />Añadir espejo</button></div>
  {/if}
</div>

{#if editor}
  <EditorEspejo {cliente} {hace} {quien} {equipos} trabajo={editor.trabajo} todos={base} nuevo={editor.nuevo} onguardar={guardado} onclose={() => (editor = null)} />
{/if}
{#if orden}
  <OrdenDialog
    {cliente}
    equipo={hace}
    tipo="guarda_copias"
    cuerpo={orden.cuerpo}
    titulo={orden.titulo}
    descripcion={orden.descripcion}
    onclose={() => (orden = null)}
    alTerminar={() => alTerminar?.()}
  />
{/if}

<style>
  .lista-espejos {
    display: grid;
    gap: 10px;
  }
  .espejo {
    padding: 12px 14px;
    display: grid;
    gap: 4px;
  }
  .espejo.tras {
    margin-left: 22px;
  }
  .espejo.pausado {
    opacity: 0.75;
  }
  .tras-de {
    margin: 0;
    display: inline-flex;
    gap: 4px;
    align-items: center;
    font-size: var(--fs-xs);
  }
  .cab {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
  }
  .cab h3 {
    margin: 0;
    font-size: var(--fs-md, 1rem);
    overflow-wrap: anywhere;
  }
  .acciones {
    margin-left: auto;
    display: inline-flex;
    align-items: center;
    gap: 4px;
  }
  .linea {
    margin: 0;
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    align-items: center;
    font-size: var(--fs-sm);
    overflow-wrap: anywhere;
  }
  .dest {
    display: inline-flex;
    gap: 6px;
    align-items: center;
    min-width: 0;
  }
  .flecha {
    color: var(--text-muted);
  }
  .igual {
    color: var(--warn-text, var(--text));
    font-weight: 600;
  }
  .aviso-igual {
    margin: 0;
    display: inline-flex;
    gap: 4px;
    align-items: center;
    font-size: var(--fs-xs);
    color: var(--warn-text, var(--text-muted));
  }
  .estado {
    margin: 0;
    font-size: var(--fs-xs);
  }
  .freno div {
    display: grid;
    gap: 6px;
    justify-items: start;
  }
  .msg-fallo {
    margin: 2px 0 0;
    font-size: var(--fs-sm);
    color: var(--danger-text, var(--danger));
    overflow-wrap: anywhere;
  }
  .freno p {
    margin: 0;
  }
  .vacio {
    margin: 0;
  }
  .anadir {
    display: flex;
  }
  @media (max-width: 520px) {
    .espejo.tras {
      margin-left: 10px;
    }
    .acciones {
      margin-left: 0;
      width: 100%;
    }
  }
</style>
