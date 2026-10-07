<script lang="ts">
  // Un espejo de un repositorio desde su copia (tarea 7d.2; plan 0.7.26, bloque 4):
  // el mismo repositorio (mismos archivos, misma contraseña) en otro destino. Lo hace
  // el almacén donde está, en local y sin contraseñas. Con un almacén que entiende
  // los trabajos de espejo, abre el editor guiado ya con ese repositorio, su zona y
  // «después de cada copia nueva»; la orden va al almacén con la lista entera de sus
  // espejos (clave de administración). Con uno anterior, lo de antes (PasoEspejoDestinos).
  import EditorEspejo from "./espejos/EditorEspejo.svelte";
  import OrdenDialog from "./OrdenDialog.svelte";
  import PasoEspejoDestinos from "./PasoEspejoDestinos.svelte";
  import { repoEnAlmacen } from "$lib/cadenas";
  import { claveNube, PRINCIPAL, zonasDe } from "$lib/destinos";
  import { admiteTrabajos, AVISO_IGUAL, conTrabajo, cuerpoAlmacen, nombrePorDefecto, paraOrden, textoRetencion, trabajoNuevo, trabajosDelAlmacen, type TrabajoEspejo } from "$lib/espejoTrabajos";
  import type { Cliente, Equipo, RepositorioResumen } from "$lib/tipos";

  interface Props {
    cliente: Cliente;
    /** El equipo dueño del repositorio. */
    equipo: Equipo;
    repo: RepositorioResumen;
    equipos: Equipo[];
    onclose: () => void;
    /** Desde la página de un destino («Usar en una copia»): su clave, ya elegida si sirve. */
    destinoInicial?: string | null;
  }
  let { cliente, equipo, repo, equipos, onclose, destinoInicial = null }: Props = $props();

  // svelte-ignore state_referenced_locally
  const en = repoEnAlmacen(equipo, repo, equipos);
  const almacen = $derived(en?.almacen ? (equipos.find((e) => e.id === en.almacen.id) ?? en.almacen) : null);
  const conTrabajos = $derived(admiteTrabajos(almacen));
  const base = $derived(trabajosDelAlmacen(almacen).map(paraOrden));

  /** El espejo nuevo: ese repositorio, desde su zona, después de cada copia nueva; adónde, lo de la página de origen o la primera otra zona o nube. */
  function inicial(): TrabajoEspejo {
    const t = trabajoNuevo("almacen", base.length);
    t.que = { tipo: "repos", repos: en ? [en.nombre] : [] };
    if (en && en.zona !== PRINCIPAL) t.zona = en.zona;
    t.cuando = { tras_copia: true };
    const m = destinoInicial ? /^(zona|nube):([^:]+):(.+)$/.exec(destinoInicial) : null;
    const otraZona = almacen ? zonasDe(almacen).find((z) => z.id !== (en?.zona ?? PRINCIPAL)) : undefined;
    const nubes = almacen ? [...(almacen.resumen?.guarda_copias?.nubes ?? []), ...(almacen.resumen?.nubes ?? [])] : [];
    const nube = nubes[0];
    const pedida = m && almacen && m[2] === almacen.id && m[1] === "nube" ? nubes.find((n) => claveNube(almacen.id, n.nombre) === destinoInicial) : undefined;
    if (m && almacen && m[2] === almacen.id && m[1] === "zona") t.adonde = { tipo: "zona", carpeta: m[3] };
    else if (pedida) t.adonde = { tipo: "nube", nube: pedida.nombre, carpeta: "Resguardo" };
    else if (otraZona) t.adonde = { tipo: "zona", carpeta: otraZona.id };
    else if (nube) t.adonde = { tipo: "nube", nube: nube.nombre, carpeta: "Resguardo" };
    t.nombre = `Espejo de «${repo.nombre}»`;
    return t;
  }
  // svelte-ignore state_referenced_locally
  const trabajo = conTrabajos ? inicial() : null;
  let guardado = $state<TrabajoEspejo | null>(null);
</script>

{#if almacen && en && !conTrabajos}
  <PasoEspejoDestinos {cliente} {equipo} {repo} {equipos} {onclose} {destinoInicial} />
{:else if almacen && en && trabajo && !guardado}
  <EditorEspejo {cliente} hace={almacen} quien="almacen" {equipos} {trabajo} todos={base} nuevo={true} onguardar={(t) => (guardado = t)} {onclose} />
{:else if almacen && guardado}
  <OrdenDialog
    {cliente}
    equipo={almacen}
    tipo="guarda_copias"
    cuerpo={cuerpoAlmacen(conTrabajo(base, guardado))}
    titulo="Añadir un espejo"
    descripcion={`${almacen.nombre} copiará «${repo.nombre}» de ${equipo.nombre} a otro destino (${guardado.nombre || nombrePorDefecto(guardado.adonde)}): los mismos archivos cifrados, sin abrirlos. Se restaura con la misma contraseña del kit. ${textoRetencion(guardado)}.${guardado.retencion.modo === "igual" ? ` ${AVISO_IGUAL}` : ""}`}
    {onclose}
  />
{/if}
