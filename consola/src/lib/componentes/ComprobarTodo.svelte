<script lang="ts">
  // 16v (plan 0.8.3): «Comprobar todo» de un cliente. Primero dice qué se va a
  // comprobar (y qué no, con el motivo); al pulsar, manda a cada equipo las órdenes de
  // siempre (verificar, verificar la derivada, probar desde el espejo: inofensivas, sin
  // clave ni espera) y va enseñando el veredicto de cada una con lo que cuenta el equipo
  // en su informe. Los kits los comprueba la persona, uno a uno, con la contraseña que
  // escribe (el diálogo de siempre). El informe se puede bajar en CSV en cualquier momento.
  import { untrack } from "svelte";
  import { Check, CircleAlert, Download, KeyRound, ListChecks, LoaderCircle, Minus, TriangleAlert, X } from "@lucide/svelte";
  import Modal from "$ui/componentes/Modal.svelte";
  import * as api from "$lib/api";
  import { enFondo } from "$lib/actividad.svelte";
  import { mandarOrden } from "$lib/ordenar";
  import { cargarCliente, actual, reloj } from "$lib/estado.svelte";
  import { recargarInformes, ultimos } from "$lib/informes.svelte";
  import { cuentas, informeCsv, kitsDe, planComprobar, resultadoDe, type Comprobacion, type Resultado, type Veredicto } from "$lib/comprobarTodo";
  import type { ReglaCopia } from "$lib/regla321";
  import { nombreArchivo } from "$lib/csv";
  import { bajar } from "$lib/informeConfigCargar";
  import { lista, plural, relativo } from "$lib/formato";
  import type { Cliente, Equipo, Orden } from "$lib/tipos";
  import OrdenDialog from "./OrdenDialog.svelte";

  let { cliente, equipos, reglas, filtro = "", onclose }: { cliente: Cliente; equipos: Equipo[]; reglas: ReglaCopia[]; filtro?: string; onclose: () => void } = $props();

  // El plan se fija al abrir (si el resumen cambia mientras tanto, no se mueve la lista).
  const plan: Comprobacion[] = untrack(() => planComprobar(equipos, reglas, actual.equipos));
  const aMandar = plan.filter((c) => c.orden);
  const sinMandar = plan.filter((c) => !c.orden);
  const porClase = (k: Comprobacion["clase"]) => aMandar.filter((c) => c.clase === k).length;
  const sinConexion = [...new Set(aMandar.filter((c) => !c.equipo.conectado).map((c) => c.equipo.nombre))];

  /** Cuándo se pidió (ms), las órdenes mandadas por comprobación y los errores al mandar. */
  let desde = $state<number | null>(null);
  let ordenes = $state<Record<string, Orden>>({});
  let errores = $state<Record<string, string>>({});
  let mandando = $state(false);
  let siguiendo = $state(false);

  const equipoAhora = (id: string) => actual.equipos.find((e) => e.id === id);
  const filas = $derived(
    plan.map((c) => {
      const r: Resultado = errores[c.clave] ? { veredicto: "mal", texto: errores[c.clave], cuando: null } : resultadoDe(c, desde, ultimos.porEquipo[c.equipo.id], equipoAhora(c.equipo.id), ordenes[c.clave]);
      return { c, r };
    }),
  );
  const n = $derived(cuentas(filas.filter((f) => f.c.orden).map((f) => f.r)));
  const kits = $derived(kitsDe(equipos.map((e) => equipoAhora(e.id) ?? e)));
  const kitsPendientes = $derived(kits.filter((k) => k.admite && k.resultado !== "abre"));
  /**
   * Con muchos equipos sin actualizar, una línea por repositorio («Actualiza Resguardo en…»)
   * llenaba la lista: más de 3 se resumen en una sola línea con los equipos.
   */
  const MAX_SIN_SOPORTE = 3;
  const kitsViejos = $derived(kits.filter((k) => !k.admite && !k.resultado));
  const resumirViejos = $derived(kitsViejos.length > MAX_SIN_SOPORTE);
  const kitsALista = $derived(resumirViejos ? kits.filter((k) => k.admite || k.resultado) : kits);
  const equiposViejos = $derived([...new Set(kitsViejos.map((k) => k.equipo.nombre))]);
  const nombresViejos = $derived(equiposViejos.length > 4 ? `${equiposViejos.slice(0, 4).join(", ")} y ${equiposViejos.length - 4} más` : lista(equiposViejos));
  let kit = $state<{ equipo: Equipo; repo: { id: string; nombre: string } } | null>(null);

  async function comprobar() {
    mandando = true;
    desde = Date.now();
    // Por equipo, una orden tras otra (su número crece); hasta 3 equipos a la vez.
    const porEquipo = new Map<string, Comprobacion[]>();
    for (const c of aMandar) porEquipo.set(c.equipo.id, [...(porEquipo.get(c.equipo.id) ?? []), c]);
    const cola = [...porEquipo.values()];
    const trabajador = async () => {
      while (cola.length) {
        const cs = cola.shift()!;
        let eq = equipoAhora(cs[0].equipo.id) ?? cs[0].equipo;
        for (const c of cs) {
          try {
            const o = await mandarOrden({ cliente, equipo: eq, tipo: c.orden!.tipo, cuerpo: c.orden!.cuerpo });
            ordenes[c.clave] = o;
            eq = o.seq === eq.siguiente_seq ? { ...eq, siguiente_seq: o.seq + 1 } : { ...eq, seq_espera: o.seq + 1000 };
          } catch (e) {
            errores[c.clave] = (e as Error).message;
          }
        }
      }
    };
    try {
      await Promise.all([trabajador(), trabajador(), trabajador()]);
    } finally {
      mandando = false;
    }
    seguir();
  }

  /** Sigue las respuestas: cada 10 s, el resumen, los informes nuevos y las órdenes (30 min como mucho aquí). */
  let parar: (() => void) | null = null;
  function seguir() {
    siguiendo = true;
    const inicio = Date.now();
    const t = setInterval(async () => {
      const pendientes = filas.filter((f) => f.r.veredicto === "esperando");
      if (!pendientes.length || Date.now() - inicio > 30 * 60_000) return void parar?.();
      await enFondo(async () => {
        await Promise.all([cargarCliente(cliente.id, { silencioso: true }), recargarInformes(cliente.id)]);
        const equiposConOrden = [...new Set(pendientes.filter((f) => ordenes[f.c.clave]).map((f) => f.c.equipo.id))];
        await Promise.all(
          equiposConOrden.map(async (id) => {
            try {
              const l = await api.ordenesEquipo(cliente.id, id, 50);
              for (const f of pendientes) {
                const o = ordenes[f.c.clave];
                const x = o && l.find((y) => y.id === o.id);
                if (x) ordenes[f.c.clave] = x;
              }
            } catch {
              /* se reintenta */
            }
          }),
        );
      });
    }, 10_000);
    parar = () => {
      clearInterval(t);
      siguiendo = false;
      parar = null;
    };
  }
  $effect(() => () => parar?.());

  function descargar() {
    bajar(informeCsv(filas, kits), "text/csv;charset=utf-8", nombreArchivo(["Resguardo", cliente.nombre, "comprobar todo", new Date().toISOString().slice(0, 10)], "csv"));
  }

  const ICONO: Record<Veredicto, string> = { bien: "ok", aviso: "warn", mal: "bad", esperando: "info", no_se_manda: "neutral" };
  const QUE = { repo: "Repositorio", derivada: "Copia derivada", espejo: "Espejo" } as const;
  const KIT = { abre: "La abre", no_abre: "No la abre", sin_respuesta: "Sin respuesta" } as const;
</script>

<Modal labelledby="t-comprobar" {onclose} width={720} dismissible={!mandando} sePierde="Lo que ya se pidió sigue en marcha en cada equipo: lo verás en cada repositorio.">
  <div class="dlg-title">
    <span class="ticon"><ListChecks size={18} /></span>
    <div>
      <h2 id="t-comprobar">Comprobar todo{filtro ? ` · «${filtro}»` : ""}</h2>
      <p>Cada equipo verifica sus repositorios y sus copias derivadas, y prueba a restaurar unos archivos desde sus espejos a una carpeta temporal. No cambia ni borra nada. Las contraseñas de los kits las compruebas tú, abajo.</p>
    </div>
  </div>

  {#if desde === null}
    <p>
      {#if aMandar.length}Se {aMandar.length === 1 ? "pedirá" : "pedirán"}
        {[porClase("repo") && plural(porClase("repo"), "verificación", "verificaciones"), porClase("derivada") && plural(porClase("derivada"), "verificación de copia derivada", "verificaciones de copias derivadas"), porClase("espejo") && plural(porClase("espejo"), "prueba desde un espejo", "pruebas desde espejos")].filter(Boolean).join(", ")}
        en {plural(new Set(aMandar.map((c) => c.equipo.id)).size, "equipo", "equipos")}. Pueden tardar: una verificación lee una parte de cada repositorio.
      {:else}No hay nada que comprobar en estos equipos todavía.{/if}
    </p>
    {#if sinConexion.length}<p class="faint">Sin conexión ahora: {sinConexion.join(", ")}. Lo harán cuando vuelvan.</p>{/if}
    {#if sinMandar.length}
      <details class="fuera">
        <summary>{plural(sinMandar.length, "cosa no se comprueba", "cosas no se comprueban")} desde aquí</summary>
        <ul class="lista-c">
          {#each sinMandar as c (c.clave)}
            <li><span class="nombre">{c.equipo.nombre} · {c.nombre}</span><span class="faint que">{QUE[c.clase]}: {c.nota}</span></li>
          {/each}
        </ul>
      </details>
    {/if}
  {:else}
    <p class="cuentas" aria-live="polite">
      <span class="tone-ok"><Check size={14} />{n.bien} bien</span>
      {#if n.aviso}<span class="tone-warn"><TriangleAlert size={14} />{n.aviso} con avisos</span>{/if}
      {#if n.mal}<span class="tone-bad"><CircleAlert size={14} />{n.mal} mal</span>{/if}
      {#if n.esperando}<span class="tone-info"><LoaderCircle size={14} class="spin" />{n.esperando} sin terminar</span>{/if}
      <span class="faint">pedido {relativo(new Date(desde).toISOString(), reloj.ahora)}</span>
    </p>
    <ul class="lista-c">
      {#each filas as { c, r } (c.clave)}
        <li class:apagado={r.veredicto === "no_se_manda"}>
          <span class="ic tone-{ICONO[r.veredicto]}">
            {#if r.veredicto === "bien"}<Check size={15} />{:else if r.veredicto === "aviso"}<TriangleAlert size={15} />{:else if r.veredicto === "mal"}<CircleAlert size={15} />{:else if r.veredicto === "esperando"}<LoaderCircle size={15} class="spin" />{:else}<Minus size={15} />{/if}
          </span>
          <span class="nombre">{c.equipo.nombre} · {c.nombre}</span>
          <span class="faint">{QUE[c.clase]}</span>
          <span class="que" class:mal={r.veredicto === "mal"}>{r.texto}</span>
          <a class="link faint ver" href="/c/{cliente.id}/equipos/{c.equipo.id}/repositorios/{c.repo.id}">Ver</a>
        </li>
      {/each}
    </ul>
  {/if}

  <section class="kits" aria-labelledby="t-kits">
    <h3 class="section-title" id="t-kits"><KeyRound size={15} />Kits (contraseñas de los repositorios)</h3>
    {#if !kits.length}
      <p class="faint">Sin repositorios todavía.</p>
    {:else}
      <p class="faint">
        {#if !kits.some((k) => k.admite)}Ningún equipo sabe aún comprobar su kit desde la consola: actualiza Resguardo en ellos.
        {:else if kitsPendientes.length}{plural(kitsPendientes.length, "kit", "kits")} sin comprobar o que no abrieron la última vez. Escribe la contraseña de cada uno: el equipo prueba que abre el repositorio y no la guarda.
        {:else}Todos los kits que se pueden comprobar desde aquí abrieron su repositorio la última vez.{/if}
      </p>
      {#if resumirViejos && kits.some((k) => k.admite)}
        <p class="faint">{plural(kitsViejos.length, "repositorio", "repositorios")} de {plural(equiposViejos.length, "equipo", "equipos")} no se {kitsViejos.length === 1 ? "puede" : "pueden"} comprobar desde aquí: actualiza Resguardo en {nombresViejos}.</p>
      {/if}
      <ul class="lista-c">
        {#each kitsALista as k (`${k.equipo.id}|${k.repo.id}`)}
          <li>
            <span class="ic tone-{k.resultado === 'abre' ? 'ok' : k.resultado ? 'bad' : 'neutral'}">{#if k.resultado === "abre"}<Check size={15} />{:else if k.resultado}<CircleAlert size={15} />{:else}<Minus size={15} />{/if}</span>
            <span class="nombre">{k.equipo.nombre} · {k.repo.nombre}</span>
            <span class="que faint">{k.resultado ? `${KIT[k.resultado]} · ${relativo(k.cuando, reloj.ahora)}` : k.admite ? "Sin comprobar" : `Actualiza Resguardo en «${k.equipo.nombre}» para comprobarlo`}</span>
            {#if k.admite}<button type="button" class="btn btn-sm" onclick={() => (kit = { equipo: k.equipo, repo: { id: k.repo.id, nombre: k.repo.nombre } })}>Comprobar</button>{/if}
          </li>
        {/each}
      </ul>
    {/if}
  </section>

  <footer>
    {#if desde !== null}
      {#if siguiendo}<span class="espera"><LoaderCircle size={15} class="spin" />Mirando cada 10 s… puedes cerrar: siguen en cada equipo.</span>{/if}
      <button type="button" class="btn" onclick={descargar}><Download size={15} />Bajar el informe (CSV)</button>
      <button type="button" class="btn btn-primary" onclick={onclose} disabled={mandando}><X size={15} />Cerrar</button>
    {:else}
      <button type="button" class="btn btn-ghost" onclick={onclose}>Cancelar</button>
      <button type="button" class="btn" onclick={descargar}><Download size={15} />Bajar cómo está ahora</button>
      <button type="button" class="btn btn-primary" disabled={!aMandar.length || mandando} onclick={comprobar}><ListChecks size={15} />Comprobar · {aMandar.length}</button>
    {/if}
  </footer>
</Modal>

{#if kit}
  <OrdenDialog
    {cliente}
    equipo={kit.equipo}
    tipo="comprobar_contrasena"
    cuerpo={{ repo: kit.repo.id }}
    titulo="Comprobar la contraseña de «{kit.repo.nombre}»"
    descripcion="Escribe la contraseña de tu kit. {kit.equipo.nombre} probará si abre «{kit.repo.nombre}» de verdad, y sus copias derivadas con la misma contraseña. No cambia nada ni guarda la contraseña."
    repo={kit.repo}
    accion="Comprobar"
    alTerminar={() => void cargarCliente(cliente.id, { silencioso: true })}
    onclose={() => (kit = null)}
  />
{/if}

<style>
  .lista-c {
    display: flex;
    flex-direction: column;
    max-height: 42vh;
    overflow: auto;
    margin: 0;
    padding: 0;
    list-style: none;
    border: 1px solid var(--border);
    border-radius: var(--radius);
  }
  .lista-c li {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 4px 10px;
    padding: 8px var(--sp-3);
    border-top: 1px solid var(--border);
    font-size: var(--fs-sm);
  }
  .lista-c li:first-child {
    border-top: none;
  }
  .apagado {
    opacity: 0.65;
  }
  .nombre {
    font-weight: 500;
    overflow-wrap: anywhere;
  }
  .que {
    flex: 1;
    min-width: 0;
    color: var(--text-2);
    overflow-wrap: anywhere;
  }
  .que.mal {
    color: var(--bad);
  }
  .ic {
    display: grid;
    place-items: center;
    width: 18px;
    color: var(--text-3);
  }
  .ic.tone-ok,
  .tone-ok {
    color: var(--ok);
  }
  .ic.tone-bad,
  .tone-bad {
    color: var(--bad);
  }
  .ic.tone-warn,
  .tone-warn {
    color: var(--warn);
  }
  .ic.tone-info,
  .tone-info {
    color: var(--info);
  }
  .cuentas {
    display: flex;
    flex-wrap: wrap;
    gap: 6px 14px;
    align-items: center;
  }
  .cuentas span {
    display: inline-flex;
    align-items: center;
    gap: 4px;
  }
  .fuera summary {
    cursor: pointer;
    margin-bottom: var(--sp-2);
    color: var(--text-2);
  }
  .kits {
    margin-top: var(--sp-4);
  }
  .espera {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    margin-right: auto;
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
</style>
