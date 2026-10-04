<script lang="ts">
  import { onMount } from "svelte";
  import { fade } from "svelte/transition";
  import { dur } from "$lib/motion";
  import { ArrowLeft, Check, CircleAlert, KeyRound, LoaderCircle, Printer, ShieldCheck, TriangleAlert } from "@lucide/svelte";
  import * as api from "$lib/api";
  import type { KitEntry, Repo } from "$lib/api";
  import { formatDate } from "$lib/format";
  import { repoKind } from "$lib/repoKind";
  import { kitState } from "$lib/kit.svelte";
  import { toast } from "$lib/toast.svelte";
  import { withPassword } from "$lib/passwordPrompt.svelte";
  import Logo from "./Logo.svelte";
  import { anyModalOpen } from "./Modal.svelte";

  // Kit de recuperación: una hoja para imprimir (o guardar como PDF) con lo
  // necesario para abrir las copias si se pierde este equipo. Sin secretos,
  // salvo la contraseña de un destino si el usuario la escribe a propósito.
  interface Props {
    repos: Repo[];
    /** Destinos incluidos (null: todos). */
    ids: string[] | null;
    onclose: () => void;
    onchange: (repo: Repo) => void;
  }
  let { repos, ids, onclose, onchange }: Props = $props();

  let entries = $state<KitEntry[] | null>(null);
  let error = $state("");
  let device = $state("");
  let appVersion = $state("");
  let printed = $state(false);
  let saving = $state(false);
  const now = new Date();

  /** Contraseñas escritas (y comprobadas) para imprimir, por destino. */
  let includePw = $state<Record<string, boolean>>({});
  let pwInput = $state<Record<string, string>>({});
  let pwOk = $state<Record<string, string>>({});
  let pwError = $state<Record<string, string>>({});
  let checking = $state<Record<string, boolean>>({});

  onMount(async () => {
    api.webInfo().then((w) => (device = w.default_name), () => (device = ""));
    import("@tauri-apps/api/app").then((m) => m.getVersion()).then((v) => (appVersion = v), () => (appVersion = ""));
    try {
      entries = await api.recoveryInfo(ids);
    } catch (e) {
      error = String(e);
    }
  });

  async function checkPw(e: KitEntry) {
    const pw = pwInput[e.id] ?? "";
    if (!pw) return;
    checking[e.id] = true;
    pwError[e.id] = "";
    try {
      await api.checkPassword(e.id, pw);
      pwOk[e.id] = pw;
    } catch (err) {
      pwError[e.id] = String(err);
    } finally {
      checking[e.id] = false;
    }
  }

  function togglePw(e: KitEntry, on: boolean) {
    includePw[e.id] = on;
    if (!on) {
      delete pwOk[e.id];
      pwInput[e.id] = "";
      pwError[e.id] = "";
    }
  }

  function print() {
    window.print();
    printed = true;
  }

  async function confirmSaved() {
    if (!entries) return;
    saving = true;
    try {
      // Anotarlo cambia la configuración: con la contraseña del destino (la
      // ya comprobada para imprimir, si se escribió; si no, se pide).
      for (const e of entries) {
        const configId = e.config_id ?? null;
        if (pwOk[e.id]) {
          onchange(await api.kitConfirm(e.id, configId, pwOk[e.id]));
          continue;
        }
        const done = await withPassword({
          title: "Confirmar el kit de recuperación",
          message: `Anota que guardaste el kit de «${e.name}» en un lugar seguro.`,
          repoName: e.name,
          confirmLabel: "Confirmar",
          action: async (password) => onchange(await api.kitConfirm(e.id, configId, password)),
        });
        if (done === null) return;
      }
      toast(entries.length === 1 ? `Kit de recuperación de «${entries[0].name}» guardado` : "Kit de recuperación guardado");
      onclose();
    } catch (e) {
      toast(String(e), "error");
    } finally {
      saving = false;
    }
  }

  const repoOf = (id: string) => repos.find((r) => r.id === id);
  const dateLong = new Intl.DateTimeFormat("es", { day: "numeric", month: "long", year: "numeric" }).format(now);

  const PROVIDER: Record<string, string> = { s3: "S3 (compatible)", b2: "Backblaze B2", azure: "Azure", gs: "Google Cloud Storage" };

  /** Ubicación para los comandos: en REST con usuario, con marcadores en lugar de las credenciales. */
  function commandLocation(e: KitEntry) {
    if (e.kind === "rest" && e.rest_auth) return e.location.replace(/^(rest:https?:\/\/)/, "$1USUARIO:CONTRASEÑA@");
    return /\s/.test(e.location) ? `"${e.location}"` : e.location;
  }
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && !anyModalOpen() && onclose()} />

<div class="kit-overlay" transition:fade={{ duration: dur(150) }} role="dialog" aria-modal="true" aria-labelledby="kit-title">
  <header class="toolbar no-print">
    <button class="btn btn-ghost btn-sm" onclick={onclose}><ArrowLeft size={14} /> Volver</button>
    <div class="tb-title">
      <strong id="kit-title">Kit de recuperación</strong>
      <span class="faint">Imprímelo o guárdalo como PDF y guárdalo fuera de este equipo.</span>
    </div>
    <button class="btn btn-primary" onclick={print} disabled={!entries}><Printer size={15} /> Imprimir o guardar como PDF</button>
  </header>

  <div class="layout">
    <aside class="options no-print">
      <h3>Antes de imprimir</h3>
      {#if entries}
        {#each entries as e (e.id)}
          {@const repo = repoOf(e.id)}
          {@const state = repo ? kitState(repo) : "missing"}
          <div class="opt">
            <div class="opt-head">
              <strong>{e.name}</strong>
              {#if state === "ok"}
                <span class="pill ok"><Check size={11} strokeWidth={3} /> Kit guardado {formatDate(repo!.kit!.saved_at)}</span>
              {:else if state === "stale"}
                <span class="pill warn">La ubicación cambió desde el último kit</span>
              {:else}
                <span class="pill">Sin guardar</span>
              {/if}
            </div>
            <label class="check">
              <input type="checkbox" checked={!!includePw[e.id]} onchange={(ev) => togglePw(e, ev.currentTarget.checked)} />
              Incluir la contraseña impresa
            </label>
            {#if includePw[e.id]}
              {#if pwOk[e.id]}
                <p class="ok-line"><Check size={13} /> Contraseña comprobada: se imprimirá.</p>
              {:else}
                <form class="pw" onsubmit={(ev) => (ev.preventDefault(), checkPw(e))}>
                  <KeyRound size={14} />
                  <input class="input" type="password" autocomplete="off" placeholder="Contraseña de «{e.name}»" bind:value={pwInput[e.id]} />
                  <button class="btn btn-sm" disabled={checking[e.id] || !pwInput[e.id]}>
                    {#if checking[e.id]}<span class="spin" style="display:grid"><LoaderCircle size={13} /></span>{:else}Comprobar{/if}
                  </button>
                </form>
                {#if pwError[e.id]}<p class="err">{pwError[e.id]}</p>{/if}
              {/if}
              <p class="warn-line">
                <TriangleAlert size={13} />
                <span>Guárdalo como guardarías las llaves de la oficina: con la contraseña impresa, quien tenga este papel puede abrir las copias.</span>
              </p>
            {/if}
          </div>
        {/each}
        {#if printed}
          <div class="saved" transition:fade={{ duration: dur(150) }}>
            <p>¿Ya lo imprimiste o lo guardaste como PDF fuera de este equipo?</p>
            <button class="btn btn-primary" onclick={confirmSaved} disabled={saving}><ShieldCheck size={15} /> Ya lo guardé en un lugar seguro</button>
          </div>
        {:else}
          <p class="faint small">Después de imprimirlo podrás marcarlo como guardado.</p>
        {/if}
      {/if}
    </aside>

    <article class="sheet">
      {#if error}
        <div class="notice notice-danger" role="alert"><CircleAlert size={16} /><p>{error}</p></div>
      {:else if !entries}
        <p class="loading"><span class="spin" style="display:grid"><LoaderCircle size={16} /></span> Leyendo los repositorios…</p>
      {:else}
        <header class="sheet-head">
          <Logo size={34} />
          <div>
            <h1>Kit de recuperación</h1>
            <p>{device ? `Equipo ${device}` : "Este equipo"} · {dateLong}</p>
          </div>
        </header>
        <p class="lead">
          Con esta hoja puedes abrir las copias de este equipo aunque se pierda o se estropee: las contraseñas solo están guardadas en él.
          Guárdala fuera de la oficina, en un lugar seguro.
        </p>

        {#each entries as e, i (e.id)}
          {@const kind = repoKind(e.location)}
          <section class="dest">
            <h2><span class="num">{i + 1}</span> {e.name} <span class="kind">{kind.label}</span></h2>
            <dl>
              <dt>Ubicación</dt>
              <dd class="mono">{e.location}</dd>
              {#if e.kind === "rest"}
                <dt>En el servidor</dt>
                <dd>Carpeta de datos de rest-server{e.rest_path ? ` + /${e.rest_path}` : ""}</dd>
                {#if e.rest_auth}
                  <dt>Usuario del servidor</dt>
                  <dd>No se imprime: lo tiene quien administra el servidor (o mira su configuración).</dd>
                {/if}
              {/if}
              {#if e.cloud}
                <dt>Proveedor</dt>
                <dd>{PROVIDER[e.cloud.provider] ?? e.cloud.provider}{e.cloud.region ? ` · región ${e.cloud.region}` : ""}</dd>
                {#if e.cloud.endpoint}<dt>Servidor</dt><dd class="mono">{e.cloud.endpoint}</dd>{/if}
                {#if e.cloud.bucket}<dt>Bucket</dt><dd class="mono">{e.cloud.bucket}{e.cloud.prefix ? ` · carpeta ${e.cloud.prefix}` : ""}</dd>{/if}
                <dt>ID de la clave</dt>
                <dd>
                  <span class="mono">{e.cloud.key_id ?? "—"}</span>
                  <span class="note">La clave secreta no se imprime: crea una clave nueva en la consola del proveedor si no la tienes.</span>
                </dd>
              {/if}
              <dt>ID del repositorio</dt>
              <dd class="mono small-id">{e.config_id ?? `no se pudo leer (${e.error ?? "sin acceso"})`}</dd>
              <dt>Contraseña</dt>
              <dd class="pw-line">
                {#if pwOk[e.id]}<span class="mono printed-pw">{pwOk[e.id]}</span>{:else}<span class="blank"></span>{/if}
              </dd>
            </dl>
            <div class="cmds">
              <p>Para ver sus versiones y recuperar la más reciente:</p>
              <pre>{#if e.cloud?.provider === "b2"}B2_ACCOUNT_ID=…  B2_ACCOUNT_KEY=…   (las claves de la nube)
{:else if e.cloud}AWS_ACCESS_KEY_ID=…  AWS_SECRET_ACCESS_KEY=…   (las claves de la nube)
{/if}restic -r {commandLocation(e)} snapshots
restic -r {commandLocation(e)} restore latest --target C:\Recuperado</pre>
            </div>
          </section>
        {/each}

        <section class="howto">
          <h2>Cómo recuperar sin Resguardo</h2>
          <ol>
            <li>Descarga restic para tu sistema desde <span class="mono">github.com/restic/restic/releases</span> (es un solo programa).</li>
            <li>Abre una terminal y ejecuta los comandos de la ficha del repositorio. restic te pedirá su contraseña.</li>
            <li><span class="mono">latest</span> es la versión más reciente; para otra, usa su ID de la lista de <span class="mono">snapshots</span>.</li>
            <li>
              En un servidor REST con usuario: <span class="mono">rest:http://usuario:CONTRASEÑA@host:puerto/ruta</span> (cambia
              <span class="mono">usuario</span> y <span class="mono">CONTRASEÑA</span> por los del servidor).
            </li>
            <li>
              En la nube, antes de los comandos, define las claves: en S3 y compatibles <span class="mono">AWS_ACCESS_KEY_ID</span> y
              <span class="mono">AWS_SECRET_ACCESS_KEY</span>; en Backblaze B2 nativo, <span class="mono">B2_ACCOUNT_ID</span> y
              <span class="mono">B2_ACCOUNT_KEY</span>. En Windows (PowerShell): <span class="mono">$env:AWS_ACCESS_KEY_ID="…"</span>; en Linux o macOS:
              <span class="mono">export AWS_ACCESS_KEY_ID=…</span>
            </li>
          </ol>
          <h2>Cómo recuperar con Resguardo</h2>
          <p>
            Instala Resguardo en otro equipo, pulsa <strong>Añadir repositorio</strong> → <strong>Conectar uno que ya tengo</strong> y escribe la ubicación y
            la contraseña de la ficha. Después, <strong>Restaurar archivos…</strong>
          </p>
          <h2>La regla 3-2-1</h2>
          <p>
            Tres copias de tus datos, en dos tipos de soporte distintos y una fuera de la oficina. Este kit es la llave de todas: sin la contraseña,
            nadie, tampoco Resguardo, puede abrir las copias.
          </p>
        </section>
        <footer class="sheet-foot">Generado por Resguardo{appVersion ? ` ${appVersion}` : ""} el {formatDate(now.toISOString())}.</footer>
      {/if}
    </article>
  </div>
</div>

<style>
  .kit-overlay {
    position: fixed;
    inset: 0;
    z-index: 30;
    overflow: auto;
    background: var(--bg);
  }
  .toolbar {
    position: sticky;
    top: 0;
    z-index: 1;
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 12px 24px;
    background: color-mix(in srgb, var(--bg) 88%, transparent);
    backdrop-filter: blur(8px);
    border-bottom: 1px solid var(--border);
  }
  .tb-title {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-width: 0;
    font-size: 13px;
  }
  .tb-title strong {
    font-size: 15px;
  }
  .layout {
    display: grid;
    grid-template-columns: minmax(240px, 300px) minmax(0, 820px);
    justify-content: center;
    gap: 24px;
    padding: 24px;
    align-items: start;
  }
  @media (max-width: 1000px) {
    .layout {
      grid-template-columns: minmax(0, 1fr);
    }
  }
  .options {
    position: sticky;
    top: 80px;
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 16px;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
  }
  @media (max-width: 1000px) {
    .options {
      position: static;
    }
  }
  .options h3 {
    margin: 0;
    font-size: 14px;
  }
  .opt {
    display: flex;
    flex-direction: column;
    gap: 7px;
    padding-top: 10px;
    border-top: 1px solid var(--border);
    font-size: 13px;
  }
  .opt-head {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: 6px;
  }
  .pill {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 0 8px;
    font-size: 11.5px;
    font-weight: 600;
    line-height: 20px;
    border-radius: 999px;
    color: var(--text-2);
    background: var(--surface-3);
  }
  .pill.ok {
    color: var(--ok);
    background: var(--ok-soft);
  }
  .pill.warn {
    color: var(--warn);
    background: var(--warn-soft);
  }
  .check {
    display: flex;
    align-items: center;
    gap: 7px;
    cursor: pointer;
  }
  .pw {
    display: flex;
    align-items: center;
    gap: 6px;
    color: var(--text-3);
  }
  .pw .input {
    flex: 1;
    min-width: 0;
    height: 30px;
  }
  .ok-line,
  .warn-line,
  .err,
  .small {
    display: flex;
    gap: 6px;
    margin: 0;
    font-size: 12.5px;
    line-height: 1.45;
  }
  .ok-line {
    align-items: center;
    color: var(--ok);
  }
  .warn-line {
    color: var(--warn);
  }
  .warn-line :global(svg) {
    flex: none;
    margin-top: 2px;
  }
  .saved {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 12px;
    border-radius: var(--radius);
    background: var(--accent-soft);
    font-size: 13px;
  }
  .saved p {
    margin: 0;
  }

  /* La hoja: siempre como papel (claro), también con el tema oscuro. */
  .sheet {
    --ink: #1b2430;
    --ink-2: #4d5766;
    --rule: #d9dee5;
    color: var(--ink);
    background: #fff;
    border-radius: 6px;
    box-shadow: var(--shadow-lg);
    padding: 44px 52px;
    font-size: 13px;
    line-height: 1.5;
    min-width: 0;
  }
  @media (max-width: 700px) {
    .sheet {
      padding: 26px 22px;
    }
  }
  .loading {
    display: flex;
    align-items: center;
    gap: 8px;
    color: var(--ink-2);
  }
  .sheet-head {
    display: flex;
    align-items: center;
    gap: 14px;
    padding-bottom: 14px;
    border-bottom: 2px solid var(--ink);
  }
  .sheet-head h1 {
    margin: 0;
    font-size: 24px;
    letter-spacing: -0.01em;
  }
  .sheet-head p {
    margin: 2px 0 0;
    color: var(--ink-2);
  }
  .lead {
    margin: 14px 0 6px;
    font-size: 13.5px;
  }
  .dest {
    margin-top: 18px;
    padding: 14px 16px;
    border: 1px solid var(--rule);
    border-radius: 6px;
    break-inside: avoid;
  }
  .dest h2,
  .howto h2 {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 0 0 10px;
    font-size: 15px;
  }
  .num {
    display: grid;
    place-items: center;
    width: 22px;
    height: 22px;
    font-size: 12px;
    border-radius: 50%;
    color: #fff;
    background: var(--ink);
  }
  .kind {
    margin-left: auto;
    font-size: 11.5px;
    font-weight: 600;
    color: var(--ink-2);
  }
  dl {
    display: grid;
    grid-template-columns: 150px minmax(0, 1fr);
    gap: 6px 12px;
    margin: 0;
  }
  dt {
    font-weight: 600;
    color: var(--ink-2);
  }
  dd {
    display: flex;
    flex-direction: column;
    margin: 0;
    min-width: 0;
    overflow-wrap: anywhere;
  }
  .mono {
    font-family: var(--mono);
    font-size: 12px;
  }
  .small-id {
    font-size: 11px;
  }
  .note {
    font-size: 12px;
    color: var(--ink-2);
  }
  .pw-line .blank {
    display: block;
    height: 22px;
    border-bottom: 1px solid var(--ink);
    max-width: 360px;
  }
  .printed-pw {
    font-size: 14px;
    font-weight: 650;
  }
  .cmds p {
    margin: 12px 0 4px;
    color: var(--ink-2);
  }
  .cmds pre {
    margin: 0;
    padding: 8px 10px;
    font-family: var(--mono);
    font-size: 11.5px;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    background: #f3f5f8;
    border-radius: 4px;
  }
  .howto {
    margin-top: 22px;
    break-inside: avoid-page;
  }
  .howto h2 {
    margin-top: 16px;
  }
  .howto ol {
    margin: 0;
    padding-left: 20px;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .howto p {
    margin: 0;
  }
  .sheet-foot {
    margin-top: 24px;
    padding-top: 10px;
    border-top: 1px solid var(--rule);
    font-size: 11.5px;
    color: var(--ink-2);
  }

  @media print {
    :global(.app),
    :global(.toaster) {
      display: none !important;
    }
    :global(html),
    :global(body) {
      background: #fff !important;
    }
    .no-print {
      display: none !important;
    }
    .kit-overlay {
      position: static;
      overflow: visible;
      background: #fff;
    }
    .layout {
      display: block;
      padding: 0;
    }
    .sheet {
      box-shadow: none;
      padding: 0;
      border-radius: 0;
    }
    .cmds pre {
      background: #f3f5f8 !important;
      -webkit-print-color-adjust: exact;
      print-color-adjust: exact;
    }
  }
</style>
