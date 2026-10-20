<script lang="ts">
  // El código de conexión de ESTA consola (docs/consolas-multiples.md), para
  // que la otra («Conectar también a otra consola…») conecte aquí sus equipos
  // sin dejar la suya. Con un cliente que aún no existe aquí (superusuario),
  // se crea con una sal nueva («Recibir un cliente»); con uno que ya existe
  // (propietario), solo se pide otra ficha. El código lleva la dirección, la
  // identidad, la huella de la autoridad TLS, la ficha y la sal del cliente.
  // 15nn: abierto sin cliente, pregunta antes si es de uno que ya existe aquí
  // (lo normal al traer equipos de otra consola): así no se crea otro igual.
  import { goto } from "$app/navigation";
  import { Monitor, TriangleAlert } from "@lucide/svelte";
  import Modal from "$ui/componentes/Modal.svelte";
  import * as api from "$lib/api";
  import { app, cargarClientes } from "$lib/estado.svelte";
  import { todosLosClientes } from "$lib/nombresClientes.svelte";
  import AvisoNombreRepetido from "./AvisoNombreRepetido.svelte";
  import { aB64, aleatorio } from "$lib/cripto/bytes";
  import { crearCodigo, huellaConPuntos } from "$lib/conexion";
  import { huellaCorta } from "$lib/servidores";
  import { fechaLarga, plural } from "$lib/formato";
  import type { Cliente } from "$lib/tipos";
  import BloqueCopiable from "./BloqueCopiable.svelte";

  let { cliente = null, onclose }: { cliente?: Cliente | null; onclose: () => void } = $props();

  let nombreCliente = $state("");
  let nombreConsola = $state(typeof location !== "undefined" ? location.host : "");
  let usos = $state(50);
  let dias = $state(7);
  let ocupado = $state(false);
  let error = $state("");
  let hecho = $state<{ codigo: string; identidad: string; huella: string; caduca: string; usos: number; cliente: { id: string; nombre: string } } | null>(null);

  // 15nn: sin cliente, ¿es de uno que ya existe aquí? Su código solo lo da su propietario.
  const propios = $derived(app.clientes.filter((c) => c.rol === "propietario"));
  const otros = $derived(app.clientes.length - propios.length);
  let eleccion = $state<"existente" | "nuevo">(app.clientes.some((c) => c.rol === "propietario") ? "existente" : "nuevo");
  let existenteId = $state(app.clientes.find((c) => c.rol === "propietario")?.id ?? "");
  function usarExistente(c: { id: string }) {
    eleccion = "existente";
    existenteId = c.id;
  }

  async function crear(e: SubmitEvent) {
    e.preventDefault();
    ocupado = true;
    error = "";
    try {
      const srv = await api.servidor();
      let ficha, sal: string, cli: { id: string; nombre: string };
      const elegido = cliente ?? (eleccion === "existente" && existenteId ? await api.cliente(existenteId) : null);
      if (elegido) {
        ficha = await api.nuevaFicha(elegido.id, { usos, dias });
        sal = elegido.sal_cliente;
        cli = { id: elegido.id, nombre: elegido.nombre };
      } else {
        // Un cliente nuevo aquí, con su propia sal: la otra consola calcula la K_cfg de aquí con ella.
        const r = await api.recibirCliente({ nombre: nombreCliente.trim(), sal_cliente: aB64(aleatorio(16)), usos, dias });
        ficha = r;
        sal = r.cliente.sal_cliente;
        cli = { id: r.cliente.id, nombre: r.cliente.nombre };
        await cargarClientes();
      }
      if (ficha.servidor.identidad !== srv.identidad) throw new Error("El servidor dio otra identidad para la ficha: no se crea el código.");
      const codigo = crearCodigo({
        // v1.34: la dirección para los agentes (en una consola en internet, agentes.<dominio>, con
        // la autoridad TLS propia que fijan); si el servidor no la da, la de esta consola.
        url: (srv.url_agentes || location.origin).replace(/\/+$/, ""),
        identidad: srv.identidad,
        huella_ca: srv.huella_ca,
        ficha: ficha.ficha,
        sal_cliente: sal,
        nombre: nombreConsola.trim() || location.host,
        cliente: cli.nombre,
        caduca: ficha.caduca,
      });
      hecho = { codigo, identidad: srv.identidad, huella: srv.huella_ca, caduca: ficha.caduca, usos: ficha.usos, cliente: cli };
    } catch (err) {
      error = (err as Error).message;
    } finally {
      ocupado = false;
    }
  }
</script>

<Modal labelledby="t-codigo-conexion" {onclose} width={580} dismissible={!hecho} sePierde="El código no se vuelve a enseñar: cópialo antes de cerrar.">
  <div class="dlg-title">
    <span class="ticon"><Monitor size={18} /></span>
    <div>
      {#if !hecho}
        <h2 id="t-codigo-conexion">Dar un código de conexión</h2>
        <p>Para que los equipos de un cliente que ya se gestiona en otra consola se gestionen <strong>también</strong> desde esta, sin dejar aquella. En la otra se pega este código en «Conectar también a otra consola…».</p>
      {:else}
        <h2 id="t-codigo-conexion">Código de conexión de «{hecho.cliente.nombre}»</h2>
        <p>Cópialo y pégalo en la otra consola: en el cliente → «Ajustes» → «Servidor» → «Conectar también a otra consola…» (o en «Conectar también…» del aviso de equipos que no están en todas tus consolas).</p>
      {/if}
    </div>
  </div>
  {#if !hecho}
    <form class="form" onsubmit={crear}>
      {#if !cliente}
        <fieldset class="field eleccion">
          <legend class="field-label">¿Es de un cliente que ya tienes aquí?</legend>
          <label class="opcion" class:apagada={!propios.length}>
            <input type="radio" name="cc-eleccion" value="existente" bind:group={eleccion} disabled={!propios.length} />
            <span>Sí: elige cuál y te damos su código de conexión</span>
          </label>
          {#if eleccion === "existente" && propios.length}
            <select id="cc-existente" class="input" bind:value={existenteId} aria-label="Cliente de esta consola">
              {#each propios as c (c.id)}<option value={c.id}>{c.nombre}</option>{/each}
            </select>
          {/if}
          <label class="opcion">
            <input type="radio" name="cc-eleccion" value="nuevo" bind:group={eleccion} />
            <span>No, aún no está aquí: crearlo</span>
          </label>
          {#if otros}<span class="field-hint">Solo salen los clientes de los que eres propietario: el código de los demás lo da su propietario.</span>{/if}
        </fieldset>
        {#if eleccion === "nuevo"}
          <div class="field">
            <label class="field-label" for="cc-cliente">Nombre del cliente</label>
            <input id="cc-cliente" class="input" required maxlength="80" bind:value={nombreCliente} placeholder="Café del Sur" />
            <span class="field-hint">Se crea aquí con su propia sal. Su clave de administración sigue siendo la misma que en la otra consola.</span>
          </div>
          <AvisoNombreRepetido nombre={nombreCliente} clientes={todosLosClientes()} alUsar={usarExistente} usables={propios.map((c) => c.id)} />
        {/if}
      {/if}
      <div class="field">
        <label class="field-label" for="cc-consola">Cómo se llamará esta consola en los equipos</label>
        <input id="cc-consola" class="input" maxlength="80" bind:value={nombreConsola} />
      </div>
      <div class="dos">
        <div class="field">
          <label class="field-label" for="cc-usos">Equipos que pueden conectarse</label>
          <input id="cc-usos" class="input" type="number" min="1" max="1000" bind:value={usos} />
        </div>
        <div class="field">
          <label class="field-label" for="cc-dias">El código vale</label>
          <select id="cc-dias" class="input" bind:value={dias}>
            {#each [1, 3, 7, 14, 30] as d (d)}<option value={d}>{d === 1 ? "1 día" : `${d} días`}</option>{/each}
          </select>
        </div>
      </div>
      {#if error}<p class="error-campo" role="alert">{error}</p>{/if}
      <footer>
        <button type="button" class="btn btn-ghost" onclick={onclose}>Cancelar</button>
        <button class="btn btn-primary" disabled={ocupado || (!cliente && (eleccion === "nuevo" ? !nombreCliente.trim() : !existenteId))}>{ocupado ? "Creando…" : "Crear el código"}</button>
      </footer>
    </form>
  {:else}
    <div class="form">
      <BloqueCopiable texto={hecho.codigo} etiqueta="Copiar el código" alto={5} />
      <div class="huellas">
        <div><span class="faint">Identidad de esta consola</span><code>{huellaCorta(hecho.identidad)}</code></div>
        <div><span class="faint">Autoridad TLS</span><code>{huellaConPuntos(hecho.huella)}</code></div>
      </div>
      <p class="faint pequeno">Quien lo pegue en la otra consola verá estas huellas: compárenlas de palabra. Vale para {plural(hecho.usos, "equipo", "equipos")} hasta el {fechaLarga(hecho.caduca)}.</p>
      <div class="notice notice-warn"><TriangleAlert size={16} /><p>El código solo se muestra ahora y lleva una ficha para dar de alta equipos aquí: compártelo solo con quien administra la otra consola. Sin la clave de administración del cliente no sirve para conectar ningún equipo.</p></div>
      <footer>
        <button class="btn btn-primary" onclick={() => (onclose(), cliente ? undefined : goto(`/c/${hecho!.cliente.id}`))}>Listo</button>
      </footer>
    </div>
  {/if}
</Modal>

<style>
  .eleccion {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin: 0;
    padding: 0;
    border: 0;
  }
  .opcion {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: var(--fs-sm);
  }
  .opcion.apagada {
    color: var(--text-3);
  }
  .eleccion select {
    margin-left: 26px;
    width: calc(100% - 26px);
  }
  .dos {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: var(--sp-3);
  }
  .huellas {
    display: grid;
    gap: 6px;
    font-size: var(--fs-sm);
  }
  .huellas > div {
    display: grid;
    grid-template-columns: 12em 1fr;
    gap: 8px;
  }
  .huellas code {
    word-break: break-all;
    font-size: 12px;
  }
  .pequeno {
    margin: 0;
    font-size: var(--fs-xs);
  }
  @media (max-width: 520px) {
    .dos,
    .huellas > div {
      grid-template-columns: 1fr;
    }
  }
</style>
