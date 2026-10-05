<script lang="ts">
  // Acciones en bloque sobre varios equipos: «Copiar ahora» y «Verificar».
  // Con `copias` («equipo|copia», desde la lista de Copias), solo esas copias.
  // Cada equipo recibe sus propias órdenes, selladas para él como siempre
  // (inofensivas: no piden clave). Primero se dice qué va a pasar; después,
  // la lista de equipos con lo que contesta cada uno. Nada destructivo se
  // ofrece en bloque.
  import { Check, CircleAlert, LoaderCircle, Play, ShieldCheck, WifiOff, X } from "@lucide/svelte";
  import Modal from "$ui/componentes/Modal.svelte";
  import * as api from "$lib/api";
  import { enFondo } from "$lib/actividad.svelte";
  import { mandarOrden } from "$lib/ordenar";
  import { plural } from "$lib/formato";
  import { cargarCliente } from "$lib/estado.svelte";
  import type { Cliente, Equipo, Orden } from "$lib/tipos";
  import BotonCargando from "./BotonCargando.svelte";

  type Accion = "copiar" | "verificar";
  let { cliente, equipos, accion, copias = null, onclose }: { cliente: Cliente; equipos: Equipo[]; accion: Accion; copias?: Set<string> | null; onclose: () => void } = $props();

  interface Tarea {
    tipo: string;
    cuerpo: Record<string, unknown>;
    que: string;
  }
  /** Lo que se pedirá a cada equipo (vacío: no tiene nada que hacer). */
  const plan = $derived(
    equipos.map((e) => {
      const tareas: Tarea[] =
        accion === "copiar"
          ? (e.resumen?.copias ?? []).filter((k) => k.activa !== false && (!copias || copias.has(`${e.id}|${k.id}`))).map((k) => ({ tipo: "copiar_ahora", cuerpo: { repo: k.repo, copia: k.id }, que: k.nombre }))
          : (e.resumen?.repositorios ?? []).filter((r) => !r.solo_lectura && (r.versiones ?? 0) > 0).map((r) => ({ tipo: "verificar_ahora", cuerpo: { repo: r.id }, que: r.nombre }));
      return { equipo: e, tareas };
    }),
  );
  const conTareas = $derived(plan.filter((p) => p.tareas.length));
  const sinTareas = $derived(plan.filter((p) => !p.tareas.length));
  const total = $derived(conTareas.reduce((n, p) => n + p.tareas.length, 0));
  const TITULO = $derived(copias ? { copiar: "Copiar ahora las copias elegidas", verificar: "Verificar varios equipos" } : { copiar: "Copiar ahora en varios equipos", verificar: "Verificar varios equipos" });

  type Estado = { fase: "cola" | "enviando" | "esperando" | "fin"; ordenes: Orden[]; error?: string };
  let estados = $state<Record<string, Estado>>({});
  let enMarcha = $state(false);
  let terminado = $state(false);

  const FINAL = ["hecha", "fallida", "rechazada", "cancelada", "caducada"];

  async function ejecutar() {
    enMarcha = true;
    for (const p of conTareas) estados[p.equipo.id] = { fase: "cola", ordenes: [] };
    // Hasta 3 equipos a la vez; dentro de cada equipo, una orden tras otra (su número crece).
    const cola = [...conTareas];
    const trabajador = async () => {
      while (cola.length) {
        const p = cola.shift()!;
        const est = estados[p.equipo.id];
        est.fase = "enviando";
        let eq = p.equipo;
        try {
          for (const t of p.tareas) {
            const o = await mandarOrden({ cliente, equipo: eq, tipo: t.tipo, cuerpo: t.cuerpo });
            est.ordenes = [...est.ordenes, o];
            eq = { ...eq, siguiente_seq: o.seq + 1 };
          }
          est.fase = "esperando";
        } catch (e) {
          est.error = (e as Error).message;
          est.fase = est.ordenes.length ? "esperando" : "fin";
        }
      }
    };
    await Promise.all([trabajador(), trabajador(), trabajador()]);
    void cargarCliente(cliente.id, { silencioso: true });
    seguir();
  }

  /** Sigue las respuestas (como mucho 3 min aquí; luego siguen en «Órdenes»). */
  function seguir() {
    const inicio = Date.now();
    const t = setInterval(async () => {
      const pendientes = Object.entries(estados).filter(([, s]) => s.fase === "esperando");
      if (!pendientes.length || Date.now() - inicio > 180_000) {
        clearInterval(t);
        terminado = true;
        return;
      }
      await Promise.all(
        pendientes.map(async ([id, s]) => {
          try {
            const l = await enFondo(() => api.ordenesEquipo(cliente.id, id, 30));
            s.ordenes = s.ordenes.map((o) => l.find((x) => x.id === o.id) ?? o);
            if (s.ordenes.every((o) => FINAL.includes(o.estado))) s.fase = "fin";
          } catch {
            /* se reintenta */
          }
        }),
      );
    }, 2500);
  }

  function resumenDe(e: Equipo, s: Estado | undefined): { tono: "ok" | "bad" | "info" | "neutral"; texto: string } {
    if (!s) return { tono: "neutral", texto: "" };
    if (s.fase === "cola") return { tono: "neutral", texto: "En cola" };
    if (s.fase === "enviando") return { tono: "info", texto: "Enviando…" };
    const hechas = s.ordenes.filter((o) => o.estado === "hecha").length;
    const malas = s.ordenes.filter((o) => ["fallida", "rechazada", "caducada"].includes(o.estado));
    if (s.error && !s.ordenes.length) return { tono: "bad", texto: s.error };
    if (malas.length) return { tono: "bad", texto: malas[0].mensaje ?? `${malas.length} sin hacer` };
    if (s.fase === "fin") return { tono: "ok", texto: accion === "copiar" ? `${plural(hechas, "copia pedida", "copias pedidas")}: el equipo la${hechas === 1 ? "" : "s"} está haciendo` : `${plural(hechas, "verificación pedida", "verificaciones pedidas")}` };
    if (!e.conectado) return { tono: "neutral", texto: "Sin conexión: lo hará cuando vuelva" };
    return { tono: "info", texto: `${hechas} de ${s.ordenes.length} recibidas…` };
  }
</script>

<Modal labelledby="t-bloque" {onclose} width={600} dismissible={!enMarcha || terminado}>
  <div class="dlg-title">
    <span class="ticon">{#if accion === "copiar"}<Play size={18} />{:else}<ShieldCheck size={18} />{/if}</span>
    <div>
      <h2 id="t-bloque">{TITULO[accion]}</h2>
      <p>
        {#if accion === "copiar"}{copias ? "Cada equipo hace ahora las copias elegidas, sin esperar a su hora." : "Cada equipo hace ahora sus copias activas, sin esperar a su hora."} No borra nada.
        {:else}Cada equipo comprueba una parte de sus repositorios para confirmar que las copias se pueden leer.{/if}
      </p>
    </div>
  </div>

  {#if !enMarcha}
    <p>
      {#if total}Se {total === 1 ? "pedirá" : "pedirán"} {plural(total, accion === "copiar" ? "copia" : "verificación", accion === "copiar" ? "copias" : "verificaciones")} en {plural(conTareas.length, "equipo", "equipos")}.{:else}Ninguno de los equipos elegidos tiene {accion === "copiar" ? "copias activas" : "repositorios con versiones"}.{/if}
    </p>
    <ul class="lista-b">
      {#each conTareas as p (p.equipo.id)}
        <li><span class="nombre">{p.equipo.nombre}{#if !p.equipo.conectado}<span class="faint sin"><WifiOff size={12} />sin conexión</span>{/if}</span><span class="faint que">{p.tareas.map((t) => t.que).join(", ")}</span></li>
      {/each}
      {#each sinTareas as p (p.equipo.id)}
        <li class="apagado"><span class="nombre">{p.equipo.nombre}</span><span class="faint que">{accion === "copiar" ? "Sin copias activas: se salta" : "Sin versiones que verificar: se salta"}</span></li>
      {/each}
    </ul>
    <footer>
      <button type="button" class="btn btn-ghost" onclick={onclose}>Cancelar</button>
      <BotonCargando class="btn btn-primary" disabled={!total} onclick={ejecutar}>{accion === "copiar" ? "Copiar ahora" : "Verificar"} · {plural(conTareas.length, "equipo", "equipos")}</BotonCargando>
    </footer>
  {:else}
    <ul class="lista-b" aria-live="polite">
      {#each conTareas as p (p.equipo.id)}
        {@const s = estados[p.equipo.id]}
        {@const r = resumenDe(p.equipo, s)}
        <li>
          <span class="ic tone-{r.tono}">
            {#if !s || s.fase === "cola"}<span class="punto"></span>
            {:else if r.tono === "ok"}<Check size={15} />
            {:else if r.tono === "bad"}<CircleAlert size={15} />
            {:else}<LoaderCircle size={15} class="spin" />{/if}
          </span>
          <span class="nombre">{p.equipo.nombre}</span>
          <span class="que" class:mal={r.tono === "bad"}>{r.texto}</span>
          {#if s?.ordenes.length}<a class="link faint ver" href="/c/{cliente.id}/equipos/{p.equipo.id}?tab=ordenes">Ver</a>{/if}
        </li>
      {/each}
    </ul>
    <footer>
      {#if !terminado}<span class="espera"><LoaderCircle size={15} class="spin" />Esperando respuestas… puedes cerrar: siguen en «Órdenes».</span>{/if}
      <button type="button" class="btn btn-primary" onclick={onclose}><X size={15} />Cerrar</button>
    </footer>
  {/if}
</Modal>

<style>
  .lista-b {
    display: flex;
    flex-direction: column;
    max-height: 50vh;
    overflow: auto;
    margin: 0;
    padding: 0;
    list-style: none;
    border: 1px solid var(--border);
    border-radius: var(--radius);
  }
  .lista-b li {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 4px 10px;
    padding: 9px var(--sp-3);
    border-top: 1px solid var(--border);
    font-size: var(--fs-sm);
  }
  .lista-b li:first-child {
    border-top: none;
  }
  .apagado {
    opacity: 0.65;
  }
  .nombre {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    font-weight: 500;
  }
  .sin {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    font-weight: 400;
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
  .ic.tone-ok {
    color: var(--ok);
  }
  .ic.tone-bad {
    color: var(--bad);
  }
  .ic.tone-info {
    color: var(--info);
  }
  .punto {
    width: 7px;
    height: 7px;
    background: var(--border-strong);
    border-radius: 999px;
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
