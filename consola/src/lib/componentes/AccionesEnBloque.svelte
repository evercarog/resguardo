<script lang="ts">
  // Acciones en bloque sobre varios equipos: «Copiar ahora», «Verificar» y
  // (v1.52, tarea 6) «Pausar» y «Reanudar». Con `copias` («equipo|copia»,
  // desde la lista de Copias), solo esas copias. Cada equipo recibe sus propias
  // órdenes, selladas para él como siempre. Copiar, verificar y reanudar son
  // inofensivas (no piden clave). Pausar pide la clave de administración (se
  // calcula una vez para el cliente y se comprueba equipo a equipo) y, como en
  // la ficha, espera la espera mínima: se puede cancelar en «Órdenes». Primero
  // se dice qué va a pasar; después, la lista de equipos con lo que contesta
  // cada uno. Nada que borre se ofrece en bloque.
  import { Check, CircleAlert, CirclePause, CirclePlay, LoaderCircle, Play, ShieldCheck, WifiOff, X } from "@lucide/svelte";
  import Modal from "$ui/componentes/Modal.svelte";
  import * as api from "$lib/api";
  import { enFondo } from "$lib/actividad.svelte";
  import { borrar } from "$lib/cripto/bytes";
  import { etiquetaValida } from "$lib/cripto/claves";
  import { kcfgDelCliente, mandarOrden, pruebaParaEquipo } from "$lib/ordenar";
  import { plural, relativo } from "$lib/formato";
  import { cargarCliente } from "$lib/estado.svelte";
  import type { Cliente, Equipo, Orden } from "$lib/tipos";
  import BotonCargando from "./BotonCargando.svelte";
  import CampoClave from "./CampoClave.svelte";

  type Accion = "copiar" | "verificar" | "pausar" | "reanudar";
  let { cliente, equipos, accion, copias = null, onclose }: { cliente: Cliente; equipos: Equipo[]; accion: Accion; copias?: Set<string> | null; onclose: () => void } = $props();

  interface Tarea {
    tipo: string;
    cuerpo: Record<string, unknown>;
    que: string;
  }
  /** Pausar: cuántas horas (0 = hasta reanudar), como en la ficha del equipo. */
  let horas = $state(24);
  let clave = $state("");
  let errorClave = $state("");
  let preparando = $state(false);
  const HORAS: [number, string][] = [
    [4, "4 horas"],
    [24, "1 día"],
    [72, "3 días"],
    [168, "1 semana"],
    [0, "Hasta que las reanude"],
  ];
  const activas = (e: Equipo) => (e.resumen?.copias ?? []).filter((k) => k.activa !== false);
  /** Lo que se pedirá a cada equipo (vacío: no tiene nada que hacer). */
  const plan = $derived(
    equipos.map((e) => {
      let tareas: Tarea[] = [];
      if (accion === "copiar") tareas = activas(e).filter((k) => !copias || copias.has(`${e.id}|${k.id}`)).map((k) => ({ tipo: "copiar_ahora", cuerpo: { repo: k.repo, copia: k.id }, que: k.nombre }));
      else if (accion === "verificar") tareas = (e.resumen?.repositorios ?? []).filter((r) => !r.solo_lectura && (r.versiones ?? 0) > 0).map((r) => ({ tipo: "verificar_ahora", cuerpo: { repo: r.id }, que: r.nombre }));
      else if (accion === "pausar") tareas = activas(e).length ? [{ tipo: "pausar", cuerpo: { repo: "", horas }, que: plural(activas(e).length, "copia automática", "copias automáticas") }] : [];
      else tareas = e.resumen?.pausado_hasta ? [{ tipo: "reanudar", cuerpo: { repo: "" }, que: e.resumen.pausado_hasta === "indefinido" ? "en pausa hasta que alguien la reanude" : `en pausa hasta ${relativo(e.resumen.pausado_hasta)}` }] : [];
      return { equipo: e, tareas };
    }),
  );
  const conTareas = $derived(plan.filter((p) => p.tareas.length));
  const sinTareas = $derived(plan.filter((p) => !p.tareas.length));
  const total = $derived(conTareas.reduce((n, p) => n + p.tareas.length, 0));
  const TITULO = $derived(
    copias
      ? { copiar: "Copiar ahora las copias elegidas", verificar: "Verificar varios equipos", pausar: "Pausar varios equipos", reanudar: "Reanudar varios equipos" }
      : { copiar: "Copiar ahora en varios equipos", verificar: "Verificar varios equipos", pausar: "Pausar las copias de varios equipos", reanudar: "Reanudar las copias de varios equipos" },
  );
  const BOTON: Record<Accion, string> = { copiar: "Copiar ahora", verificar: "Verificar", pausar: "Pausar", reanudar: "Reanudar" };
  const SALTA: Record<Accion, string> = {
    copiar: "Sin copias activas: se salta",
    verificar: "Sin versiones que verificar: se salta",
    pausar: "Sin copias automáticas: se salta",
    reanudar: "No está en pausa: se salta",
  };
  const NINGUNO: Record<Accion, string> = { copiar: "copias activas", verificar: "repositorios con versiones", pausar: "copias automáticas", reanudar: "copias en pausa" };

  type Estado = { fase: "cola" | "enviando" | "esperando" | "fin"; ordenes: Orden[]; error?: string };
  let estados = $state<Record<string, Estado>>({});
  let enMarcha = $state(false);
  let terminado = $state(false);

  const FINAL = ["hecha", "fallida", "rechazada", "cancelada", "caducada"];
  /** Una orden con espera (pausar) se queda en el servidor hasta su hora: no se espera aquí. */
  const programada = (o: Orden) => !!o.not_before && Date.parse(o.not_before) > Date.now() && !FINAL.includes(o.estado);

  async function ejecutar() {
    errorClave = "";
    let kcfg: Uint8Array | null = null;
    if (accion === "pausar") {
      if (!clave) return void (errorClave = "Escribe la clave de administración.");
      preparando = true;
      try {
        kcfg = await kcfgDelCliente(cliente, clave);
      } catch (e) {
        return void (errorClave = (e as Error).message);
      } finally {
        preparando = false;
      }
      // Si no abre ninguno de los equipos, la clave no es la del cliente: no se empieza.
      if (!conTareas.some((p) => etiquetaValida(kcfg!, p.equipo))) {
        borrar(kcfg);
        return void (errorClave = "La clave de administración no es correcta. No se ha enviado nada.");
      }
    }
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
        let prueba: Uint8Array | null = null;
        try {
          if (kcfg) prueba = await pruebaParaEquipo(cliente, eq, clave, kcfg);
          for (const t of p.tareas) {
            const o = await mandarOrden({ cliente, equipo: eq, tipo: t.tipo, cuerpo: t.cuerpo, secretos: prueba ? { prueba } : undefined });
            est.ordenes = [...est.ordenes, o];
            // v1.58: una con espera puede llevar el número reservado (no mueve el siguiente).
            eq = o.seq === eq.siguiente_seq ? { ...eq, siguiente_seq: o.seq + 1 } : { ...eq, seq_espera: o.seq + 1000 };
          }
          est.fase = est.ordenes.every(programada) ? "fin" : "esperando";
        } catch (e) {
          est.error = (e as Error).message;
          est.fase = est.ordenes.length ? "esperando" : "fin";
        } finally {
          borrar(prueba);
        }
      }
    };
    try {
      await Promise.all([trabajador(), trabajador(), trabajador()]);
    } finally {
      borrar(kcfg);
      clave = "";
    }
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
            if (s.ordenes.every((o) => FINAL.includes(o.estado) || programada(o))) s.fase = "fin";
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
    const prog = s.ordenes.find(programada);
    if (prog) return { tono: "ok", texto: `Pausa programada: empieza ${relativo(prog.not_before)}. Hasta entonces se puede cancelar en «Órdenes».` };
    if (s.fase === "fin") {
      const texto =
        accion === "copiar"
          ? `${plural(hechas, "copia pedida", "copias pedidas")}: el equipo la${hechas === 1 ? "" : "s"} está haciendo`
          : accion === "verificar"
            ? plural(hechas, "verificación pedida", "verificaciones pedidas")
            : accion === "pausar"
              ? "Copias en pausa"
              : "Copias reanudadas";
      return { tono: "ok", texto };
    }
    if (!e.conectado) return { tono: "neutral", texto: "Sin conexión: lo hará cuando vuelva" };
    return { tono: "info", texto: `${hechas} de ${s.ordenes.length} recibidas…` };
  }
</script>

<Modal labelledby="t-bloque" {onclose} width={600} dismissible={!enMarcha || terminado}>
  <div class="dlg-title">
    <span class="ticon">
      {#if accion === "copiar"}<Play size={18} />{:else if accion === "verificar"}<ShieldCheck size={18} />{:else if accion === "pausar"}<CirclePause size={18} />{:else}<CirclePlay size={18} />{/if}
    </span>
    <div>
      <h2 id="t-bloque">{TITULO[accion]}</h2>
      <p>
        {#if accion === "copiar"}{copias ? "Cada equipo hace ahora las copias elegidas, sin esperar a su hora." : "Cada equipo hace ahora sus copias activas, sin esperar a su hora."} No borra nada.
        {:else if accion === "verificar"}Cada equipo comprueba una parte de sus repositorios para confirmar que las copias se pueden leer.
        {:else if accion === "pausar"}Las copias automáticas de cada equipo se detienen el tiempo que elijas. «Copiar ahora» sigue funcionando. Como en la ficha de cada equipo, empieza pasada la espera de seguridad y hasta entonces se puede cancelar.
        {:else}Las copias automáticas de cada equipo vuelven a su horario a partir de ahora.{/if}
      </p>
    </div>
  </div>

  {#if !enMarcha}
    <p>
      {#if total && accion === "pausar"}Se pausarán las copias automáticas de {plural(conTareas.length, "equipo", "equipos")}.{:else if total && accion === "reanudar"}Se reanudarán las copias automáticas de {plural(conTareas.length, "equipo", "equipos")}.{:else if total}Se {total === 1 ? "pedirá" : "pedirán"} {accion === "copiar" ? plural(total, "copia", "copias") : plural(total, "verificación", "verificaciones")} en {plural(conTareas.length, "equipo", "equipos")}.{:else}Ninguno de los equipos elegidos tiene {NINGUNO[accion]}.{/if}
    </p>
    <ul class="lista-b">
      {#each conTareas as p (p.equipo.id)}
        <li><span class="nombre">{p.equipo.nombre}{#if !p.equipo.conectado}<span class="faint sin"><WifiOff size={12} />sin conexión</span>{/if}</span><span class="faint que">{p.tareas.map((t) => t.que).join(", ")}</span></li>
      {/each}
      {#each sinTareas as p (p.equipo.id)}
        <li class="apagado"><span class="nombre">{p.equipo.nombre}</span><span class="faint que">{SALTA[accion]}</span></li>
      {/each}
    </ul>
    {#if accion === "pausar" && total}
      <div class="field">
        <label class="field-label" for="bloque-horas">Durante</label>
        <select id="bloque-horas" class="input" bind:value={horas}>
          {#each HORAS as [h, t] (h)}<option value={h}>{t}</option>{/each}
        </select>
      </div>
      <CampoClave requerido id="bloque-clave" etiqueta="Clave de administración" bind:value={clave} error={errorClave} ayuda="Se comprueba con cada equipo antes de enviarle nada." />
    {/if}
    <footer>
      <button type="button" class="btn btn-ghost" onclick={onclose}>Cancelar</button>
      <BotonCargando class="btn btn-primary" disabled={!total} cargando={preparando} textoCargando="Comprobando la clave…" onclick={ejecutar}>{BOTON[accion]} · {plural(conTareas.length, "equipo", "equipos")}</BotonCargando>
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
