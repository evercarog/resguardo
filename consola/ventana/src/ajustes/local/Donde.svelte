<script lang="ts">
  // Dónde se guardan las copias (modo local): repositorios en un disco o
  // carpeta del equipo, un servidor de copias (rest-server), S3, B2 o SFTP;
  // su retención (con los preajustes de la consola, p. ej. «Programas contables») y su
  // copia externa. Al crear uno, su contraseña y el kit de recuperación.
  // Como en la consola: «Usar uno que ya existe» (p. ej. el de la app de
  // escritorio, con su historial) y «Traer historial» de otro a uno de aquí.
  import { Database, FolderOpen, HardDrive, History, Plus, Printer, Server, ShieldCheck } from "@lucide/svelte";
  import EditorRetencion from "$lib/componentes/EditorRetencion.svelte";
  import { copiaRegla, errorRegla, PRESETS, resumenRegla } from "$lib/retencion";
  import type { Regla } from "$lib/tipos";
  import { esBloqueo, servicio } from "../../puente.svelte";
  import { contrasenaNueva, idDe, TIPOS_DESTINO, type DestinoLocal, type PropsParte, type RepoLocal } from "./comun";
  import ElegirCarpeta from "./ElegirCarpeta.svelte";
  import Kit from "./Kit.svelte";
  import TraerHistorial from "./TraerHistorial.svelte";
  import UsarExistente from "./UsarExistente.svelte";

  let { estado, recargar, alBloquear }: PropsParte = $props();
  let error = $state("");
  let hecho = $state("");
  let ocupado = $state(false);

  async function pedir<T>(que: string, cuerpo: Record<string, unknown>): Promise<T | null> {
    ocupado = true;
    error = hecho = "";
    try {
      const r = await servicio<T>(que, cuerpo);
      await recargar();
      return r;
    } catch (e) {
      if (esBloqueo(e)) alBloquear();
      error = (e as Error).message;
      return null;
    } finally {
      ocupado = false;
    }
  }

  // ---- Repositorio nuevo ----
  let nuevo = $state<null | { nombre: string; destino: string; tipo: DestinoLocal["tipo"]; donde: string; usuario: string; secreto: string; ca: string; nombreDestino: string; contrasena: string; guardada: boolean }>(null);
  let creado = $state<null | { nombre: string; contrasena: string; id: string }>(null);
  let elegirCarpeta = $state<null | "nuevo" | "externa">(null);
  function empezarNuevo() {
    nuevo = { nombre: "Copias", destino: "", tipo: "local", donde: "", usuario: "", secreto: "", ca: "", nombreDestino: "", contrasena: contrasenaNueva(), guardada: false };
    creado = null;
  }
  const problemaNuevo = $derived.by(() => {
    if (!nuevo) return null;
    if (!nuevo.nombre.trim()) return "Ponle un nombre.";
    if (!nuevo.destino && !nuevo.donde.trim()) return nuevo.tipo === "local" ? "Elige la carpeta o el disco." : "Escribe dónde.";
    if (!nuevo.destino && nuevo.tipo === "rest" && !/^https?:\/\//.test(nuevo.donde.trim())) return "La dirección del servidor empieza por https://.";
    if (!nuevo.guardada) return "Marca que has guardado la contraseña (o imprime el kit).";
    return null;
  });
  async function crear() {
    if (!nuevo || problemaNuevo) return;
    const id = idDe(nuevo.nombre);
    const destino = nuevo.destino
      ? { id: nuevo.destino }
      : {
          id: idDe(nuevo.nombreDestino || nuevo.tipo),
          nombre: nuevo.nombreDestino.trim() || (nuevo.tipo === "local" ? "Disco o carpeta del equipo" : TIPOS_DESTINO[nuevo.tipo]),
          tipo: nuevo.tipo,
          donde: nuevo.donde.trim(),
          ...(nuevo.usuario ? { usuario: nuevo.usuario } : {}),
          ...(nuevo.secreto ? { secreto: nuevo.secreto } : {}),
          ...(nuevo.ca.includes("BEGIN CERTIFICATE") ? { ca_pem: nuevo.ca } : {}),
        };
    const r = await pedir<{ mensaje: string }>("crear_repositorio", {
      repositorio: { id, nombre: nuevo.nombre.trim(), contrasena: nuevo.contrasena, destino, retencion: copiaRegla(PRESETS[2].regla) },
    });
    if (r) {
      hecho = r.mensaje;
      creado = { nombre: nuevo.nombre.trim(), contrasena: nuevo.contrasena, id };
      nuevo = null;
    }
  }

  // ---- Uno que ya existe y traer historial (como en la consola) ----
  let existente = $state(false);
  let historialDe = $state<string | null>(null);

  // ---- Retención ----
  let retencion = $state<null | { repo: RepoLocal; regla: Regla }>(null);
  async function guardarRetencion(aplicar: boolean) {
    if (!retencion) return;
    const r = await pedir<{ mensaje: string }>("retencion", { repo: retencion.repo.id, retencion: retencion.regla, aplicar });
    if (r) {
      hecho = aplicar ? `${r.mensaje} Se está aplicando ahora (puede tardar).` : r.mensaje;
      retencion = null;
    }
  }

  // ---- Copia externa ----
  let externa = $state<null | { repo: RepoLocal; destino: string; tipo: DestinoLocal["tipo"]; donde: string; usuario: string; secreto: string; hora: string }>(null);
  async function guardarExterna(quitar = false) {
    if (!externa) return;
    const destino = externa.destino
      ? { id: externa.destino }
      : { id: idDe(`externa-${externa.tipo}`), nombre: TIPOS_DESTINO[externa.tipo], tipo: externa.tipo, donde: externa.donde.trim(), ...(externa.usuario ? { usuario: externa.usuario } : {}), ...(externa.secreto ? { secreto: externa.secreto } : {}) };
    const r = await pedir<{ mensaje: string }>("copia_externa", { repo: externa.repo.id, externa: quitar ? {} : { destino, hora: externa.hora } });
    if (r) {
      hecho = r.mensaje;
      externa = null;
    }
  }
  const destinoDe = (id: string) => estado.destinos.find((d) => d.id === id);
  const icono = (t: string) => (t === "local" ? HardDrive : t === "rest" ? Server : Database);
</script>

{#if elegirCarpeta}
  <ElegirCarpeta
    titulo="Carpeta o disco donde guardar"
    crear
    alCerrar={() => (elegirCarpeta = null)}
    alElegir={(rs) => {
      if (elegirCarpeta === "nuevo" && nuevo) nuevo.donde = rs[0];
      if (elegirCarpeta === "externa" && externa) externa.donde = rs[0];
      elegirCarpeta = null;
    }}
  />
{/if}

<div class="v-pila">
  {#if creado}
    <Kit nombreEquipo={estado.nombre_equipo} recien={creado} />
  {/if}

  {#each estado.repositorios as r (r.id)}
    {@const d = destinoDe(r.destino)}
    {@const Icono = icono(d?.tipo ?? "")}
    <section class="v-tarjeta v-pila">
      <div class="v-fila">
        <Icono size={18} aria-hidden="true" />
        <div class="v-cortar">
          <h3 class="v-titulo v-cortar">{r.nombre}</h3>
          <p class="v-mini v-cortar">{d ? `${TIPOS_DESTINO[d.tipo] ?? d.tipo} · ${d.donde}` : r.destino}</p>
        </div>
      </div>
      <p class="v-mini">Se guardan: {r.retencion ? resumenRegla(r.retencion) : "todas las versiones (sin retención)."}</p>
      <p class="v-mini">Copia externa: {r.externa ? `a «${destinoDe(r.externa.destino)?.nombre ?? r.externa.destino}» cada día a las ${r.externa.hora}` : "no"}.</p>
      <div class="v-fila nuevos">
        <button class="btn btn-sm" onclick={() => (retencion ={ repo: r, regla: copiaRegla(r.retencion ?? PRESETS[2].regla) })}><ShieldCheck size={14} aria-hidden="true" />Qué versiones se guardan</button>
        <button class="btn btn-sm" onclick={() => (externa = { repo: r, destino: r.externa?.destino ?? "", tipo: "local", donde: "", usuario: "", secreto: "", hora: r.externa?.hora ?? "02:00" })}>Copia externa</button>
        <button class="btn btn-sm" onclick={() => (historialDe = historialDe === r.id ? null : r.id)}><History size={14} aria-hidden="true" />Traer historial</button>
      </div>

      {#if historialDe === r.id}
        <TraerHistorial repo={r} {estado} {alBloquear} alCerrar={() => (historialDe = null)} />
      {/if}

      {#if retencion?.repo.id === r.id}
        <div class="v-pila sub">
          <div class="v-fila presets">
            {#each PRESETS as p (p.id)}<button class="btn btn-sm" onclick={() => retencion && (retencion.regla = copiaRegla(p.regla))}>{p.texto}</button>{/each}
          </div>
          <EditorRetencion id={`ret-${r.id}`} bind:regla={retencion.regla} admite={true} />
          {#if errorRegla(retencion.regla)}<p class="v-mini aviso">{errorRegla(retencion.regla)}</p>{/if}
          <div class="v-fila fin">
            <button class="btn btn-ghost btn-sm" onclick={() => (retencion = null)}>Cancelar</button>
            <button class="btn btn-sm" disabled={ocupado || !!errorRegla(retencion.regla)} onclick={() => guardarRetencion(false)}>Guardar</button>
            <button class="btn btn-primary btn-sm" disabled={ocupado || !!errorRegla(retencion.regla)} onclick={() => guardarRetencion(true)}>Guardar y aplicar ya</button>
          </div>
        </div>
      {/if}

      {#if externa?.repo.id === r.id}
        <div class="v-pila sub">
          <p class="v-mini">Cada día, las versiones nuevas se copian también a otro sitio (otro disco, otro servidor o la nube): si se pierde este, quedan allí.</p>
          <label class="field">
            <span class="field-label">A dónde</span>
            <select class="input" bind:value={externa.destino}>
              <option value="">Un destino nuevo…</option>
              {#each estado.destinos.filter((x) => x.id !== r.destino) as x (x.id)}<option value={x.id}>{x.nombre}</option>{/each}
            </select>
          </label>
          {#if !externa.destino}
            <select class="input" bind:value={externa.tipo} aria-label="Tipo de destino">
              {#each Object.entries(TIPOS_DESTINO) as [t, n] (t)}<option value={t}>{n}</option>{/each}
            </select>
            <div class="v-fila">
              <input class="input mono" bind:value={externa.donde} placeholder={externa.tipo === "local" ? "F:\\CopiaExterna" : externa.tipo === "rest" ? "https://servidor:8000" : "bucket"} aria-label="Dónde" />
              {#if externa.tipo === "local"}<button class="btn btn-sm" onclick={() => (elegirCarpeta = "externa")}><FolderOpen size={14} />Elegir</button>{/if}
            </div>
            {#if externa.tipo !== "local"}
              <input class="input" bind:value={externa.usuario} placeholder="Usuario o ID de la clave" aria-label="Usuario" autocomplete="off" />
              <input class="input" type="password" bind:value={externa.secreto} placeholder="Contraseña o clave secreta" aria-label="Contraseña" autocomplete="off" />
            {/if}
          {/if}
          <label class="v-fila"><span class="field-label">A las</span><input class="input hora" type="time" bind:value={externa.hora} /></label>
          <div class="v-fila fin">
            {#if r.externa}<button class="btn btn-ghost btn-sm" disabled={ocupado} onclick={() => guardarExterna(true)}>Quitar</button>{/if}
            <button class="btn btn-ghost btn-sm" onclick={() => (externa = null)}>Cancelar</button>
            <button class="btn btn-primary btn-sm" disabled={ocupado || (!externa.destino && !externa.donde.trim())} onclick={() => guardarExterna()}>Guardar</button>
          </div>
        </div>
      {/if}
    </section>
  {/each}

  {#if nuevo}
    <form class="v-tarjeta v-pila" onsubmit={(e) => (e.preventDefault(), void crear())}>
      <h3 class="v-titulo">Dónde guardar las copias</h3>
      <label class="field"><span class="field-label">Nombre</span><input class="input" bind:value={nuevo.nombre} maxlength="80" /></label>
      {#if estado.destinos.length}
        <label class="field">
          <span class="field-label">En</span>
          <select class="input" bind:value={nuevo.destino}>
            <option value="">Un sitio nuevo…</option>
            {#each estado.destinos as d (d.id)}<option value={d.id}>{d.nombre}</option>{/each}
          </select>
        </label>
      {/if}
      {#if !nuevo.destino}
        <div class="field">
          <span class="field-label">Tipo</span>
          <select class="input" bind:value={nuevo.tipo}>
            {#each Object.entries(TIPOS_DESTINO) as [t, n] (t)}<option value={t}>{n}</option>{/each}
          </select>
        </div>
        {#if nuevo.tipo === "local"}
          <div class="v-fila">
            <input class="input mono" bind:value={nuevo.donde} placeholder="E:\Copias" aria-label="Carpeta" />
            <button type="button" class="btn btn-sm" onclick={() => (elegirCarpeta = "nuevo")}><FolderOpen size={14} />Elegir</button>
          </div>
          <p class="v-mini">Un disco USB o externo protege de un fallo del disco del equipo; para un robo o un incendio, añade después una copia externa.</p>
        {:else if nuevo.tipo === "rest"}
          <input class="input mono" bind:value={nuevo.donde} placeholder="https://servidor:8000" aria-label="Dirección del servidor" />
          <input class="input" bind:value={nuevo.usuario} placeholder="Usuario" aria-label="Usuario" autocomplete="off" />
          <input class="input" type="password" bind:value={nuevo.secreto} placeholder="Contraseña del servidor" aria-label="Contraseña del servidor" autocomplete="off" />
          <textarea class="input mono" rows="2" bind:value={nuevo.ca} placeholder="Certificado propio (opcional): -----BEGIN CERTIFICATE-----…" aria-label="Certificado"></textarea>
        {:else if nuevo.tipo === "sftp"}
          <input class="input mono" bind:value={nuevo.donde} placeholder="usuario@servidor:/copias" aria-label="Dirección SFTP" />
          <p class="v-mini">Usa la llave SSH del equipo (la del servicio).</p>
        {:else}
          <input class="input mono" bind:value={nuevo.donde} placeholder={nuevo.tipo === "s3" ? "s3.eu-west-1.amazonaws.com/bucket" : "bucket"} aria-label="Bucket" />
          <input class="input" bind:value={nuevo.usuario} placeholder="ID de la clave" aria-label="ID de la clave" autocomplete="off" />
          <input class="input" type="password" bind:value={nuevo.secreto} placeholder="Clave secreta" aria-label="Clave secreta" autocomplete="off" />
        {/if}
        <label class="field"><span class="field-hint">Nombre del sitio (opcional)</span><input class="input" bind:value={nuevo.nombreDestino} maxlength="80" placeholder={TIPOS_DESTINO[nuevo.tipo]} /></label>
      {/if}
      <div class="field">
        <span class="field-label">Contraseña de las copias</span>
        <code class="selectable mono pw">{nuevo.contrasena}</code>
        <span class="field-hint">Cifra las copias. Sin ella no se pueden abrir: guárdala fuera de este equipo (imprime el kit al terminar).</span>
      </div>
      <label class="switch-row"><input type="checkbox" class="switch" bind:checked={nuevo.guardada} /><span>He guardado la contraseña</span></label>
      {#if problemaNuevo}<p class="v-mini aviso">{problemaNuevo}</p>{/if}
      <div class="v-fila fin">
        <button type="button" class="btn btn-ghost" onclick={() => (nuevo = null)}>Cancelar</button>
        <button class="btn btn-primary" disabled={ocupado || !!problemaNuevo}>{ocupado ? "Creando (puede tardar)…" : "Crear"}</button>
      </div>
    </form>
  {:else if existente}
    <UsarExistente
      {estado}
      {recargar}
      {alBloquear}
      alCerrar={() => (existente = false)}
      alHecho={(m) => {
        existente = false;
        hecho = m;
      }}
    />
  {:else}
    <div class="v-fila nuevos">
      <button class="btn btn-primary" onclick={empezarNuevo}><Plus size={15} aria-hidden="true" />{estado.repositorios.length ? "Otro sitio para copias" : "Elegir dónde guardar las copias"}</button>
      <button class="btn" onclick={() => ((existente = true), (hecho = error = ""))}><Database size={15} aria-hidden="true" />Usar uno que ya existe</button>
    </div>
  {/if}
  {#if creado}<button class="btn btn-sm" onclick={() => window.print()}><Printer size={14} aria-hidden="true" />Imprimir el kit</button>{/if}
  {#if hecho}<p class="v-ok" role="status">{hecho}</p>{/if}
  {#if error}<p class="v-error" role="alert">{error}</p>{/if}
</div>

<style>
  .sub {
    padding-top: var(--sp-3);
    border-top: 1px solid var(--border);
  }
  .presets,
  .nuevos {
    flex-wrap: wrap;
  }
  .fin {
    justify-content: flex-end;
    flex-wrap: wrap;
  }
  .hora {
    width: 120px;
  }
  .pw {
    padding: 8px 10px;
    border-radius: var(--radius-sm);
    background: var(--surface-2);
    font-size: var(--fs-body);
    letter-spacing: 0.04em;
  }
  .aviso {
    color: var(--warn);
    margin: 0;
  }
</style>
