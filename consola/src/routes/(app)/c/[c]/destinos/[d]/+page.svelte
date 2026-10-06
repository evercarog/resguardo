<script lang="ts">
  // La página de un destino (como la de un repositorio): una zona de un
  // almacén, un disco o carpeta de un equipo, una nube o un destino suelto del
  // catálogo. Cabecera con su nombre, qué es, dónde está y sus acciones
  // («Usar en una copia», «Nombre», «Regla 3-2-1», «Quitar este destino» con la
  // clave de administración); cifras; dónde está (sistema de archivos, entorno);
  // lo que sabe la regla 3-2-1-1-0; sus repositorios; lo que lo usa (copias
  // externas y derivadas, el espejo); dónde se puede usar; y lo último que pasó.
  // Todo sale de los resúmenes de los equipos y del catálogo (lib/fichaDestino.ts).
  import { untrack } from "svelte";
  import { page } from "$app/state";
  import { ArrowRight, Cloud, Database, HardDrive, Lock, Network, Pencil, Server, ShieldCheck, Trash2, TriangleAlert, Usb, CircleCheck, CircleAlert, Activity, Plus, StickyNote } from "@lucide/svelte";
  import { actual, cargarCliente, puede, reloj } from "$lib/estado.svelte";
  import { cargarInformes, ultimos } from "$lib/informes.svelte";
  import { catalogoDe, cargarCatalogo } from "$lib/catalogoDestinos.svelte";
  import { bytes, numero, plural, relativo } from "$lib/formato";
  import { tip } from "$lib/tooltip";
  import { bytesRepo, estadoRepo, informeDe, nVersiones } from "$lib/repo";
  import { TEXTO_TIPO } from "$lib/destinos";
  import { nombreTipoNube } from "$lib/espejo";
  import { destinoQuitable } from "$lib/datosEquipo";
  import { actividadDestino, reposEnDestino, usosDeDestino, usosPosibles, vistaPorClave } from "$lib/fichaDestino";
  import { marcarDesdeVista, TEXTO_INMUTABLE, TEXTO_LUGAR, textoEntorno, type MarcarDestino } from "$lib/regla321";
  import type { DestinoResumen, Equipo } from "$lib/tipos";
  import CabeceraPagina from "$lib/componentes/CabeceraPagina.svelte";
  import Cifra from "$lib/componentes/Cifra.svelte";
  import Chip from "$lib/componentes/Chip.svelte";
  import Esqueleto from "$lib/componentes/Esqueleto.svelte";
  import Tiempo from "$lib/componentes/Tiempo.svelte";
  import Vacio from "$lib/componentes/Vacio.svelte";
  import Ayuda from "$lib/componentes/Ayuda.svelte";
  import OrdenDialog from "$lib/componentes/OrdenDialog.svelte";
  import RenombrarDestino from "$lib/componentes/RenombrarDestino.svelte";
  import AtributosDestino from "$lib/componentes/regla/AtributosDestino.svelte";
  import NotasDialogo from "$lib/componentes/notas/NotasDialogo.svelte";
  import ContadorNotas from "$lib/componentes/notas/ContadorNotas.svelte";

  const c = $derived(page.params.c ?? "");
  // SvelteKit ya decodifica el parámetro (las claves solo llevan [a-z0-9:_.-]).
  const clave = $derived(page.params.d ?? "");
  const administra = $derived(puede.administrar(actual.cliente?.rol));

  $effect(() => {
    const cc = actual.id;
    if (cc) untrack(() => void cargarCatalogo(cc));
  });
  $effect(() => {
    // Las cargas, sin seguir lo que leen (regla del $effect: AGENTS.md).
    const [cc, ids] = [actual.id, actual.equipos.map((e) => e.id)];
    untrack(() => void cargarInformes(cc, ids));
  });

  const v = $derived(vistaPorClave(clave, actual.equipos, catalogoDe(actual.id)));
  const suyos = $derived(v ? reposEnDestino(v, actual.equipos) : []);
  const filas = $derived(
    suyos.map(({ equipo: e, repo: r }) => {
      const inf = informeDe(ultimos.porEquipo[e.id], r.id);
      return { e, r, versiones: nVersiones(r, inf), tam: bytesRepo(r, inf), estado: estadoRepo(r, inf, e.resumen?.copias ?? [], reloj.ahora) };
    }),
  );
  const protegido = $derived(filas.reduce((n, x) => n + (x.tam ?? 0), 0));
  const usos = $derived(v ? usosDeDestino(v, actual.equipos, c) : []);
  const posibles = $derived(v ? usosPosibles(v, actual.equipos, c) : []);
  const actividad = $derived(v ? actividadDestino(v, actual.equipos, c) : []);
  const marca = $derived<MarcarDestino | null>(v ? marcarDesdeVista(v, actual.equipos) : null);
  const atrib = $derived(v?.catalogo?.atributos ?? null);
  const entorno = $derived(marca?.equipo ? textoEntorno(marca.equipo) : null);
  const d = $derived<DestinoResumen | undefined>(v?.destino);
  // El espacio libre: el de la zona o el que dio el espejo de esa nube.
  const espacio = $derived.by(() => {
    if (v?.zona?.espacio) return v.zona.espacio;
    if (v?.nube) return v.nube.equipo.resumen?.guarda_copias?.espejo?.destinos?.find((x) => x.tipo === "nube" && x.nube === v!.nube!.nombre)?.espacio ?? null;
    return null;
  });
  const equiposQueLoUsan = $derived(actual.equipos.filter((e) => v?.equipos.includes(e.nombre)));

  /** Qué es, en palabras: «Almacén · zona principal», «Disco extraíble de CAJA-1», «Dropbox · conectada en ALMACEN-01»… */
  const queEs = $derived.by(() => {
    if (!v) return "";
    if (v.zona) return `${v.zona.principal ? "Almacén · zona principal" : "Almacén · otra zona"} de ${v.zona.almacen.nombre}`;
    if (v.nube) return `${nombreTipoNube(v.nube.tipo)} · conectada en ${v.nube.equipo.nombre}`;
    if (d?.tipo === "local") return `${d.red ? "Carpeta de otra máquina de la red" : d.extraible ? "Disco extraíble" : "Carpeta"} de ${v.equipos.join(", ")}${d.unidad ? ` (${d.unidad})` : ""}`;
    if (v.clase === "suelto") return `${TEXTO_TIPO[v.tipo] ?? v.tipo} · sin repositorios todavía`;
    return TEXTO_TIPO[v.tipo] ?? v.tipo;
  });
  const Icono = $derived(v?.zona ? Server : v?.nube ? Cloud : d?.tipo === "local" ? (d.extraible ? Usb : d.red ? Network : HardDrive) : ["b2", "s3", "nube"].includes(v?.tipo ?? "") ? Cloud : v?.tipo === "rest" || v?.tipo === "sftp" ? Server : Database);
  const mismoEquipo = $derived(d?.tipo === "local" && !d.red && !d.extraible);

  // «Quitar este destino» (v1.56; con la clave de administración desde la v1.4x): en cada equipo que lo tiene y ya no lo usa.
  const quitables = $derived(v && v.clase === "equipo" && !suyos.length ? actual.equipos.flatMap((e) => (e.resumen?.destinos ?? []).filter((x) => v!.ids.includes(x.id) && destinoQuitable(e, x.id)).map((x) => ({ equipo: e, destino: x }))) : []);
  let quitar = $state<{ equipo: Equipo; destino: DestinoResumen } | null>(null);
  let renombrar = $state(false);
  let marcar = $state(false);
  let notas = $state(false);
  let verTodosUsos = $state(false);
  const usosVista = $derived(verTodosUsos ? posibles : posibles.slice(0, 6));
  const recargar = () => actual.id && void cargarCliente(actual.id, { silencioso: true });
</script>

<svelte:head><title>{v?.nombre ?? "Destino"} · {actual.cliente?.nombre ?? ""} · Resguardo Server</title></svelte:head>

<div class="page">
  {#if !actual.cargado || !actual.cliente}
    <Esqueleto forma="cifras" n={4} />
    <Esqueleto forma="tarjetas" n={2} />
  {:else if !v}
    <div class="card">
      <Vacio icono={Database} ilustracion="no-encontrado" titulo="Este destino ya no está" texto="Puede que se quitara la zona o se desconectara la nube. Los demás destinos están en «Repositorios y destinos».">
        <a class="btn btn-primary" href="/c/{c}/repositorios#destinos">Ver los destinos</a>
      </Vacio>
    </div>
  {:else}
    <CabeceraPagina
      titulo={v.nombre}
      icono={Icono}
      migas={[{ texto: actual.cliente.nombre, href: `/c/${c}` }, { texto: "Repositorios y destinos", href: `/c/${c}/repositorios#destinos` }, { texto: v.nombre }]}
      resumen="{queEs}{v.renombrado ? ` · antes «${v.nombrePorDefecto}»` : ''}"
    >
      {#snippet acciones()}
        {#if posibles.length}<a class="btn btn-primary" href="#usar"><Plus size={16} />Usar en una copia</a>{/if}
        {#if administra}<button class="btn" onclick={() => (renombrar = true)}><Pencil size={15} />Nombre</button>{/if}
        {#if administra && v.clase !== "suelto"}<button class="btn" onclick={() => (marcar = true)} use:tip={"Dónde está y si es inmutable, para la regla 3-2-1-1-0"}><ShieldCheck size={15} />Regla 3-2-1</button>{/if}
        {#if d}<button class="btn btn-ghost" onclick={() => (notas = true)}><StickyNote size={15} />Notas <ContadorNotas tipo="destino" objeto={d.id} /></button>{/if}
      {/snippet}
    </CabeceraPagina>

    <div class="cifras" role="list" aria-label="Cifras del destino">
      <Cifra icono={Database} etiqueta="Repositorios" valor={numero(filas.length)} sub={filas.length ? `de ${plural(new Set(filas.map((x) => x.e.id)).size, "equipo", "equipos")}` : v.clase === "nube" ? "para el espejo del almacén" : "ninguno todavía"} />
      <Cifra icono={HardDrive} etiqueta="Protegido" valor={protegido ? bytes(protegido) : "—"} sub={filas.length ? "lo que ocupan sus versiones" : "sin repositorios"} />
      <Cifra icono={Server} etiqueta="Espacio libre" valor={espacio ? bytes(espacio.libre) : "—"} de={espacio ? `de ${bytes(espacio.total)}` : undefined} sub={espacio ? (espacio.leido ? `medido ${relativo(espacio.leido, reloj.ahora)}` : "lo último que dijo") : v.clase === "nube" || ["b2", "s3"].includes(v.tipo) ? "el proveedor no dice un límite" : "no se sabe todavía"} />
      <Cifra icono={Activity} etiqueta="Lo último" valor={actividad[0] ? relativo(actividad[0].cuando, reloj.ahora) : "—"} sub={actividad[0] ? actividad[0].texto : "nada todavía"} mal={actividad[0]?.tono === "bad"} />
    </div>

    <div class="dos">
      <section class="card p" aria-labelledby="t-donde">
        <h2 class="section-title" id="t-donde">Dónde está</h2>
        <dl class="datos">
          <dt>Qué es</dt>
          <dd>{queEs}</dd>
          {#if v.donde}<dt>Dirección</dt><dd><span class="pastilla mono">{v.donde}</span></dd>{/if}
          {#if v.zona}
            <dt>Almacén</dt>
            <dd><a class="link" href="/c/{c}/equipos/{v.zona.almacen.id}">{v.zona.almacen.nombre}</a>{#if v.zona.puerto}{" · puerto "}<span class="pastilla mono">{v.zona.puerto}</span>{/if}</dd>
            <dt>Equipos</dt>
            <dd>{plural(v.zona.usuarios, "equipo copia aquí", "equipos copian aquí")}</dd>
          {:else if equiposQueLoUsan.length}
            <dt>{v.clase === "nube" ? "Conectada en" : "Lo tiene"}</dt>
            <dd>{#each equiposQueLoUsan as e, i (e.id)}{i ? ", " : ""}<a class="link" href="/c/{c}/equipos/{e.id}">{e.nombre}</a>{/each}</dd>
          {/if}
          {#if marca?.sistemaArchivos}<dt>Sistema de archivos</dt><dd><span class="pastilla mono">{marca.sistemaArchivos}</span> <span class="faint">solo un dato</span></dd>{/if}
          {#if entorno}<dt>Entorno</dt><dd>{entorno}</dd>{/if}
        </dl>
        <div class="chips">
          {#if v.zona}
            <span class="badge badge-sm tone-ok" use:tip={"Los equipos pueden añadir copias, pero no borrarlas: protege contra el ransomware."}><Lock size={11} />Solo añadir</span>
            {#if v.zona.almacen.resumen?.guarda_copias?.solo_red_local}<span class="badge badge-sm tone-neutral">Solo red local</span>{/if}
            {#if v.zona.escucha === false}<span class="badge badge-sm tone-warn"><TriangleAlert size={11} />Sin responder</span>{/if}
          {/if}
          {#if d?.inmutable}<span class="badge badge-sm tone-ok"><Lock size={11} />Inmutable<Ayuda id="inmutable" /></span>{/if}
          {#if d?.tipo === "local" && d.red}<span class="badge badge-sm tone-neutral"><Network size={11} />En otra máquina</span>{/if}
          {#if d?.tipo === "local" && d.extraible}<span class="badge badge-sm tone-neutral"><Usb size={11} />Extraíble</span>{/if}
          {#if mismoEquipo}<span class="badge badge-sm tone-warn" use:tip={"Las copias se quedan en el mismo equipo que protegen: si se daña o lo cifra un ransomware, se pierden las dos."}><TriangleAlert size={11} />En el mismo equipo</span>{/if}
          {#if v.nube && !["b2", "s3"].includes(v.nube.tipo)}<span class="badge badge-sm tone-neutral" use:tip={"Quien tenga su permiso puede borrar lo copiado: conviene que otro destino sea inmutable."}>No inmutable</span>{/if}
        </div>
      </section>

      {#if marca && v.clase !== "suelto"}
        <section class="card p" aria-labelledby="t-regla">
          <div class="cab-sec">
            <h2 class="section-title" id="t-regla"><ShieldCheck size={16} />Para la regla 3-2-1-1-0 <Ayuda id="regla-321" /></h2>
            {#if administra}<button class="btn btn-sm btn-ghost" onclick={() => (marcar = true)}>Cambiar</button>{/if}
          </div>
          <dl class="datos">
            <dt>Dónde está</dt>
            <dd>{TEXTO_LUGAR[atrib?.lugar ?? marca.porDefecto.lugar]} <span class="faint">· {atrib?.lugar ? "marcado" : "deducido del tipo"}</span></dd>
            <dt>Inmutable</dt>
            <dd>{TEXTO_INMUTABLE[atrib?.inmutable ?? marca.porDefecto.inmutable]} <span class="faint">· {atrib?.inmutable ? "marcado" : "deducido del tipo"}</span></dd>
            <dt>Soporte</dt>
            <dd>{atrib?.soporte ? `«${atrib.soporte}» (marcado)` : "Su equipo y su disco (dos destinos con el mismo soporte cuentan una vez)"}</dd>
          </dl>
        </section>
      {/if}
    </div>

    <section aria-labelledby="t-repos">
      <div class="section-head"><h2 id="t-repos">Repositorios aquí <span class="count">· {filas.length}</span> <Ayuda id="repositorio" /></h2></div>
      {#if filas.length}
        <div class="card p-0">
          <ul class="lista-d">
            {#each filas as x (x.e.id + x.r.id)}
              <li>
                <a class="fila-d" href="/c/{c}/equipos/{x.e.id}/repositorios/{encodeURIComponent(x.r.id)}">
                  <Database size={16} class="ic" />
                  <span class="fd-texto"><strong>{x.r.nombre}</strong><span class="faint">{x.e.nombre} · {plural(x.versiones, "versión", "versiones")}{x.tam ? ` · ${bytes(x.tam)}` : ""}</span></span>
                  <Chip pequeno tono={x.estado.tono} texto={x.estado.texto} />
                  <ArrowRight size={14} class="flecha" />
                </a>
              </li>
            {/each}
          </ul>
        </div>
      {:else}
        <p class="faint vacio-linea">{v.clase === "nube" ? "Una nube conectada en un almacén no guarda repositorios propios: recibe el espejo o copias derivadas." : "Ningún repositorio se guarda aquí todavía."}</p>
      {/if}
    </section>

    {#if usos.length}
      <section aria-labelledby="t-usos">
        <div class="section-head"><h2 id="t-usos">Espejos y copias que lo usan <span class="count">· {usos.length}</span></h2></div>
        <div class="card p-0">
          <ul class="lista-d">
            {#each usos as u, i (i)}
              <li>
                <a class="fila-d" href={u.href}>
                  {#if u.tipo === "espejo_sale"}<ArrowRight size={16} class="ic" />{:else}<Cloud size={16} class="ic" />{/if}
                  <span class="fd-texto"><strong>{u.texto}</strong><span class="faint">{u.tipo === "externa" ? "Copia externa: llega aquí" : u.tipo === "derivada" ? "Repositorio a partir de otro: llega aquí" : u.tipo === "espejo_entra" ? "El almacén refleja aquí lo que guarda" : "Lo que guarda esta zona sale a otro sitio"}{#if u.ultima}{" · "}<Tiempo iso={u.ultima} />{/if}</span></span>
                  {#if u.resultado}<Chip pequeno tono={u.resultado === "fallo" ? "bad" : u.resultado === "aviso" ? "warn" : "ok"} texto={u.resultado === "fallo" ? "Falló" : u.resultado === "aviso" ? "Con avisos" : "Al día"} />{/if}
                  <ArrowRight size={14} class="flecha" />
                </a>
              </li>
            {/each}
          </ul>
        </div>
      </section>
    {/if}

    <section id="usar" aria-labelledby="t-usar">
      <div class="section-head"><h2 id="t-usar">Usar en una copia</h2></div>
      {#if posibles.length}
        <p class="explica">
          {#if v.clase === "nube"}Esta nube está conectada en {v.nube?.equipo.nombre}: sirve para el <strong>espejo</strong> de lo que guarda ese almacén o para un <strong>repositorio nuevo a partir de</strong> otro, en el equipo dueño.
          {:else if v.clase === "suelto"}Todavía no tiene repositorios: créale uno y elige qué copiar.
          {:else}Una copia nueva de las carpetas de un equipo puede guardarse aquí.{/if}
        </p>
        <div class="card p-0">
          <ul class="lista-d">
            {#each usosVista as u (u.href)}
              <li>
                <a class="fila-d" href={u.href}>
                  <Plus size={16} class="ic" />
                  <span class="fd-texto"><strong>{u.texto}</strong><span class="faint">{u.detalle}</span></span>
                  <ArrowRight size={14} class="flecha" />
                </a>
              </li>
            {/each}
          </ul>
        </div>
        {#if posibles.length > 6}<button class="btn btn-sm btn-ghost mas" onclick={() => (verTodosUsos = !verTodosUsos)}>{verTodosUsos ? "Ver menos" : `Ver las ${posibles.length}`}</button>{/if}
      {:else}
        <p class="faint vacio-linea">{v.clase === "nube" ? `Ningún repositorio está todavía en el almacén ${v.nube?.equipo.nombre}: cuando haya alguno, podrás reflejarlo aquí.` : "Ningún equipo puede usarlo ahora."}</p>
      {/if}
    </section>

    <section aria-labelledby="t-act">
      <div class="section-head"><h2 id="t-act">Lo último que pasó</h2></div>
      {#if actividad.length}
        <div class="card p-0">
          <ul class="lista-d">
            {#each actividad as s, i (i)}
              <li>
                <a class="fila-d" href={s.href}>
                  <span class="ic-estado tone-{s.tono}">{#if s.tono === "ok"}<CircleCheck size={16} />{:else if s.tono === "warn"}<TriangleAlert size={16} />{:else}<CircleAlert size={16} />{/if}</span>
                  <span class="fd-texto"><strong>{s.texto}</strong><span class="faint" class:mal={s.tono === "bad"}>{s.tono === "ok" ? "Correcta" : s.tono === "warn" ? "Con avisos" : "Falló"}{#if s.detalle}{": "}{s.detalle}{/if}</span></span>
                  <span class="faint num"><Tiempo iso={s.cuando} /></span>
                </a>
              </li>
            {/each}
          </ul>
        </div>
      {:else}
        <p class="faint vacio-linea">Nada todavía: aquí saldrá la última copia de cada repositorio y el espejo que llega o sale.</p>
      {/if}
    </section>

    {#if administra && quitables.length}
      <section class="card p peligro" aria-labelledby="t-quitar">
        <h2 class="section-title" id="t-quitar">Quitar este destino</h2>
        <p class="faint">Ya no lo usa nada. El equipo olvida el destino y sus credenciales; lo guardado allí se queda. Pide la clave de administración.</p>
        <div class="chips">
          {#each quitables as q (q.equipo.id + q.destino.id)}
            <button class="btn btn-sm quitar-dest" onclick={() => (quitar = q)}><Trash2 size={13} />{quitables.length > 1 ? `Quitar de ${q.equipo.nombre}` : "Quitar este destino"}</button>
          {/each}
        </div>
      </section>
    {/if}
  {/if}
</div>

{#if renombrar && v && actual.id}<RenombrarDestino cliente={actual.id} destino={v} onclose={() => (renombrar = false)} />{/if}
{#if marcar && marca && actual.id}<AtributosDestino cliente={actual.id} destino={marca} onclose={() => (marcar = false)} />{/if}
{#if notas && d && v}<NotasDialogo tipo="destino" objeto={d.id} nombre={v.nombre} onclose={() => (notas = false)} />{/if}
{#if quitar && actual.cliente}
  <OrdenDialog
    cliente={actual.cliente}
    equipo={quitar.equipo}
    tipo="quitar_destino"
    cuerpo={{ destino: quitar.destino.id }}
    titulo="Quitar este destino"
    descripcion={quitar.destino.tipo === "local"
      ? `${quitar.equipo.nombre} olvidará «${quitar.destino.nombre}». No se borra nada de su carpeta: si aún tiene copias guardadas, te lo dirá y seguirán ahí.`
      : `${quitar.equipo.nombre} olvidará «${quitar.destino.nombre}» y sus credenciales. Lo guardado allí se queda.`}
    accion="Quitar el destino"
    onclose={() => ((quitar = null), recargar())}
    alTerminar={recargar}
  />
{/if}

<style>
  .dos {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(min(100%, 340px), 1fr));
    gap: var(--sp-4);
    margin-bottom: var(--sp-8);
  }
  .dos > .card {
    display: grid;
    align-content: start;
    gap: var(--sp-3);
  }
  .cab-sec {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--sp-2);
  }
  .section-title {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 0;
  }
  .datos {
    display: grid;
    grid-template-columns: minmax(8.5em, auto) minmax(0, 1fr);
    gap: 8px var(--sp-4);
    margin: 0;
    font-size: var(--fs-sm);
  }
  .datos dt {
    color: var(--text-3);
  }
  .datos dd {
    margin: 0;
    color: var(--text-1);
    overflow-wrap: anywhere;
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  section + section {
    margin-top: var(--sp-8);
  }
  .dos > section + section {
    margin-top: 0;
  }
  .lista-d {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .lista-d li + li {
    border-top: 1px solid var(--border);
  }
  .fila-d {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    min-height: 44px;
    padding: 10px 16px;
    color: inherit;
    text-decoration: none;
  }
  .fila-d:hover {
    background: var(--surface-2);
  }
  .fila-d:hover strong {
    text-decoration: underline;
  }
  .fila-d :global(.ic) {
    flex: none;
    color: var(--text-3);
  }
  .fila-d :global(.flecha) {
    flex: none;
    color: var(--text-3);
  }
  .fd-texto {
    display: grid;
    flex: 1;
    min-width: 0;
    gap: 1px;
  }
  .fd-texto strong {
    font-weight: 500;
  }
  .fd-texto .faint {
    font-size: var(--fs-sm);
  }
  .fd-texto .mal {
    color: var(--bad);
  }
  .ic-estado {
    display: inline-flex;
    flex: none;
    color: var(--tone);
  }
  .explica {
    margin: 0 0 var(--sp-3);
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  .vacio-linea {
    margin: 0;
    font-size: var(--fs-sm);
  }
  .mas {
    margin-top: var(--sp-2);
  }
  .peligro {
    display: grid;
    gap: var(--sp-2);
    margin-top: var(--sp-8);
  }
  .peligro p {
    margin: 0;
    font-size: var(--fs-sm);
  }
  .quitar-dest {
    color: var(--bad);
  }
  @media (max-width: 640px) {
    .datos {
      grid-template-columns: 1fr;
      gap: 2px;
    }
    .datos dd {
      margin-bottom: 6px;
    }
  }
</style>
