<script lang="ts">
  // «Cambiar la clave de administración» (cliente → Personas y ajustes): la
  // orden firmada `cambiar_clave_admin` a cada equipo del cliente (lib/cambioClave.ts).
  //
  // 1. La clave actual se comprueba con la etiqueta de los equipos (K_cfg); la
  //    nueva, con su fuerza, generada si se quiere, y repetida.
  // 2. A cada equipo que tiene la actual: su verificador nuevo y su K_cfg nueva
  //    (y la de cada otra consola que lo gestiona, con la sal de esa consola).
  // 3. Los que no están conectados la aplican al conectar (la orden espera 7
  //    días): mientras, el cliente está «a medias» y en esos equipos sigue
  //    valiendo la anterior. La lista dice en qué va cada uno.
  // 4. El paquete de exportación guardado en el servidor (si lo hay) se vuelve
  //    a cifrar con la clave nueva. Al final, la hoja de la clave para imprimir.
  //
  // Las claves solo viven aquí (y en los campos mientras se escriben): al
  // equipo llegan su prueba y su verificador sellados; al servidor, nada.
  import { onDestroy } from "svelte";
  import { CircleCheck, KeyRound, Printer, Shuffle, TriangleAlert } from "@lucide/svelte";
  import Modal from "$ui/componentes/Modal.svelte";
  import * as api from "$lib/api";
  import { argon2Navegador } from "$lib/cripto/argon2";
  import { aleatorio, borrar } from "$lib/cripto/bytes";
  import { kCfg, materialCliente, pruebaAdmin } from "$lib/cripto/claves";
  import { ClaveNueva, claveGenerada, equiposDelCambio, estadoCambio, repartir, type EstadoCambio } from "$lib/cambioClave";
  import { comprobarLlaves, fijar } from "$lib/fijadas";
  import { mandarOrden } from "$lib/ordenar";
  import * as exportar from "$lib/exportar";
  import { fuerza } from "$lib/fuerza";
  import { avisar } from "$lib/avisos.svelte";
  import { fechaLarga, plural } from "$lib/formato";
  import type { Cliente, Equipo, Orden } from "$lib/tipos";
  import type { Tono } from "$lib/salud";
  import Ayuda from "./Ayuda.svelte";
  import BloqueCopiable from "./BloqueCopiable.svelte";
  import CampoClave from "./CampoClave.svelte";
  import Chip from "./Chip.svelte";

  let { cliente, equipos, onclose }: { cliente: Cliente; equipos: Equipo[]; onclose: () => void } = $props();

  let paso = $state<"claves" | "enviando" | "hoja">("claves");
  let actual = $state("");
  let nueva = $state("");
  let repetida = $state("");
  let guardada = $state(false);
  let impresa = $state(false);
  let error = $state("");
  let pasoTxt = $state("");
  let ocupado = $state(false);
  const f = $derived(fuerza(nueva));
  const MIN = 16;
  const valida = $derived(!!actual && [...nueva].length >= MIN && f.nivel >= 2 && nueva === repetida && nueva !== actual && guardada);

  /** Cada equipo y en qué va su cambio. */
  interface Fila {
    equipo: Equipo;
    estado: "preparando" | "enviada" | EstadoCambio | "ya_nueva" | "otra_clave" | "error";
    orden?: Orden;
    mensaje?: string;
  }
  let filas = $state<Fila[]>([]);
  let paquete = $state<"" | "recifrado" | "fallo">("");
  const candidatos = $derived(equiposDelCambio(equipos));
  const otrasConsolas = $derived(ClaveNueva.otrasConsolas(candidatos));
  let vivo = true;
  onDestroy(() => {
    vivo = false;
    actual = nueva = repetida = "";
  });

  function generar() {
    nueva = repetida = claveGenerada(aleatorio);
  }

  async function cambiar(e: SubmitEvent) {
    e.preventDefault();
    if (!valida || ocupado) return;
    error = "";
    ocupado = true;
    const clave = new ClaveNueva(argon2Navegador, nueva, cliente.sal_cliente);
    let kcfgActual: Uint8Array | null = null;
    try {
      pasoTxt = "Comprobando la clave actual…";
      const m = await materialCliente(argon2Navegador, actual, cliente.sal_cliente);
      kcfgActual = kCfg(m);
      borrar(m);
      pasoTxt = "Preparando la clave nueva…";
      const r = repartir(candidatos, kcfgActual, await clave.kcfg());
      if (!r.conActual.length) {
        throw new Error(
          r.yaNueva.length && !r.otra.length
            ? "Todos los equipos tienen ya esa clave nueva: no hay nada que cambiar."
            : "La clave actual no es correcta (no coincide con la de ningún equipo de este cliente).",
        );
      }
      filas = [
        ...r.conActual.map((eq): Fila => ({ equipo: eq, estado: "preparando" })),
        ...r.yaNueva.map((eq): Fila => ({ equipo: eq, estado: "ya_nueva" })),
        ...r.otra.map((eq): Fila => ({ equipo: eq, estado: "otra_clave" })),
      ];
      paso = "enviando";
      for (const fila of filas) {
        if (fila.estado !== "preparando") continue;
        const eq = fila.equipo;
        pasoTxt = `Preparando ${eq.nombre}…`;
        try {
          // Las llaves del equipo, las de siempre (la etiqueta se acaba de comprobar con la clave actual).
          if ((await comprobarLlaves(cliente.id, eq)) === "cambiada") throw new Error("Sus llaves cambiaron desde que se comprobaron en este navegador: no se envía nada.");
          await fijar(cliente.id, eq);
          const prueba = await pruebaAdmin(argon2Navegador, actual, eq.sal_equipo);
          try {
            fila.orden = await mandarOrden({ cliente, equipo: eq, tipo: "cambiar_clave_admin", cuerpo: await clave.cuerpo(eq), secretos: { prueba } });
            fila.estado = "enviada";
          } finally {
            borrar(prueba);
          }
        } catch (err) {
          fila.estado = "error";
          fila.mensaje = (err as Error).message;
        }
      }
      // El paquete de exportación guardado aquí (si lo hay): con la clave nueva.
      pasoTxt = "Comprobando el paquete de exportación…";
      try {
        const bytes = await api.bajarPaquete(cliente.id);
        if (bytes) {
          const contenido = await exportar.abrir(actual, cliente, bytes);
          await api.subirPaquete(cliente.id, await exportar.cifrar(nueva, cliente, contenido));
          paquete = "recifrado";
        }
      } catch {
        paquete = "fallo";
      }
      const enviadas = filas.filter((x) => x.estado === "enviada").length;
      if (enviadas) avisar(`Cambio de la clave de administración enviado a ${plural(enviadas, "equipo", "equipos")}.`);
      else error = "No se pudo enviar a ningún equipo.";
      void seguir();
    } catch (err) {
      error = (err as Error).message;
      paso = "claves";
    } finally {
      borrar(kcfgActual);
      clave.olvidar();
      ocupado = false;
      pasoTxt = "";
    }
  }

  /** Mientras el diálogo está abierto: en qué va cada orden. */
  async function seguir() {
    while (vivo && filas.some((x) => x.orden && ["enviada", "pendiente", "en_marcha"].includes(x.estado))) {
      for (const fila of filas) {
        if (!fila.orden || !["enviada", "pendiente", "en_marcha"].includes(fila.estado)) continue;
        try {
          const o = (await api.ordenesEquipo(cliente.id, fila.equipo.id, 20)).find((y) => y.id === fila.orden!.id);
          if (o) {
            fila.orden = o;
            fila.estado = estadoCambio(o);
            fila.mensaje = o.estado === "hecha" ? undefined : (o.mensaje ?? undefined);
          }
        } catch {
          // Sin red un momento: se vuelve a mirar.
        }
      }
      await new Promise((r) => setTimeout(r, 3000));
    }
  }

  const TEXTO: Record<Fila["estado"], [Tono, string]> = {
    preparando: ["info", "Preparando…"],
    enviada: ["info", "Enviada"],
    pendiente: ["warn", "Esperando a que se conecte"],
    en_marcha: ["info", "Aplicándose…"],
    aplicada: ["ok", "Clave nueva aplicada"],
    rechazada: ["bad", "Rechazada"],
    cancelada: ["bad", "Cancelada o caducada"],
    ya_nueva: ["ok", "Ya tenía la nueva"],
    otra_clave: ["warn", "Tiene otra clave"],
    error: ["bad", "No se envió"],
  };
  const chip = (x: Fila): [Tono, string] => (x.estado === "pendiente" && x.equipo.conectado ? ["info", "Enviada"] : TEXTO[x.estado]);
  const quedan = $derived(filas.filter((x) => x.estado === "enviada" || x.estado === "pendiente" || x.estado === "en_marcha"));
  const aMedias = $derived(quedan.length > 0 || filas.some((x) => x.estado === "error" || x.estado === "rechazada" || x.estado === "otra_clave"));
  const caduca = $derived(quedan.map((x) => x.orden?.caduca).filter(Boolean).sort()[0] ?? null);
</script>

<Modal labelledby="t-clave-admin" {onclose} width={600} dismissible={paso !== "enviando" || !ocupado}>
  {#if paso === "claves"}
    <form class="form" onsubmit={cambiar}>
      <div class="dlg-title">
        <span class="ticon"><KeyRound size={18} /></span>
        <div>
          <h2 id="t-clave-admin">Cambiar la clave de administración <Ayuda id="clave-admin" /></h2>
          <p>Se manda a {plural(candidatos.length, "equipo", "equipos")} de {cliente.nombre}. Cada uno guarda la nueva y, desde que la aplica, deja de aceptar la anterior. El servidor nunca ve ninguna de las dos.</p>
        </div>
      </div>
      <CampoClave requerido id="ca-actual" etiqueta="Clave actual" autocomplete="current-password" autofocus bind:value={actual} />
      <CampoClave requerido id="ca-nueva" etiqueta="Clave nueva" autocomplete="new-password" bind:value={nueva} ayuda="Al menos {MIN} caracteres. Mejor una generada o una frase larga que no uses en otro sitio." />
      {#if nueva}
        <div class="fuerza" aria-live="polite">
          <div class="barra nivel-{f.nivel}" role="meter" aria-valuemin="0" aria-valuemax="4" aria-valuenow={f.nivel} aria-label="Fuerza de la clave"><span></span></div>
          <span class="pequeno">{f.texto}{[...nueva].length < MIN ? ` · Al menos ${MIN} caracteres.` : f.consejo ? ` · ${f.consejo}` : ""}</span>
        </div>
      {/if}
      <CampoClave requerido id="ca-repetida" etiqueta="Repite la clave nueva" autocomplete="new-password" bind:value={repetida} error={repetida && repetida !== nueva ? "No coincide." : nueva && nueva === actual ? "Es la misma que la actual." : ""} />
      <button type="button" class="btn btn-sm generar" onclick={generar}><Shuffle size={14} />Generar una clave segura</button>
      <div class="notice notice-warn">
        <TriangleAlert size={16} />
        <p>
          Los equipos <strong>sin conexión</strong> la aplicarán al conectarse (la orden espera 7 días). Hasta entonces el cliente queda <strong>a medias</strong>: en esos equipos sigue valiendo la clave anterior.
          {#if otrasConsolas.length}También lo gestionan {otrasConsolas.join(", ")}: allí valdrá la nueva en cuanto cada equipo la aplique (quien las use necesitará la clave nueva).{/if}
        </p>
      </div>
      <label class="switch-row"><input type="checkbox" bind:checked={guardada} /><span>He guardado la clave nueva en un sitio seguro (un gestor de contraseñas o impresa)</span></label>
      {#if error}<div class="notice notice-danger" role="alert"><TriangleAlert size={16} /><p>{error}</p></div>{/if}
      <footer>
        {#if pasoTxt}<span class="espera" role="status">{pasoTxt}</span>{/if}
        <button type="button" class="btn btn-ghost" onclick={onclose} disabled={ocupado}>Cancelar</button>
        <button class="btn btn-primary" disabled={!valida || ocupado}><KeyRound size={15} />Cambiar la clave</button>
      </footer>
    </form>
  {:else if paso === "enviando"}
    <div class="form">
      <div class="dlg-title">
        <span class="ticon"><KeyRound size={18} /></span>
        <div>
          <h2 id="t-clave-admin">Cambiando la clave de administración</h2>
          <p>{ocupado ? "Enviando la orden firmada a cada equipo…" : "Cada equipo la aplica en cuanto la recibe. Puedes cerrar: lo que falte se aplica solo."}</p>
        </div>
      </div>
      <ul class="lista" aria-live="polite">
        {#each filas as x (x.equipo.id)}
          {@const [tono, texto] = chip(x)}
          <li>
            <span class="nombre">{x.equipo.nombre}{#if !x.equipo.conectado}<span class="faint"> · sin conexión</span>{/if}</span>
            <Chip pequeno {tono} {texto} girando={x.estado === "preparando" || x.estado === "en_marcha"} />
            {#if x.mensaje}<span class="faint mensaje">{x.mensaje}</span>{/if}
          </li>
        {/each}
      </ul>
      {#if !ocupado && aMedias}
        <div class="notice notice-warn">
          <TriangleAlert size={16} />
          <p>
            {#if quedan.length}<strong>El cliente está a medias</strong>: {plural(quedan.length, "equipo aún no ha", "equipos aún no han")} aplicado la clave nueva y en {quedan.length === 1 ? "él" : "ellos"} sigue valiendo la anterior hasta que se conecten{caduca ? ` (la orden caduca el ${fechaLarga(caduca)})` : ""}. Lo verás en Personas y ajustes.{/if}
            {#if filas.some((x) => x.estado === "otra_clave")} Los que «tienen otra clave» (quizá un cambio anterior aún sin aplicar) no se tocan: cámbiala en ellos cuando la apliquen.{/if}
            {#if filas.some((x) => x.estado === "error" || x.estado === "rechazada")} Donde no se pudo, vuelve a intentarlo con la clave que aún tengan.{/if}
          </p>
        </div>
      {/if}
      {#if paquete === "recifrado"}<p class="faint pequeno">El paquete de exportación guardado en este servidor se abre ya con la clave nueva.</p>{/if}
      {#if paquete === "fallo"}<p class="faint pequeno">El paquete de exportación guardado en este servidor no se pudo volver a cifrar: se sigue abriendo con la clave anterior (puedes exportarlo de nuevo).</p>{/if}
      {#if error}<div class="notice notice-danger" role="alert"><TriangleAlert size={16} /><p>{error}</p></div>{/if}
      <footer>
        {#if pasoTxt}<span class="espera" role="status">{pasoTxt}</span>{/if}
        <button class="btn btn-primary" disabled={ocupado} onclick={() => (paso = "hoja")}>Seguir: la hoja de la clave</button>
      </footer>
    </div>
  {:else}
    <div class="form">
      <div class="dlg-title">
        <span class="ticon"><Printer size={18} /></span>
        <div>
          <h2 id="t-clave-admin">Guarda la clave nueva</h2>
          <p>Imprímela o guárdala en un gestor de contraseñas, y destruye la hoja de la anterior. Sin ella, los equipos siguen copiando, pero no se pueden cambiar.</p>
        </div>
      </div>
      <article class="kit" id="kit-imprimible">
        <h3>Clave de administración · {cliente.nombre}</h3>
        <dl>
          <dt>Cliente</dt><dd>{cliente.nombre}</dd>
          <dt>Servidor</dt><dd>{typeof location !== "undefined" ? location.host : ""}</dd>
          <dt>Clave</dt><dd><code class="selectable pw">{nueva}</code></dd>
          <dt>Cambiada el</dt><dd>{fechaLarga(new Date().toISOString())}</dd>
        </dl>
      </article>
      <div><button type="button" class="btn" onclick={() => window.print()}><Printer size={15} />Imprimir o guardar en PDF</button></div>
      <BloqueCopiable texto={nueva} etiqueta="Copiar la clave" alto={2} />
      <div class="notice notice-info">
        <CircleCheck size={16} />
        <p>Los <strong>kits de recuperación</strong> de los repositorios no cambian (llevan la contraseña de cada repositorio, no esta clave): no hace falta reimprimirlos. Si guardaste esta clave junto a ellos, cambia esa hoja.{#if otrasConsolas.length} Dásela también a quien use {otrasConsolas.join(", ")}.{/if}</p>
      </div>
      <label class="switch-row"><input type="checkbox" bind:checked={impresa} /><span>He guardado la clave nueva fuera de esta máquina</span></label>
      <footer>
        <button class="btn btn-primary" disabled={!impresa} onclick={onclose}>Listo</button>
      </footer>
    </div>
  {/if}
</Modal>

<style>
  .generar {
    align-self: flex-start;
  }
  .fuerza {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    margin-top: -6px;
  }
  .fuerza .barra {
    flex: 0 0 140px;
    height: 6px;
    border-radius: 3px;
    background: var(--border);
    overflow: hidden;
  }
  .fuerza .barra span {
    display: block;
    height: 100%;
    width: 8%;
    background: var(--bad);
  }
  .nivel-1 span {
    width: 30% !important;
  }
  .nivel-2 span {
    width: 55% !important;
    background: var(--warn) !important;
  }
  .nivel-3 span {
    width: 80% !important;
    background: var(--ok) !important;
  }
  .nivel-4 span {
    width: 100% !important;
    background: var(--ok) !important;
  }
  .pequeno {
    font-size: var(--fs-xs);
    color: var(--text-2);
  }
  .espera {
    margin-right: auto;
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  .lista {
    display: flex;
    flex-direction: column;
    margin: 0;
    padding: 0;
    list-style: none;
    border: 1px solid var(--border);
    border-radius: var(--radius);
  }
  .lista li {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px var(--sp-3);
    padding: 10px var(--sp-3);
    border-top: 1px solid var(--border);
  }
  .lista li:first-child {
    border-top: none;
  }
  .nombre {
    flex: 1;
    min-width: 0;
    font-weight: 500;
  }
  .mensaje {
    flex-basis: 100%;
    font-size: var(--fs-xs);
  }
  .kit {
    padding: var(--sp-4);
    border: 1px solid var(--border);
    border-radius: var(--radius);
  }
  .kit h3 {
    margin: 0 0 var(--sp-3);
  }
  .kit dl {
    display: grid;
    grid-template-columns: max-content 1fr;
    gap: 6px var(--sp-4);
    margin: 0;
  }
  .kit dt {
    color: var(--text-2);
  }
  .kit dd {
    margin: 0;
  }
  .pw {
    font-size: 1.05rem;
    letter-spacing: 0.04em;
    word-break: break-all;
  }
  @media print {
    :global(body *) {
      visibility: hidden;
    }
    :global(#kit-imprimible),
    :global(#kit-imprimible *) {
      visibility: visible;
    }
    :global(#kit-imprimible) {
      position: fixed;
      inset: 0 auto auto 0;
      width: 100%;
      border: none;
      color: #000;
    }
  }
</style>
