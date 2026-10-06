<script lang="ts">
  // Paso «espejo» de una copia (tarea 7d.2, docs/copias-en-cadena.md): el mismo
  // repositorio (mismos archivos, misma contraseña) en otro destino. Lo hace el
  // almacén donde está, en local y sin contraseñas, con el motor del espejo: es
  // un destino de su espejo con solo ese repositorio, desde su zona y «después
  // de cada copia». La orden va al almacén (`guarda_copias { espejo }`, clave de
  // administración), con los demás destinos de su espejo tal cual.
  import { TriangleAlert } from "@lucide/svelte";
  import OrdenDialog from "./OrdenDialog.svelte";
  import Ayuda from "./Ayuda.svelte";
  import { destinosParaPasos, detalleDestino, repoEnAlmacen, TEXTO_FUERA_RETENCION, usosPosibles } from "$lib/cadenas";
  import { catalogoDe, cargarCatalogo } from "$lib/catalogoDestinos.svelte";
  import ElegirDestinoPaso, { type OpcionDestino } from "./ElegirDestinoPaso.svelte";
  import ConectarNube from "./ConectarNube.svelte";
  import ConectarDestino from "./ConectarDestino.svelte";
  import { zonasDe, nombreZonaPorDefecto, PRINCIPAL } from "$lib/destinos";
  import { destinoParaOrden, errorDiasRetencion, horaParaConsolasAnteriores, horarioDiario, nombreTipoNube, RETENCION_ESPEJO, TIPOS_NUBE, type DestinoEspejoOrden } from "$lib/espejo";
  import { errorCarpetaEspejo } from "$lib/ganchos";
  import type { Cliente, Equipo, RepositorioResumen } from "$lib/tipos";

  interface Props {
    cliente: Cliente;
    /** El equipo dueño del repositorio. */
    equipo: Equipo;
    repo: RepositorioResumen;
    equipos: Equipo[];
    onclose: () => void;
  }
  let { cliente, equipo, repo, equipos, onclose }: Props = $props();

  // svelte-ignore state_referenced_locally
  const en = repoEnAlmacen(equipo, repo, equipos);
  const almacen = $derived(en?.almacen ? (equipos.find((e) => e.id === en.almacen.id) ?? en.almacen) : null);
  const zonas = $derived(almacen ? zonasDe(almacen) : []);
  const otrasZonas = $derived(zonas.filter((z) => z.id !== (en?.zona ?? PRINCIPAL)));
  const nubes = $derived(almacen?.resumen?.guarda_copias?.nubes ?? []);
  const espejo = $derived(almacen?.resumen?.guarda_copias?.espejo ?? null);
  const win = $derived(/windows/i.test(almacen?.so ?? ""));

  // svelte-ignore state_referenced_locally
  let f = $state({
    destino: otrasZonas[0] ? `zona:${otrasZonas[0].id}` : nubes[0] ? `nube:${nubes[0].nombre}` : "carpeta",
    carpeta: "",
    carpetaNube: "Resguardo",
    conRetencion: false,
    dias: RETENCION_ESPEJO.defecto as number,
  });
  const nuevo = $derived.by<DestinoEspejoOrden>(() => {
    const base = { repos: en ? [en.nombre] : [], vistos: [], tras_copia: true, ...(en && en.zona !== PRINCIPAL ? { zona: en.zona } : {}), ...(f.conRetencion ? { retencion_dias: f.dias } : {}) };
    if (f.destino.startsWith("zona:")) return { tipo: "zona", carpeta: f.destino.slice(5), ...base };
    if (f.destino.startsWith("nube:")) return { tipo: "nube", nube: f.destino.slice(5), carpeta: f.carpetaNube.trim(), ...base };
    return { tipo: "carpeta", carpeta: f.carpeta.trim(), ...base };
  });
  /** Los destinos que ya tiene el espejo, tal cual (con el horario de siempre si no tenían), y el nuevo. */
  const destinos = $derived([...(espejo?.destinos ?? []).map((d) => {
    const o = destinoParaOrden(d);
    return o.horario ? o : { ...o, horario: horarioDiario(espejo?.hora ?? "02:00") };
  }), nuevo]);
  const cuerpo = $derived({ espejo: { destinos, hora: horaParaConsolasAnteriores(destinos, espejo?.hora ?? "02:00"), ...(espejo?.limite_kib ? { limite_kib: espejo.limite_kib } : {}) } });
  const errorCarpeta = $derived(f.destino === "carpeta" && f.carpeta.trim() ? errorCarpetaEspejo(f.carpeta, win) : null);
  const valido = $derived(
    !!en && (f.destino !== "carpeta" || (!!f.carpeta.trim() && !errorCarpeta)) && (!f.destino.startsWith("nube:") || !!f.carpetaNube.trim()) && (!f.conRetencion || !errorDiasRetencion(f.dias)),
  );
  // Todos los destinos del cliente (docs/editor-de-copias.md): los que no sirven para el espejo, con el porqué.
  // svelte-ignore state_referenced_locally
  void cargarCatalogo(cliente.id);
  const opciones = $derived.by<OpcionDestino[]>(() => {
    const l: OpcionDestino[] = destinosParaPasos(equipos, catalogoDe(cliente.id)).map((v) => {
      const uso = usosPosibles(v, equipo, repo, equipos).espejo;
      const valor = uso.ok && v.zona ? `zona:${v.zona.id}` : uso.ok && v.nube ? `nube:${v.nube.nombre}` : `no:${v.clave}`;
      return { valor, nombre: v.nombre, detalle: detalleDestino(v), clase: v.clase, uso };
    });
    if (almacen) l.push({ valor: "carpeta", nombre: `Otra carpeta de ${almacen.nombre}…`, clase: "carpeta", uso: { ok: true } });
    // Primero los que se pueden usar.
    return l.sort((a, b) => Number(b.uso.ok) - Number(a.uso.ok));
  });
  let conectar = $state<{ equipo: Equipo; nube: string; tipo: string } | null>(null);
  const tipoNube = $derived(f.destino.startsWith("nube:") ? nubes.find((n) => `nube:${n.nombre}` === f.destino)?.tipo : undefined);
</script>

{#if almacen && en}
  <OrdenDialog
    {cliente}
    equipo={almacen}
    tipo="guarda_copias"
    {cuerpo}
    titulo="Paso «espejo»"
    descripcion={`${almacen.nombre} copiará el repositorio «${repo.nombre}» de ${equipo.nombre} a otro destino después de cada copia nueva: los mismos archivos cifrados, sin abrirlos (no necesita la contraseña). Se restaura con la misma contraseña del kit.`}
    {valido}
    {onclose}
  >
    {#snippet campos()}
      <ElegirDestinoPaso id="pe-destino" etiqueta="Copiar a" {opciones} bind:value={f.destino} alConectar={(e, nube, tipo) => (conectar = { equipo: e, nube, tipo })} />
      {#if f.destino === "carpeta"}
        <div class="field">
          <label class="field-label" for="pe-carpeta">Carpeta</label>
          <input id="pe-carpeta" class="input mono" bind:value={f.carpeta} placeholder={win ? "E:\\Resguardo-espejo" : "/mnt/disco2/espejo"} spellcheck="false" />
          {#if errorCarpeta}<p class="error-campo">{errorCarpeta}</p>{/if}
        </div>
      {:else if f.destino.startsWith("nube:")}
        <div class="field">
          <label class="field-label" for="pe-nube">Carpeta dentro de la nube</label>
          <input id="pe-nube" class="input mono" bind:value={f.carpetaNube} spellcheck="false" />
        </div>
        {#if tipoNube && TIPOS_NUBE[tipoNube] && !TIPOS_NUBE[tipoNube].inmutable}
          <div class="notice notice-warn"><TriangleAlert size={16} /><p>{TIPOS_NUBE[tipoNube].nombre} no es inmutable: alguien con acceso a la cuenta podría borrar lo de allí.</p></div>
        {/if}
      {/if}
      <label class="switch-row"><input type="checkbox" bind:checked={f.conRetencion} /><span>Con la retención del original<span class="faint">Lo que la retención quite del original se borra del espejo pasados unos días (con freno si falta mucho de golpe). Sin ella, nunca borra.</span></span></label>
      {#if f.conRetencion}
        <div class="field">
          <label class="field-label" for="pe-dias">Borrar a los (días)</label>
          <input id="pe-dias" class="input num corto" type="number" min={RETENCION_ESPEJO.min} max={RETENCION_ESPEJO.max} bind:value={f.dias} />
          {#if errorDiasRetencion(f.dias)}<p class="error-campo">{errorDiasRetencion(f.dias)}</p>{/if}
        </div>
        <p class="faint nota">{TEXTO_FUERA_RETENCION}</p>
      {/if}
      <p class="faint nota">Desde {zonas.find((z) => z.id === en.zona) ? nombreZonaPorDefecto(zonas.find((z) => z.id === en.zona)!) : almacen.nombre}, carpeta <code>{en.nombre}</code>. Después de cada copia nueva (y, por si acaso, cada noche a las {espejo?.hora ?? "02:00"}). <Ayuda id="espejo" /></p>
    {/snippet}
  </OrdenDialog>
{/if}

{#if conectar}
  {#if conectar.tipo === "dropbox"}
    <ConectarNube {cliente} equipo={conectar.equipo} nombreInicial={conectar.nube} onclose={() => (conectar = null)} />
  {:else}
    <ConectarDestino {cliente} equipo={conectar.equipo} onclose={() => (conectar = null)} />
  {/if}
{/if}

<style>
  .nota {
    margin: 0;
    font-size: var(--fs-xs);
  }
  .corto {
    max-width: 140px;
  }
</style>
