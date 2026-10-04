<script lang="ts">
  // «Usar uno que ya existe» en modo local: un repositorio de restic que ya
  // existe (el de la app de escritorio, otro programa o hecho a mano) pasa a
  // ser uno más de este equipo, con todo su historial. Primero se prueba (el
  // servicio lo abre con esa contraseña y dice qué tiene); nada se borra.
  import { onDestroy } from "svelte";
  import { Database, FlaskConical } from "@lucide/svelte";
  import FormRepoExistente from "$lib/componentes/FormRepoExistente.svelte";
  import { partirDireccion, repoExistenteCompleto, repoExistenteVacio } from "$lib/direccion";
  import { esBloqueo, servicio } from "../../puente.svelte";
  import { idDe, type PropsParte } from "./comun";
  import { cuerpoExistente, probarExistente, type Prueba } from "./existente";

  let { estado, recargar, alBloquear, alCerrar, alHecho }: PropsParte & { alCerrar: () => void; alHecho: (mensaje: string) => void } = $props();

  let repo = $state(repoExistenteVacio());
  let nombre = $state("");
  let ocupado = $state(false);
  let error = $state("");
  let probado = $state<Prueba | null>(null);
  onDestroy(() => {
    repo.contrasena = repo.secreto = "";
  });

  // Lo probado deja de valer si cambia el repositorio.
  const huella = $derived(JSON.stringify([repo.tipo, repo.direccion, repo.usuario, repo.secreto, repo.ca, repo.contrasena]));
  let probadoCon = "";
  $effect(() => {
    if (huella !== probadoCon) probado = null;
  });

  async function pedir<T>(f: () => Promise<T>): Promise<T | null> {
    ocupado = true;
    error = "";
    try {
      return await f();
    } catch (e) {
      if (esBloqueo(e)) alBloquear();
      error = (e as Error).message;
      return null;
    } finally {
      ocupado = false;
    }
  }

  async function probar() {
    const h = huella;
    const p = await pedir(() => probarExistente(repo));
    if (!p) return;
    probado = p;
    probadoCon = h;
    if (!nombre.trim()) nombre = partirDireccion(repo.tipo, repo.direccion).ruta.split("/").pop() || "Copias de antes";
  }

  async function usar(e: SubmitEvent) {
    e.preventDefault();
    if (!probado || probado.en_uso || !nombre.trim()) return;
    const r = await pedir(() =>
      servicio<{ mensaje: string }>("adoptar_repositorio", {
        repositorio: { id: idDe(nombre.trim()), nombre: nombre.trim(), ...cuerpoExistente(repo, idDe(`existente-${repo.tipo}`)) },
      }),
    );
    if (!r) return;
    repo.contrasena = repo.secreto = "";
    await recargar();
    alHecho(r.mensaje);
  }
  const fecha = (t: string | null) => (t ? new Date(t).toLocaleString("es", { dateStyle: "medium", timeStyle: "short" }) : "");
</script>

<form class="v-tarjeta v-pila" onsubmit={usar} aria-labelledby="t-existente">
  <div class="v-fila">
    <Database size={18} aria-hidden="true" />
    <h3 class="v-titulo" id="t-existente">Usar un repositorio que ya existe</h3>
  </div>
  <p class="v-mini">Por ejemplo, el de la app de escritorio. <strong>Se conserva todo su historial</strong>: las versiones que ya tiene siguen ahí y las copias nuevas se añaden a ellas. No se borra ni se cambia nada.</p>
  <FormRepoExistente bind:repo id="v-existente" nombreEquipo={estado.nombre_equipo} local />
  <div class="v-fila fin">
    <button type="button" class="btn btn-sm" disabled={ocupado || !repoExistenteCompleto(repo)} onclick={probar}><FlaskConical size={14} aria-hidden="true" />{ocupado && !probado ? "Probando…" : "Probar"}</button>
  </div>
  {#if probado}
    <div class="v-pila resultado" role="status">
      <p class="v-ok">{probado.mensaje}</p>
      {#if probado.ultima}<p class="v-mini">La última versión es del {fecha(probado.ultima)}{probado.equipos.length ? ` · de ${probado.equipos.join(", ")}` : ""}.</p>{/if}
      {#if probado.en_uso}
        <p class="v-mini aviso">Este equipo ya lo usa («{probado.en_uso}»): no hace falta añadirlo otra vez.</p>
      {:else}
        <label class="field"><span class="field-label">Nombre en este equipo</span><input class="input" bind:value={nombre} maxlength="80" /></label>
      {/if}
    </div>
  {/if}
  {#if error}<p class="v-error" role="alert">{error}</p>{/if}
  <div class="v-fila fin">
    <button type="button" class="btn btn-ghost" onclick={alCerrar}>Cancelar</button>
    <button class="btn btn-primary" disabled={ocupado || !probado || !!probado.en_uso || !nombre.trim()}>{ocupado && probado ? "Añadiendo…" : "Usar este repositorio"}</button>
  </div>
  {#if !probado}<p class="v-mini">Primero «Probar»: se comprueba que la contraseña lo abre.</p>{/if}
</form>

<style>
  .fin {
    justify-content: flex-end;
    flex-wrap: wrap;
  }
  .resultado {
    padding-top: var(--sp-3);
    border-top: 1px solid var(--border);
  }
  .aviso {
    color: var(--warn);
    margin: 0;
  }
</style>
