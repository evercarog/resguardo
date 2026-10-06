<script lang="ts">
  // Copia derivada (tarea 4b, docs/copias-en-cadena.md): otra copia de un
  // repositorio en otro destino, además de la copia externa de siempre. La hace
  // el equipo dueño (que tiene las dos contraseñas) con `cambiar_derivada`:
  // crea allí el repositorio con el troceado del original y le sube las
  // versiones (todas o las que pasen el filtro), después de cada copia o a una
  // hora, con su contraseña (la misma u otra, al kit), su retención y su
  // verificación. Los secretos solo viven en este diálogo.
  import { onDestroy } from "svelte";
  import { CalendarClock, Cloud, KeyRound, Printer, RefreshCw, TriangleAlert } from "@lucide/svelte";
  import OrdenDialog from "./OrdenDialog.svelte";
  import CampoClave from "./CampoClave.svelte";
  import EditorRetencion from "./EditorRetencion.svelte";
  import Ayuda from "./Ayuda.svelte";
  import { aleatorio } from "$lib/cripto/bytes";
  import { ADMITE, admite, cuandoEnFrase, destinosParaDerivada, errorFiltro, filtroEnFrase, filtroParaOrden, idDerivadaNueva, TEXTO_FUERA_RETENCION } from "$lib/cadenas";
  import { diasBloqueo, MAX_BLOQUEO, textoRetencionDestino } from "$lib/copiaExterna";
  import { TIPOS_NUBE, nombreTipoNube } from "$lib/espejo";
  import { admitePlazos, copiaRegla, errorRegla, horarioDeCopias, REGLA_POR_DEFECTO, reglaParaOrden } from "$lib/retencion";
  import { fechaLarga } from "$lib/formato";
  import type { Cliente, DerivadaResumen, EquipoDetalle, RepositorioResumen } from "$lib/tipos";

  interface Props {
    cliente: Cliente;
    equipo: EquipoDetalle;
    repo: RepositorioResumen;
    /** La que se cambia (sin ella, una nueva). */
    derivada?: DerivadaResumen | null;
    onclose: () => void;
  }
  let { cliente, equipo, repo, derivada = null, onclose }: Props = $props();

  // svelte-ignore state_referenced_locally
  const id = derivada?.id ?? idDerivadaNueva(repo);
  // svelte-ignore state_referenced_locally
  const destinos = destinosParaDerivada(equipo, repo);
  const conNubes = $derived(admite(equipo, ADMITE.nubeEquipo));
  const conFiltros = $derived(admite(equipo, ADMITE.filtros));
  const nubes = $derived(equipo.resumen?.nubes ?? []);
  const win = $derived(/windows/i.test(equipo.so ?? ""));

  type TipoNuevo = "local" | "rest" | "s3" | "b2";
  // svelte-ignore state_referenced_locally
  let f = $state({
    destino: derivada?.destino_id ?? destinos[0]?.id ?? (nubes[0] ? `nube:${nubes[0].nombre}` : "nuevo"),
    carpetaNube: "Resguardo",
    tipo: "local" as TipoNuevo,
    nombre: "Disco 2",
    donde: "",
    usuario: "",
    secreto: "",
    cuando: (derivada?.cuando?.hora ? "hora" : derivada?.cuando?.horario ? "horario" : "tras") as "tras" | "hora" | "horario",
    hora: derivada?.cuando?.hora ?? "23:00",
    otra: false,
    contrasena: "",
    impreso: false,
    conRetencion: false,
    retencion: copiaRegla(REGLA_POR_DEFECTO),
    conBloqueo: !!derivada?.bloqueo_dias,
    bloqueoDias: (derivada?.bloqueo_dias ?? 30) as number | string,
    etiquetas: (derivada?.filtro?.etiquetas ?? []).join(", "),
    equipos: (derivada?.filtro?.equipos ?? []).join(", "),
    carpetas: Array.isArray(derivada?.filtro?.carpetas) ? derivada.filtro.carpetas.join("\n") : "",
    desde: derivada?.filtro?.desde ?? "",
    ultimosDias: (derivada?.filtro?.ultimos_dias ?? "") as number | string,
    verificar: !!derivada?.verificacion,
  });
  /** Carpetas del filtro que el resumen solo cuenta (no las enseña): al guardar se sustituyen por las de aquí. */
  // svelte-ignore state_referenced_locally
  const carpetasOcultas = typeof derivada?.filtro?.carpetas === "number" ? derivada.filtro.carpetas : 0;

  function generar() {
    const b = aleatorio(24);
    f.contrasena = btoa(String.fromCharCode(...b)).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
    b.fill(0);
    f.impreso = false;
  }
  onDestroy(() => {
    f.contrasena = f.secreto = "";
  });

  const nubeElegida = $derived(f.destino.startsWith("nube:") ? f.destino.slice(5) : null);
  const tipoNube = $derived(nubeElegida ? nubes.find((n) => n.nombre === nubeElegida)?.tipo : equipo.resumen?.destinos?.find((d) => d.id === f.destino && d.tipo === "nube")?.nube ? nubes.find((n) => n.nombre === equipo.resumen?.destinos?.find((d) => d.id === f.destino)?.nube)?.tipo : undefined);
  /** Fuera de la oficina (la nube, otro servidor): ahí se recomienda otra contraseña. */
  const fuera = $derived(!!nubeElegida || ["s3", "b2", "nube"].includes(equipo.resumen?.destinos?.find((d) => d.id === f.destino)?.tipo ?? "") || (f.destino === "nuevo" && (f.tipo === "s3" || f.tipo === "b2")));
  const idNube = (n: string) => `nube-${n.toLowerCase().normalize("NFD").replace(/[̀-ͯ]/g, "").replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "").slice(0, 30) || "nube"}`;
  const destinoCuerpo = $derived.by(() => {
    if (nubeElegida) {
      // Una que ya está en el equipo con esa nube y esa carpeta se reutiliza.
      const ya = equipo.resumen?.destinos?.find((d) => d.tipo === "nube" && d.nube === nubeElegida && (d.donde ?? "") === f.carpetaNube.trim());
      return ya ? { id: ya.id } : { id: `${idNube(nubeElegida)}-${crypto.randomUUID().slice(0, 4)}`, nombre: nubeElegida, tipo: "nube", nube: nubeElegida, donde: f.carpetaNube.trim() };
    }
    if (f.destino === "nuevo")
      return {
        id: `derivada-${crypto.randomUUID().slice(0, 8)}`,
        nombre: f.nombre.trim(),
        tipo: f.tipo,
        donde: f.donde.trim(),
        ...(f.tipo !== "local" && f.usuario ? { usuario: f.usuario } : {}),
        ...(f.tipo !== "local" && f.secreto ? { secreto: f.secreto } : {}),
      };
    return { id: f.destino };
  });
  const filtro = $derived(conFiltros ? filtroParaOrden({ etiquetas: f.etiquetas, equipos: f.equipos, carpetas: f.carpetas, desde: f.desde, ultimos_dias: f.ultimosDias }) : filtroParaOrden({ etiquetas: f.etiquetas, equipos: f.equipos }));
  const cuerpo = $derived({
    repo: repo.id,
    id,
    destino: destinoCuerpo,
    ...(f.cuando === "tras" ? { tras_copia: true } : f.cuando === "horario" && derivada?.cuando?.horario ? { horario: derivada.cuando.horario } : { hora: f.hora }),
    ...(f.conRetencion ? { retencion: reglaParaOrden(f.retencion) } : {}),
    ...(f.otra && f.contrasena ? { contrasena_destino: f.contrasena } : {}),
    bloqueo_dias: f.conBloqueo ? (diasBloqueo(f.bloqueoDias) ?? 0) : 0,
    ...(filtro ? { filtro } : {}),
    ...(f.verificar ? { verificacion: derivada?.verificacion ?? { cada_dias: 7, porcentaje: 5 } } : {}),
  });
  const errorF = $derived(conFiltros ? errorFiltro({ desde: f.desde, ultimos_dias: f.ultimosDias }) : null);
  const valido = $derived(
    !!f.destino &&
      (!nubeElegida || !!f.carpetaNube.trim()) &&
      (f.destino !== "nuevo" || (!!f.nombre.trim() && !!f.donde.trim())) &&
      (f.cuando !== "hora" || /^([01]\d|2[0-3]):[0-5]\d$/.test(f.hora)) &&
      (!f.otra || (f.contrasena.length >= 8 && f.impreso)) &&
      (!f.conRetencion || !errorRegla(f.retencion, admitePlazos(equipo))) &&
      (!f.conBloqueo || diasBloqueo(f.bloqueoDias) !== null) &&
      !errorF,
  );
  const efecto = $derived(textoRetencionDestino(f.conRetencion, f.conBloqueo ? diasBloqueo(f.bloqueoDias) : null));
  const nombreDestino = $derived(nubeElegida ?? (f.destino === "nuevo" ? f.nombre : (destinos.find((d) => d.id === f.destino)?.nombre ?? "")));
  const resumen = $derived(
    `«${repo.nombre}» → ${nombreDestino || "otro destino"} (${[cuandoEnFrase(f.cuando === "tras" ? { tras_copia: true } : f.cuando === "horario" ? derivada?.cuando : { hora: f.hora }).toLowerCase(), filtro ? `solo las versiones ${filtroEnFrase(filtro)}` : "todas las versiones", f.otra ? "con otra contraseña" : "con la misma contraseña", f.conRetencion ? "con su propia retención" : "sin retención propia"].join(", ")}).`,
  );
</script>

<OrdenDialog
  {cliente}
  {equipo}
  tipo="cambiar_derivada"
  {cuerpo}
  titulo={derivada ? "Cambiar la copia derivada" : "Copia derivada"}
  descripcion={`${equipo.nombre} copiará «${repo.nombre}» a otro destino: allí crea un repositorio con el mismo troceado (deduplica con este), le sube las versiones y desde entonces le trae las nuevas. Si algo le pasa al destino principal, queda esta.`}
  repo={{ id: repo.id, nombre: repo.nombre }}
  {valido}
  probar={{ cuerpo: { solo_probar: true }, texto: "Probar" }}
  {onclose}
>
  {#snippet campos()}
    <div class="field">
      <label class="field-label" for="dv-destino">Copiar a</label>
      <select id="dv-destino" class="input" bind:value={f.destino} disabled={!!derivada}>
        {#each destinos as d (d.id)}<option value={d.id}>{d.nombre}{d.tipo === "nube" ? ` · nube ${d.nube ?? ""}` : d.donde ? ` · ${d.donde}` : ""}</option>{/each}
        {#if conNubes}{#each nubes as n (n.nombre)}<option value="nube:{n.nombre}">{n.nombre} · {nombreTipoNube(n.tipo)} (conectada en {equipo.nombre})</option>{/each}{/if}
        <option value="nuevo">Un destino nuevo…</option>
      </select>
      {#if derivada}<span class="field-hint">Para llevarla a otro destino, quítala y añade otra (lo de allí se queda).</span>{/if}
      {#if conNubes && !nubes.length}<span class="field-hint">Para Dropbox, Google Drive u otras nubes, conéctalas antes en este equipo («Conectar Dropbox» o «Conectar otro destino»).</span>{/if}
    </div>
    {#if nubeElegida}
      <div class="field">
        <label class="field-label" for="dv-carpeta">Carpeta dentro de la nube</label>
        <input id="dv-carpeta" class="input mono" bind:value={f.carpetaNube} placeholder="Resguardo" spellcheck="false" />
        <span class="field-hint">El repositorio irá en <code>{f.carpetaNube.trim() || "Resguardo"}/{repo.id}-{id}</code>.</span>
      </div>
    {/if}
    {#if tipoNube && TIPOS_NUBE[tipoNube] && !TIPOS_NUBE[tipoNube].inmutable}
      <div class="notice notice-warn"><TriangleAlert size={16} /><p>{TIPOS_NUBE[tipoNube].nombre} no es inmutable: alguien con acceso a la cuenta (o un ransomware en un equipo con ella abierta) podría borrar lo de allí. Mejor como un destino más, no el único fuera de la oficina.</p></div>
    {/if}
    {#if f.destino === "nuevo"}
      <div class="nuevo-destino">
        <div class="fila-campos">
          <div class="field">
            <label class="field-label" for="dv-tipo">Tipo</label>
            <select id="dv-tipo" class="input" bind:value={f.tipo}>
              <option value="local">Disco o carpeta</option>
              <option value="rest">Servidor de copias (rest-server)</option>
              <option value="s3">S3 compatible</option>
              <option value="b2">Backblaze B2</option>
            </select>
          </div>
          <div class="field">
            <label class="field-label" for="dv-nombre">Nombre</label>
            <input id="dv-nombre" class="input" bind:value={f.nombre} />
          </div>
        </div>
        <div class="field">
          <label class="field-label" for="dv-donde">{f.tipo === "local" ? "Carpeta" : f.tipo === "rest" ? "Dirección (https://servidor:puerto)" : "Bucket"}</label>
          <input id="dv-donde" class="input mono" bind:value={f.donde} placeholder={f.tipo === "local" ? (win ? "E:\\Resguardo" : "/mnt/disco2/resguardo") : ""} spellcheck="false" />
        </div>
        {#if f.tipo !== "local"}
          <div class="fila-campos">
            <div class="field">
              <label class="field-label" for="dv-usuario">{f.tipo === "rest" ? "Usuario" : "Id de la clave"}</label>
              <input id="dv-usuario" class="input mono" bind:value={f.usuario} autocomplete="off" spellcheck="false" />
            </div>
            <CampoClave requerido id="dv-secreto" etiqueta={f.tipo === "rest" ? "Contraseña" : "Clave secreta"} bind:value={f.secreto} />
          </div>
        {/if}
      </div>
    {/if}

    <div class="field">
      <span class="field-label" id="dv-l-cuando"><CalendarClock size={14} />Cuándo</span>
      <div class="segmented" role="radiogroup" aria-labelledby="dv-l-cuando">
        <button type="button" role="radio" aria-checked={f.cuando === "tras"} class:on={f.cuando === "tras"} onclick={() => (f.cuando = "tras")}>Después de cada copia</button>
        <button type="button" role="radio" aria-checked={f.cuando === "hora"} class:on={f.cuando === "hora"} onclick={() => (f.cuando = "hora")}>Cada día a una hora</button>
        {#if derivada?.cuando?.horario}<button type="button" role="radio" aria-checked={f.cuando === "horario"} class:on={f.cuando === "horario"} onclick={() => (f.cuando = "horario")}>Su horario</button>{/if}
      </div>
      {#if f.cuando === "hora"}<input class="input num corto" type="time" bind:value={f.hora} aria-label="Hora" />{/if}
      {#if f.cuando === "tras"}<span class="field-hint">En cuanto una copia de «{repo.nombre}» guarde una versión nueva. Si falla, se reintenta en la siguiente.</span>{/if}
    </div>

    <label class="switch-row"><input type="checkbox" bind:checked={f.otra} onchange={() => f.otra && !f.contrasena && generar()} /><span>{derivada ? "Otra contraseña nueva" : "Otra contraseña"}<span class="faint">{derivada ? "Si no, se queda la que tiene." : "Si no, la misma que el repositorio de origen (la de su kit)."}</span></span></label>
    {#if !f.otra && fuera && !derivada}<p class="faint nota">Recomendado: otra contraseña para un destino fuera de la oficina (así una contraseña no abre los dos). No es obligatorio.</p>{/if}
    {#if f.otra}
      <article class="kit" id="kit-imprimible">
        <h3>Kit de recuperación · copia derivada</h3>
        <dl>
          <dt>Cliente</dt><dd>{cliente.nombre}</dd>
          <dt>Equipo</dt><dd>{equipo.nombre}</dd>
          <dt>Copia de</dt><dd>{repo.nombre} · <span class="pastilla mono selectable">{repo.id}-{id}</span></dd>
          <dt>Destino</dt><dd>{nombreDestino}</dd>
          <dt>Contraseña</dt><dd><code class="selectable pw">{f.contrasena}</code></dd>
          <dt>Creado</dt><dd>{fechaLarga(new Date().toISOString())}</dd>
        </dl>
      </article>
      <div class="acciones">
        <button type="button" class="btn btn-sm" onclick={generar}><RefreshCw size={14} />Otra</button>
        <button type="button" class="btn btn-sm" onclick={() => window.print()}><Printer size={14} />Imprimir o guardar en PDF</button>
      </div>
      <label class="switch-row"><input type="checkbox" bind:checked={f.impreso} /><span>He guardado el kit en un sitio seguro<span class="faint">Sin esta contraseña nadie podrá leer esta copia.</span></span></label>
    {/if}

    <label class="switch-row"><input type="checkbox" bind:checked={f.conRetencion} /><span>Retención propia<span class="faint">Si no, allí se guardan todas las versiones que suba.</span></span></label>
    {#if f.conRetencion}
      <EditorRetencion id="dvr" bind:regla={f.retencion} admite={admitePlazos(equipo)} {...horarioDeCopias(equipo.resumen?.copias, repo.id)} />
    {/if}
    <label class="switch-row"><input type="checkbox" bind:checked={f.conBloqueo} /><span>El destino tiene bloqueo de objetos (Object Lock)<span class="faint">Lo subido no se puede borrar durante unos días: la copia queda fuera del alcance de un ransomware.</span></span></label>
    {#if f.conBloqueo}
      <div class="field">
        <label class="field-label" for="dv-bloqueo">Días de bloqueo</label>
        <input id="dv-bloqueo" class="input num corto" type="number" min="1" max={MAX_BLOQUEO} bind:value={f.bloqueoDias} />
      </div>
    {/if}
    {#if efecto}<p class="faint nota">{efecto}</p>{/if}

    <details class="avanzado" open={!!filtro}>
      <summary>Qué versiones subir</summary>
      <p class="faint nota">Todas, o solo las que pasen el filtro (se cumple todo lo que escribas).</p>
      <div class="fila-campos">
        <div class="field">
          <label class="field-label" for="dv-etiquetas">Con la etiqueta</label>
          <input id="dv-etiquetas" class="input" bind:value={f.etiquetas} placeholder="diaria, semanal" />
        </div>
        <div class="field">
          <label class="field-label" for="dv-equipos">Del equipo</label>
          <input id="dv-equipos" class="input" bind:value={f.equipos} placeholder={equipo.nombre} />
        </div>
      </div>
      {#if conFiltros}
        <div class="fila-campos">
          <div class="field">
            <label class="field-label" for="dv-dias">De los últimos (días)</label>
            <input id="dv-dias" class="input num corto" type="number" min="1" max="3650" bind:value={f.ultimosDias} />
          </div>
          <div class="field">
            <label class="field-label" for="dv-desde">Desde el</label>
            <input id="dv-desde" class="input" type="date" bind:value={f.desde} />
          </div>
        </div>
        <div class="field">
          <label class="field-label" for="dv-carpetas">De estas carpetas <span class="faint">(una por línea)</span></label>
          <textarea id="dv-carpetas" class="input mono" rows="2" spellcheck="false" bind:value={f.carpetas}></textarea>
          {#if carpetasOcultas}<span class="field-hint">Tenía {carpetasOcultas === 1 ? "1 carpeta" : `${carpetasOcultas} carpetas`} en el filtro: al guardar se cambian por las que escribas aquí.</span>{/if}
        </div>
        {#if errorF}<p class="error-campo">{errorF}</p>{/if}
      {:else}
        <p class="faint nota">Actualiza el agente para filtrar también por carpetas y fechas.</p>
      {/if}
    </details>

    <label class="switch-row"><input type="checkbox" bind:checked={f.verificar} /><span>Verificar la copia derivada<span class="faint">Cada semana, el 5 % de sus datos (rotando): en 20 semanas se ha leído todo.</span></span></label>

    <div class="notice notice-info resumen"><Cloud size={16} /><p>{resumen} <Ayuda id="copia-derivada" /></p></div>
    {#if !f.conRetencion && !f.conBloqueo}{:else}<p class="faint nota">{TEXTO_FUERA_RETENCION.split(".")[0]}: una copia derivada con su propia retención cuenta como fuera del alcance de la retención del original.</p>{/if}
    <p class="faint nota"><KeyRound size={13} />La contraseña del repositorio de origen la usa solo {equipo.nombre}: el almacén nunca la ve.</p>
  {/snippet}
</OrdenDialog>

<style>
  .kit {
    padding: var(--sp-4);
    border: 1px dashed var(--border-strong);
    border-radius: var(--radius);
  }
  .kit h3 {
    margin: 0 0 var(--sp-2);
    font-size: var(--fs-md, 15px);
  }
  .kit dl {
    display: grid;
    grid-template-columns: 90px minmax(0, 1fr);
    gap: 4px 10px;
    margin: 0;
    font-size: var(--fs-sm);
  }
  .kit dt {
    color: var(--text-3);
  }
  .kit dd {
    margin: 0;
    min-width: 0;
    overflow-wrap: anywhere;
  }
  .pw {
    font-size: 13px;
    word-break: break-all;
  }
  .acciones {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .fila-campos {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: var(--sp-3);
  }
  .nuevo-destino {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
    padding: var(--sp-3);
    border: 1px solid var(--border);
    border-radius: var(--radius);
  }
  .nota {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 0;
    font-size: var(--fs-xs);
  }
  .avanzado summary {
    cursor: pointer;
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  .avanzado[open] {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
  }
  .segmented {
    flex-wrap: wrap;
  }
  .corto {
    max-width: 140px;
  }
  .resumen p {
    margin: 0;
  }
  @media (max-width: 560px) {
    .fila-campos {
      grid-template-columns: 1fr;
    }
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
