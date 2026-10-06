<script lang="ts">
  // Lo que se ajusta de un repositorio, a la vista (docs/editor-de-copias.md,
  // «Dónde vive cada acción»): Retención, Verificación, Prueba de restauración
  // y «Añadir paso» (copia nueva, espejo, repositorio nuevo a partir de este).
  // Va en la tarjeta de cada copia del editor, en la página de la copia, en la
  // del repositorio y en la ficha del equipo. Cada diálogo es el de siempre.
  import { goto } from "$app/navigation";
  import { CloudUpload, FlaskConical, FolderPlus, GitBranch, HardDrive, History, Plus, ShieldCheck } from "@lucide/svelte";
  import { tip } from "$lib/tooltip";
  import MenuAcciones, { type AccionMenu } from "./MenuAcciones.svelte";
  import OrdenDialog from "./OrdenDialog.svelte";
  import PasoEspejo from "./PasoEspejo.svelte";
  import CopiaDerivada from "./CopiaDerivada.svelte";
  import CambiarRetencion from "./CambiarRetencion.svelte";
  import { ADMITE, admite, repoEnAlmacen } from "$lib/cadenas";
  import { admiteVerificacion, fraseVerificacion } from "$lib/verificacion";
  import { admitePruebaAuto } from "$lib/configEnvio";
  import { informeDe, nVersiones } from "$lib/repo";
  import { cargarCliente } from "$lib/estado.svelte";
  import type { Cliente, Equipo, EquipoDetalle, RepositorioResumen } from "$lib/tipos";

  interface Props {
    cliente: Cliente;
    equipo: EquipoDetalle;
    repo: RepositorioResumen;
    equipos: Equipo[];
    /** Papel de administrador (lo que cambia la configuración o pide la clave de administración). */
    administra: boolean;
    /** Puede mandar órdenes (verificar, probar, retención, derivadas). */
    ordena: boolean;
    /** La copia desde la que se mira («Copia nueva» irá después de ella). */
    copia?: string | null;
    /** En el editor: la verificación y la prueba automáticas están en la tarjeta (no se navega). */
    alAutomatica?: (que: "verificacion" | "prueba") => void;
    /** En el editor: «Copia nueva» añade una tarjeta debajo (no se navega). */
    alNuevaCopia?: () => void;
    /** Algo se envió: recargar lo que se enseña. */
    alCambiar?: () => void;
  }
  let { cliente, equipo, repo, equipos, administra, ordena, copia = null, alAutomatica, alNuevaCopia, alCambiar }: Props = $props();

  const base = $derived(`/c/${cliente.id}/equipos/${equipo.id}`);
  const conVersiones = $derived(nVersiones(repo, informeDe(equipo.ultimo_informe, repo.id)) > 0);
  const conCopias = $derived(!!equipo.resumen?.copias?.some((k) => k.repo === repo.id));
  const enAlmacen = $derived(repoEnAlmacen(equipo, repo, equipos));
  const puedeEspejo = $derived(!!enAlmacen && admite(enAlmacen.almacen, ADMITE.espejoZonas));
  const conDerivadas = $derived(admite(equipo, ADMITE.derivadas));

  let retencion = $state(false);
  let espejo = $state(false);
  let derivada = $state(false);
  let orden = $state<{ tipo: string; descripcion: string; accion: string } | null>(null);

  function automatica(que: "verificacion" | "prueba") {
    if (alAutomatica) alAutomatica(que);
    else void goto(`${base}/copias?${que}=${encodeURIComponent(repo.id)}`);
  }

  const verificacion = $derived<AccionMenu[][]>([
    ordena && conVersiones ? [{ texto: "Verificar ahora", icono: ShieldCheck, onclick: () => (orden = { tipo: "verificar_ahora", descripcion: `Se comprobará ahora una parte de «${repo.nombre}» para confirmar que las copias se pueden leer.`, accion: "Verificar ahora" }) }] : [],
    administra && admiteVerificacion(equipo)
      ? [{ texto: "Automática…", detalle: repo.verificacion_auto ? fraseVerificacion(repo.verificacion_auto) : "Sin verificación automática", onclick: () => automatica("verificacion") }]
      : [],
  ]);
  const prueba = $derived<AccionMenu[][]>([
    ordena && conVersiones ? [{ texto: "Probar ahora", icono: FlaskConical, onclick: () => (orden = { tipo: "probar_restauracion", descripcion: `Se restaurarán unos archivos de «${repo.nombre}» a una carpeta temporal y se compararán. No toca tus archivos.`, accion: "Probar la restauración" }) }] : [],
    administra && admitePruebaAuto(equipo)
      ? [{ texto: "Automática…", detalle: repo.prueba_auto ? `Cada ${repo.prueba_auto.cada_dias} días` : "Sin prueba automática", onclick: () => automatica("prueba") }]
      : [],
  ]);
  const pasos = $derived<AccionMenu[][]>([
    [
      ...(administra ? [{ texto: "Copia nueva (carpetas)", icono: FolderPlus, detalle: copia ? "Después de esta copia" : undefined, onclick: () => (alNuevaCopia ? alNuevaCopia() : void goto(`${base}/copias?${new URLSearchParams({ nueva: "1", ...(copia && admite(equipo, ADMITE.cadenas) ? { tras: copia } : {}) })}`)) }] : []),
      ...(administra
        ? [{ texto: "Espejo de esta copia", icono: HardDrive, detalle: puedeEspejo ? "El mismo repositorio en otro destino" : "Solo si guarda en un almacén actualizado", disabled: !puedeEspejo, onclick: () => (espejo = true) }]
        : []),
      ...(ordena
        ? conDerivadas
          ? [{ texto: "Repositorio nuevo a partir de esta", icono: GitBranch, detalle: conCopias ? "Independiente desde ahí, con su contraseña" : "Primero, una copia que guarde aquí", disabled: !conCopias, onclick: () => (derivada = true) }]
          : !repo.externa && conCopias
            ? [{ texto: "Copia externa…", icono: CloudUpload, detalle: `Actualiza el agente de ${equipo.nombre} para más pasos`, onclick: () => void goto(`${base}?externa=${encodeURIComponent(repo.id)}`) }]
            : []
        : []),
    ],
  ]);

  function cerrar() {
    retencion = espejo = derivada = false;
    orden = null;
    alCambiar?.();
  }
</script>

{#if !repo.solo_lectura && (ordena || administra)}
  <div class="pasos-repo">
    {#if ordena}
      <button type="button" class="btn btn-sm btn-ghost" use:tip={repo.retencion ? `Guarda ${repo.retencion}` : "Sin regla: guarda todas las versiones"} onclick={() => (retencion = true)}><History size={14} />Retención</button>
    {/if}
    <MenuAcciones grupos={verificacion} texto="Verificación" icono={ShieldCheck} etiqueta="Verificación de «{repo.nombre}»" izquierda />
    <MenuAcciones grupos={prueba} texto="Prueba" icono={FlaskConical} etiqueta="Prueba de restauración de «{repo.nombre}»" izquierda />
    <MenuAcciones grupos={pasos} texto="Añadir paso" icono={Plus} clase="btn btn-sm" etiqueta="Añadir un paso a «{repo.nombre}»" izquierda />
  </div>
{/if}

{#if retencion}<CambiarRetencion {cliente} {equipo} {repo} {equipos} onclose={cerrar} />{/if}
{#if espejo}
  <PasoEspejo
    {cliente}
    {equipo}
    {repo}
    {equipos}
    onclose={() => {
      cerrar();
      void cargarCliente(cliente.id, { silencioso: true });
    }}
  />
{/if}
{#if derivada}<CopiaDerivada {cliente} {equipo} {repo} onclose={cerrar} />{/if}
{#if orden}
  <OrdenDialog {cliente} {equipo} tipo={orden.tipo} cuerpo={{ repo: repo.id }} descripcion={orden.descripcion} accion={orden.accion} onclose={cerrar} />
{/if}

<style>
  .pasos-repo {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 4px 6px;
  }
</style>
