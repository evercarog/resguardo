<script lang="ts">
  // «Mover este cliente a otra consola» (docs/consolas-multiples.md, §2.4):
  // sin cambiar de golpe, en tres pasos que se ven y se pueden parar:
  // 1) conectar también a la otra (anadir_consola, con su código de conexión);
  // 2) esperar a que cada equipo informe allí (su resumen la lista con contacto);
  // 3) «Dejar esta consola»: quitar_consola de esta misma en cada equipo que ya
  //    está allí (clave de administración otra vez). Si algo falla, el equipo
  //    sigue aquí: nunca se queda sin consola.
  import { onDestroy } from "svelte";
  import { ArrowRightLeft, CircleCheck, CircleX, KeyRound, LoaderCircle, ShieldCheck, TriangleAlert } from "@lucide/svelte";
  import Modal from "$ui/componentes/Modal.svelte";
  import * as api from "$lib/api";
  import { urlAgentes } from "$lib/estado.svelte";
  import { ErrorLlavesCambiadas, mandarOrden } from "$lib/ordenar";
  import { argon2Navegador } from "$lib/cripto/argon2";
  import { aB64, borrar } from "$lib/cripto/bytes";
  import { kCfg, materialCliente } from "$lib/cripto/claves";
  import { cuerpoAnadir, hostDe, huellaConPuntos, leerCodigo, type CodigoConexion } from "$lib/conexion";
  import { huellaCorta } from "$lib/servidores";
  import { fechaLarga, plural } from "$lib/formato";
  import type { Cliente, Equipo, Orden } from "$lib/tipos";
  import Ayuda from "./Ayuda.svelte";
  import AlertaLlaves from "./AlertaLlaves.svelte";
  import CampoClave from "./CampoClave.svelte";

  let { cliente, equipos, onclose, alTerminar }: { cliente: Cliente; equipos: Equipo[]; onclose: () => void; alTerminar?: () => void } = $props();

  type Fila = { equipo: Equipo; conectar: Orden | null; dejar: Orden | null; alli: boolean; error: string | null };
  let paso = $state<"codigo" | "conectar" | "esperar" | "dejar" | "listo">("codigo");
  let pegado = $state("");
  let comprobado = $state(false);
  let claveAdmin = $state("");
  let ocupado = $state(false);
  let pasoTxt = $state("");
  let error = $state("");
  let cambiadas = $state<Equipo | null>(null);
  let destino = $state<{ nombre: string; url: string; identidad: string } | null>(null);
  let estaIdentidad = $state("");
  let filas = $state<Fila[]>([]);
  let t: ReturnType<typeof setInterval> | undefined;

  const codigo = $derived(pegado.trim() ? leerCodigo(pegado, new Date(), [location.origin, urlAgentes()]) : null);
  const datos = $derived(codigo && typeof codigo !== "string" ? (codigo as CodigoConexion) : null);
  const activos = $derived(equipos.filter((e) => e.confirmado && e.modo === "gestionado"));
  const sinSoporte = $derived(activos.filter((e) => !e.resumen?.admite?.includes("consolas_multiples")));
  const movibles = $derived(activos.filter((e) => e.resumen?.admite?.includes("consolas_multiples")));
  const FINAL = ["hecha", "fallida", "rechazada", "cancelada", "caducada"];
  const yaAlli = (e: Equipo, ident: string) => !!e.resumen?.consolas?.some((x) => x.identidad === ident && !x.esta && x.ultimo_contacto);
  const listos = $derived(filas.filter((f) => f.alli));
  const dejadas = $derived(filas.filter((f) => f.dejar?.estado === "hecha"));

  onDestroy(() => {
    claveAdmin = pegado = "";
    clearInterval(t);
  });

  /** Cada pocos segundos: cómo van las órdenes y quién ya informa en la otra consola. */
  async function vigilar() {
    try {
      const [r, eqs] = await Promise.all([api.ordenesCliente(cliente.id, { limite: 200 }), api.equipos(cliente.id)]);
      const orden = (o: Orden | null) => (o ? (r.ordenes.find((x) => x.id === o.id) ?? o) : null);
      filas = filas.map((f) => {
        const e = eqs.find((x) => x.id === f.equipo.id) ?? f.equipo;
        return { ...f, equipo: e, conectar: orden(f.conectar), dejar: orden(f.dejar), alli: f.alli || (!!destino && yaAlli(e, destino.identidad)) };
      });
      if (paso === "dejar" && filas.filter((f) => f.dejar).every((f) => FINAL.includes(f.dejar!.estado))) {
        paso = "listo";
        clearInterval(t);
        alTerminar?.();
      }
    } catch {
      /* otra vez en la siguiente vuelta */
    }
  }

  async function conectar(e: SubmitEvent) {
    e.preventDefault();
    if (!datos) return;
    error = "";
    cambiadas = null;
    ocupado = true;
    let kcfg: Uint8Array | null = null;
    try {
      estaIdentidad = (await api.servidor()).identidad;
      destino = { nombre: datos.nombre, url: datos.url, identidad: datos.identidad };
      filas = movibles.map((equipo) => ({ equipo, conectar: null, dejar: null, alli: yaAlli(equipo, datos.identidad), error: null }));
      const faltan = filas.filter((f) => !f.alli && !f.equipo.resumen?.consolas?.some((x) => x.identidad === datos.identidad));
      if (faltan.length) {
        pasoTxt = "Preparando la clave de la otra consola…";
        const material = await materialCliente(argon2Navegador, claveAdmin, datos.sal_cliente);
        kcfg = kCfg(material);
        borrar(material);
        const cuerpo = cuerpoAnadir(datos, aB64(kcfg), cliente.sal_cliente);
        for (const [i, f] of faltan.entries()) {
          pasoTxt = `Conectando ${f.equipo.nombre} (${i + 1} de ${faltan.length})…`;
          try {
            f.conectar = await mandarOrden({ cliente, equipo: f.equipo, tipo: "anadir_consola", cuerpo, secretos: { claveAdmin } });
          } catch (err) {
            if (err instanceof ErrorLlavesCambiadas) cambiadas = f.equipo;
            f.error = (err as Error).message;
            if (i === 0) throw err;
          }
        }
        cuerpo.k_cfg = "";
      }
      claveAdmin = pegado = "";
      paso = "esperar";
      t = setInterval(vigilar, 3000);
      void vigilar();
    } catch (err) {
      if (!(err instanceof ErrorLlavesCambiadas)) error = (err as Error).message;
    } finally {
      if (kcfg) borrar(kcfg);
      ocupado = false;
      pasoTxt = "";
    }
  }

  async function dejar(e: SubmitEvent) {
    e.preventDefault();
    error = "";
    ocupado = true;
    try {
      const a = filas.filter((f) => f.alli && !f.dejar);
      for (const [i, f] of a.entries()) {
        pasoTxt = `${f.equipo.nombre} (${i + 1} de ${a.length})…`;
        try {
          f.dejar = await mandarOrden({ cliente, equipo: f.equipo, tipo: "quitar_consola", cuerpo: { identidad: estaIdentidad }, secretos: { claveAdmin } });
        } catch (err) {
          f.error = (err as Error).message;
          if (i === 0) throw err;
        }
      }
      claveAdmin = "";
      paso = "dejar";
      void vigilar();
    } catch (err) {
      error = (err as Error).message;
    } finally {
      ocupado = false;
      pasoTxt = "";
    }
  }

  function estadoDe(f: Fila): { texto: string; tono: "ok" | "mal" | "espera" } {
    if (f.dejar) {
      if (f.dejar.estado === "hecha") return { texto: `Ya solo está en ${destino?.nombre}`, tono: "ok" };
      if (FINAL.includes(f.dejar.estado)) return { texto: `Sigue aquí: ${f.dejar.mensaje ?? f.dejar.estado}`, tono: "mal" };
      return { texto: "Dejando esta consola…", tono: "espera" };
    }
    if (f.error) return { texto: `Sigue aquí: ${f.error}`, tono: "mal" };
    if (f.alli) return { texto: `Ya informa en ${destino?.nombre}`, tono: "ok" };
    if (f.conectar && FINAL.includes(f.conectar.estado) && f.conectar.estado !== "hecha") return { texto: `Sigue aquí: ${f.conectar.mensaje ?? f.conectar.estado}`, tono: "mal" };
    if (f.conectar?.estado === "hecha") return { texto: "Conectado: esperando su primer informe allí…", tono: "espera" };
    return { texto: f.conectar ? "Conectando…" : "Esperando al equipo…", tono: "espera" };
  }
</script>

<Modal labelledby="t-mover-consola" {onclose} width={620} dismissible={!ocupado}>
  <div class="dlg-title">
    <span class="ticon"><ArrowRightLeft size={18} /></span>
    <div>
      <h2 id="t-mover-consola">Mover «{cliente.nombre}» a otra consola</h2>
      <p>Primero los equipos se conectan <strong>también</strong> a la otra; cuando cada uno ya informa allí, dejan esta. Ningún equipo se queda nunca sin consola, y no se reconfigura nada.</p>
    </div>
  </div>

  {#if paso === "codigo"}
    <form class="form" onsubmit={conectar}>
      <ol class="guia">
        <li>En la <strong>otra consola</strong> (cualquier Resguardo Server que estos equipos alcancen): <strong>Clientes → Recibir un cliente → «Gestionarlo también desde aquí»</strong>, o en el cliente, <strong>Servidor → «Dar un código de conexión»</strong>.</li>
        <li>Pega aquí su código de conexión («RGC1.…»).</li>
      </ol>
      <div class="field">
        <label class="field-label" for="codigo-mover">Código de conexión</label>
        <textarea id="codigo-mover" class="input mono" rows="3" bind:value={pegado} placeholder="RGC1.…" spellcheck="false" autocomplete="off"></textarea>
        {#if typeof codigo === "string"}<p class="error-campo">{codigo}</p>{/if}
      </div>
      {#if datos}
        <div class="ficha-consola">
          <div><span class="faint">Consola</span><strong>{datos.nombre}</strong></div>
          <div><span class="faint">Dirección</span><code>{datos.url}</code></div>
          <div><span class="faint">Identidad</span><code>{huellaCorta(datos.identidad)}</code></div>
          <div><span class="faint">Autoridad TLS</span><code>{huellaConPuntos(datos.huella_ca)}</code></div>
          <div><span class="faint">Vale hasta</span><span>{fechaLarga(datos.caduca)}</span></div>
        </div>
        <label class="check"><input type="checkbox" bind:checked={comprobado} /><ShieldCheck size={14} />He comprobado las huellas de palabra con quien administra la otra consola.</label>
        <p class="faint pequeno">Se moverán {plural(movibles.length, "equipo", "equipos")}.{#if sinSoporte.length} {plural(sinSoporte.length, "equipo tiene", "equipos tienen")} un agente anterior y se quedarán aquí ({sinSoporte.map((e) => e.nombre).join(", ")}): actualízalos o usa «Mover…» (cambiar de servidor).{/if}</p>
        <CampoClave requerido id="clave-mover" etiqueta="Clave de administración" bind:value={claveAdmin}>
          {#snippet extra()}<Ayuda id="clave-admin" />{/snippet}
        </CampoClave>
      {/if}
      {#if cambiadas}<AlertaLlaves equipo={cambiadas} cliente={cliente.id} />{/if}
      {#if error}<div class="notice notice-danger" role="alert"><TriangleAlert size={16} /><p>{error}</p></div>{/if}
      <footer>
        {#if ocupado}<span class="espera" role="status"><LoaderCircle size={15} class="spin" />{pasoTxt}</span>{/if}
        <button type="button" class="btn btn-ghost" disabled={ocupado} onclick={onclose}>Cancelar</button>
        <button class="btn btn-primary" disabled={ocupado || !datos || !comprobado || !claveAdmin || !movibles.length}><KeyRound size={15} />1. Conectar a la otra</button>
      </footer>
    </form>
  {:else}
    <div class="form">
      <ul class="progreso" aria-live="polite">
        {#each filas as f (f.equipo.id)}
          {@const s = estadoDe(f)}
          <li>
            <span class="icono">
              {#if s.tono === "ok"}<CircleCheck size={16} class="ok" />{:else if s.tono === "mal"}<CircleX size={16} class="mal" />{:else}<LoaderCircle size={16} class="spin" />{/if}
            </span>
            <span class="nombre">{f.equipo.nombre}</span>
            <span class="faint">{s.texto}</span>
          </li>
        {/each}
      </ul>
      {#if paso === "esperar"}
        <p class="faint pequeno">2. Cada equipo se conecta a {destino?.nombre} ({hostDe(destino?.url ?? "")}) y sube allí su configuración, su historial y su informe. {listos.length} de {filas.length} ya informan allí. Puedes cerrar y volver: los que ya están se pueden dejar ahora y los demás, después.</p>
        <form class="form" onsubmit={dejar}>
          <CampoClave requerido id="clave-dejar" etiqueta="Clave de administración (para dejar esta consola)" bind:value={claveAdmin} />
          {#if error}<div class="notice notice-danger" role="alert"><TriangleAlert size={16} /><p>{error}</p></div>{/if}
          <footer>
            {#if ocupado}<span class="espera" role="status"><LoaderCircle size={15} class="spin" />{pasoTxt}</span>{/if}
            <button type="button" class="btn btn-ghost" disabled={ocupado} onclick={onclose}>Cerrar</button>
            <button class="btn btn-primary" disabled={ocupado || !listos.length || !claveAdmin}>3. Dejar esta consola en {plural(listos.length, "equipo", "equipos")}</button>
          </footer>
        </form>
      {:else if paso === "dejar"}
        <p class="espera" role="status"><LoaderCircle size={15} class="spin" />3. Cada equipo deja esta consola…</p>
      {:else}
        <div class="notice {dejadas.length === filas.length ? 'notice-info' : 'notice-warn'}">
          <p>
            {#if dejadas.length === filas.length}Listo: {plural(dejadas.length, "equipo ya solo está", "equipos ya solo están")} en {destino?.nombre}. Desde aquí ya no se gestionan (se conserva su historial).
            {:else}{dejadas.length} de {filas.length} ya solo están en {destino?.nombre}; los demás siguen aquí (mira el motivo arriba) y puedes repetirlo.{/if}
          </p>
        </div>
        <footer><button class="btn btn-primary" onclick={onclose}>Hecho</button></footer>
      {/if}
    </div>
  {/if}
</Modal>

<style>
  .guia {
    margin: 0;
    padding-left: 1.3em;
    color: var(--text-2);
    font-size: var(--fs-sm);
    line-height: 1.6;
  }
  .mono {
    font-family: var(--font-mono, ui-monospace, monospace);
    font-size: 12px;
    word-break: break-all;
  }
  .ficha-consola {
    display: grid;
    gap: 6px;
    padding: 10px 12px;
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: var(--radius, 8px);
    font-size: var(--fs-sm);
  }
  .ficha-consola > div {
    display: grid;
    grid-template-columns: 8.5em 1fr;
    gap: 8px;
  }
  .ficha-consola code {
    word-break: break-all;
    font-size: 12px;
  }
  .check {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: var(--fs-sm);
  }
  .pequeno {
    margin: 0;
    font-size: var(--fs-xs);
  }
  .espera {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    margin-right: auto;
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  .progreso {
    display: flex;
    flex-direction: column;
    margin: 0;
    padding: 0;
    list-style: none;
    border-top: 1px solid var(--border);
  }
  .progreso li {
    display: grid;
    grid-template-columns: 20px minmax(0, 10em) 1fr;
    gap: 8px;
    align-items: center;
    padding: 8px 0;
    border-bottom: 1px solid var(--border);
    font-size: var(--fs-sm);
  }
  .progreso .nombre {
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .icono {
    display: inline-flex;
  }
  .icono :global(.ok) {
    color: var(--ok, #16a34a);
  }
  .icono :global(.mal) {
    color: var(--bad, #dc2626);
  }
  @media (max-width: 520px) {
    .ficha-consola > div {
      grid-template-columns: 1fr;
      gap: 0;
    }
    .progreso li {
      grid-template-columns: 20px 1fr;
    }
    .progreso li > .faint {
      grid-column: 2;
    }
  }
</style>
