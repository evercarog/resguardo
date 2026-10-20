<script lang="ts">
  // Un espejo de un repositorio desde su copia (tarea 7d.2; plan 0.7.26, bloque 4):
  // el mismo repositorio (mismos archivos, misma contraseña) en otro destino. Lo hace
  // el almacén donde está, en local y sin contraseñas. Con un almacén que entiende
  // los trabajos de espejo, abre el editor guiado ya con ese repositorio, su zona y
  // «después de cada copia nueva»; la orden va al almacén con la lista entera de sus
  // espejos (clave de administración). Con uno anterior, lo de antes (PasoEspejoDestinos).
  //
  // 0.8.1 (15r): el repositorio de la copia va marcado y arriba en «Qué repositorios», y
  // antes se ofrece **añadirlo a un espejo que ya existe** de esa zona (los de «algunos
  // repositorios»: mismo destino, horario y retención) en vez de crear otro cada vez. Se
  // dice también qué espejos ya lo copian.
  import { HardDrive, Plus, Info } from "@lucide/svelte";
  import Modal from "$ui/componentes/Modal.svelte";
  import EditorEspejo from "./espejos/EditorEspejo.svelte";
  import OrdenDialog from "./OrdenDialog.svelte";
  import { avisar } from "$lib/avisos.svelte";
  import PasoEspejoDestinos from "./PasoEspejoDestinos.svelte";
  import { repoEnAlmacen } from "$lib/cadenas";
  import { claveNube, PRINCIPAL, zonasDe } from "$lib/destinos";
  import { lista as enLista } from "$lib/formato";
  import { admiteTrabajos, AVISO_IGUAL, conTrabajo, cuerpoAlmacen, nombrePorDefecto, paraOrden, textoCuando, textoRetencion, trabajoNuevo, trabajosDelAlmacen, type TrabajoEspejo } from "$lib/espejoTrabajos";
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

  // --- 15r: ¿uno que ya existe? ---
  const zonaDe = (t: TrabajoEspejo) => t.zona || PRINCIPAL;
  const enZona = (t: TrabajoEspejo) => !!en && zonaDe(t) === (en.zona || PRINCIPAL);
  /** Los espejos que ya copian este repositorio (todos, los de su equipo o él por nombre). */
  const loCopian = $derived(
    base.filter((t) => enZona(t) && (t.que.tipo === "todos" || (t.que.tipo === "equipos" && !!en && t.que.equipos.includes(en.nombre.split("/")[0])) || (t.que.tipo === "repos" && !!en && t.que.repos.includes(en.nombre)))),
  );
  /** A los de «algunos repositorios» de esa zona se les puede añadir. */
  const candidatos = $derived(base.filter((t) => enZona(t) && t.que.tipo === "repos" && !loCopian.includes(t)));
  const nombreDe = (t: TrabajoEspejo) => t.nombre || nombrePorDefecto(t.adonde);
  const nombreDeId = (id: string) => {
    const x = base.find((y) => y.id === id);
    return x ? nombreDe(x) : "otro espejo";
  };
  // svelte-ignore state_referenced_locally
  let eleccion = $state<string>(candidatos[0]?.id ?? "nuevo");
  // svelte-ignore state_referenced_locally
  let fase = $state<"elegir" | "editor" | "orden">(candidatos.length || loCopian.length ? "elegir" : "editor");

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
  /** Añadido a uno que ya existe (la orden lleva ese con el repositorio de más). */
  let anadidoA = $state<TrabajoEspejo | null>(null);

  function seguir(e: SubmitEvent) {
    e.preventDefault();
    if (eleccion === "nuevo") return void (fase = "editor");
    const t = candidatos.find((x) => x.id === eleccion);
    if (!t || t.que.tipo !== "repos" || !en) return;
    anadidoA = { ...t, que: { tipo: "repos", repos: [...t.que.repos, en.nombre] } };
    guardado = anadidoA;
    fase = "orden";
  }
</script>

{#if almacen && en && !conTrabajos}
  <PasoEspejoDestinos {cliente} {equipo} {repo} {equipos} {onclose} {destinoInicial} />
{:else if almacen && en && trabajo && fase === "elegir"}
  <Modal labelledby="t-paso-espejo" {onclose} width={540}>
    <form class="form" onsubmit={seguir}>
      <div class="dlg-title">
        <span class="ticon"><HardDrive size={18} /></span>
        <div>
          <h2 id="t-paso-espejo">Añadir un espejo de «{repo.nombre}»</h2>
          <p>{almacen.nombre} lo copiará a otro destino, tal cual. Puede ir en un espejo que ya tienes (con su destino, su horario y su retención) o en uno nuevo.</p>
        </div>
      </div>
      {#if loCopian.length}
        <div class="notice notice-info"><Info size={16} /><p>Ya lo {loCopian.length === 1 ? "copia" : "copian"} {enLista(loCopian.map((t) => `«${nombreDe(t)}»`))}. Añádelo a otro solo si quieres una copia más en otro sitio.</p></div>
      {/if}
      <div class="opciones" role="radiogroup" aria-label="Dónde va el espejo">
        {#each candidatos as t (t.id)}
          <label class="opcion" class:on={eleccion === t.id}>
            <input type="radio" name="paso-espejo" value={t.id} checked={eleccion === t.id} onchange={() => (eleccion = t.id)} />
            <span class="txt">
              <span>Añadir a «{nombreDe(t)}»</span>
              <span class="faint">{t.que.tipo === "repos" ? `Ya lleva ${t.que.repos.length === 1 ? "1 repositorio" : `${t.que.repos.length} repositorios`}` : ""} · {textoCuando(t, nombreDeId)}</span>
            </span>
          </label>
        {/each}
        <label class="opcion" class:on={eleccion === "nuevo"}>
          <input type="radio" name="paso-espejo" value="nuevo" checked={eleccion === "nuevo"} onchange={() => (eleccion = "nuevo")} />
          <span class="txt"><span><Plus size={13} />Un espejo nuevo</span><span class="faint">Eliges adónde, cuándo y su retención.</span></span>
        </label>
      </div>
      <footer>
        <button type="button" class="btn btn-ghost" onclick={onclose}>Cancelar</button>
        <button class="btn btn-primary">Seguir</button>
      </footer>
    </form>
  </Modal>
{:else if almacen && en && trabajo && fase === "editor" && !guardado}
  <EditorEspejo {cliente} hace={almacen} quien="almacen" {equipos} {trabajo} todos={base} nuevo={true} primero={en.nombre} primeroNombre={{ nombre: repo.nombre, equipo: equipo.nombre }} onguardar={(t) => ((guardado = t), (fase = "orden"))} {onclose} />
{:else if almacen && guardado}
  <OrdenDialog
    {cliente}
    equipo={almacen}
    tipo="guarda_copias"
    cuerpo={cuerpoAlmacen(conTrabajo(base, guardado))}
    titulo={anadidoA ? `Añadir a «${nombreDe(anadidoA)}»` : "Añadir un espejo"}
    descripcion={anadidoA
      ? `${almacen.nombre} copiará también «${repo.nombre}» de ${equipo.nombre} en «${nombreDe(anadidoA)}», con su mismo destino, horario y retención: los mismos archivos cifrados, sin abrirlos. La primera vez lo sube entero.`
      : `${almacen.nombre} copiará «${repo.nombre}» de ${equipo.nombre} a otro destino (${guardado.nombre || nombrePorDefecto(guardado.adonde)}): los mismos archivos cifrados, sin abrirlos. Se restaura con la misma contraseña del kit. ${textoRetencion(guardado)}.${guardado.retencion.modo === "igual" ? ` ${AVISO_IGUAL}` : ""}`}
    alTerminar={(o) => {
      if (o.estado === "hecha") avisar(anadidoA ? `«${repo.nombre}» va ahora también en «${nombreDe(anadidoA)}».` : `Espejo «${guardado?.nombre || (guardado ? nombrePorDefecto(guardado.adonde) : "nuevo")}» añadido: empieza con su primera subida.`);
    }}
    {onclose}
  />
{/if}

<style>
  .opciones {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .opcion {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    padding: 8px 10px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    font-size: var(--fs-sm);
    cursor: pointer;
  }
  .opcion.on {
    border-color: var(--accent);
    background: var(--surface-2);
  }
  .opcion input {
    margin-top: 3px;
  }
  .txt {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
    overflow-wrap: anywhere;
  }
  .txt > span:first-child {
    display: inline-flex;
    align-items: center;
    gap: 4px;
  }
  .txt .faint {
    font-size: var(--fs-xs);
  }
</style>
