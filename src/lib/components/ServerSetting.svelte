<script lang="ts">
  // «Servidor de copias» (fase 4, docs/compartir.md): este equipo guarda las
  // copias de otros equipos en una carpeta, con el rest-server oficial (solo
  // añadir, cada equipo en su carpeta, TLS y firewall).
  import { onMount } from "svelte";
  import { open } from "@tauri-apps/plugin-dialog";
  import { Check, CircleAlert, Copy, FolderOpen, Globe, Info, LoaderCircle, Lock, Plus, Server, ShieldCheck, Trash2 } from "@lucide/svelte";
  import * as api from "$lib/api";
  import type { NewServerUser, ServerStatus } from "$lib/api";
  import { formatRelative } from "$lib/format";
  import { relaunchToSettings } from "$lib/settingsNav.svelte";
  import { toast } from "$lib/toast.svelte";
  import HelpLink from "./HelpLink.svelte";
  import SettingRow from "./SettingRow.svelte";

  let srv = $state<ServerStatus | null>(null);
  let error = $state("");
  let busy = $state(false);
  let editing = $state(false);
  let path = $state("");
  let port = $state(8000);
  let localOnly = $state(true);
  let newUser = $state("");
  let created = $state<NewServerUser | null>(null);
  let copied = $state(false);
  let publicIp = $state<string | null>(null);

  async function load() {
    try {
      srv = await api.serverStatus();
    } catch (e) {
      error = String(e);
    }
  }
  onMount(load);

  function startEdit() {
    if (!srv) return;
    path = srv.path;
    port = srv.port || 8000;
    localOnly = srv.local_subnet_only;
    editing = true;
    error = "";
  }

  async function pick() {
    const p = await open({ directory: true, title: "Carpeta del Servidor de copias" });
    if (typeof p === "string") path = p;
  }

  async function run(fn: () => Promise<ServerStatus>, ok: string) {
    busy = true;
    error = "";
    try {
      srv = await fn();
      editing = false;
      toast(ok);
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  async function addUser(e: SubmitEvent) {
    e.preventDefault();
    if (!newUser.trim()) return;
    busy = true;
    error = "";
    try {
      created = await api.serverAddUser(newUser.trim());
      srv = created.status;
      newUser = "";
    } catch (err) {
      error = String(err);
    } finally {
      busy = false;
    }
  }

  async function removeUser(name: string) {
    await run(() => api.serverRemoveUser(name), `«${name}» ya no puede entrar en este servidor (sus copias se quedan en la carpeta)`);
  }

  // La dirección pública solo se consulta si se pide (a un servicio externo).
  async function lookupPublic() {
    try {
      publicIp = await api.serverPublicIp();
    } catch {
      publicIp = "no disponible";
    }
  }

  const elevated = $derived(!!srv?.elevated);
</script>

<SettingRow
  label="Servidor de copias"
  description="Guarda en una carpeta de este equipo las copias de tus otros equipos. Cada uno solo puede añadir a la suya: nadie borra ni ve lo de otro."
  needs={["admin"]}
>
  {#if srv?.enabled}
    <span class="badge badge-sm {srv.running ? 'tone-ok' : 'tone-warn'}">{srv.running ? "En marcha" : "Arrancando…"}</span>
  {:else if srv && !editing}
    <button class="btn btn-sm" onclick={() => (elevated ? startEdit() : relaunchToSettings("equipo").catch((e) => (error = String(e))))} disabled={!srv.supported}>
      {#if elevated}<Server size={13} /> Activar{:else}<ShieldCheck size={13} /> Abrir como administrador{/if}
    </button>
  {/if}
  {#snippet status()}
    {#if srv?.binary_problem && !srv.enabled}<span class="faint">{srv.binary_problem}</span>{/if}
  {/snippet}
  {#snippet below()}
    {#if editing}
      <div class="form">
        <label class="field">
          <span class="field-label">Carpeta donde se guardan las copias de los demás</span>
          <span class="with-btn">
            <input class="input mono" bind:value={path} placeholder="D:\Copias de otros equipos" />
            <button class="btn" type="button" onclick={pick}><FolderOpen size={14} /> Examinar</button>
          </span>
        </label>
        <label class="field narrow">
          <span class="field-label">Puerto</span>
          <input class="input" type="number" min="1024" max="65535" bind:value={port} />
        </label>
        <label class="check"><input type="checkbox" bind:checked={localOnly} /> Solo equipos de mi red local (recomendado)</label>
        <p class="faint note">
          <Lock size={14} />
          <span>
            Siempre con TLS (un certificado propio que tus equipos fijan), un usuario y una contraseña aleatoria por equipo y en modo «solo añadir». Se crea una
            regla del firewall solo para este puerto. Resguardo nunca abre puertos en el router.
          </span>
        </p>
        <div class="row">
          <button class="btn btn-ghost btn-sm" onclick={() => (editing = false)} disabled={busy}>Cancelar</button>
          <button class="btn btn-primary btn-sm" onclick={() => run(() => api.serverSetup(path, Number(port), localOnly), "Servidor de copias activado")} disabled={busy || !path.trim()}>
            {#if busy}<span class="spin" style="display:grid"><LoaderCircle size={13} /></span>{/if} Activar
          </button>
        </div>
      </div>
    {:else if srv?.enabled}
      <div class="form">
        <dl class="facts">
          <div><dt>Carpeta</dt><dd class="mono">{srv.path}</dd></div>
          <div>
            <dt>Dirección en la red local</dt>
            <dd class="mono">{srv.lan_addresses.length ? srv.lan_addresses.map((ip) => `https://${ip}:${srv?.port}`).join(" · ") : "—"}</dd>
          </div>
          <div><dt>Acceso</dt><dd>{srv.local_subnet_only ? "Solo equipos de la red local" : "Desde cualquier red (si abres el puerto en el router)"}</dd></div>
          {#if srv.tls_sha256}<div><dt>Huella del certificado</dt><dd class="mono small" title="Tus equipos la comprueban al conectar">{srv.tls_sha256.slice(0, 47)}…</dd></div>{/if}
        </dl>

        <h4>Equipos que copian aquí</h4>
        {#if srv.users.length}
          <ul class="users">
            {#each srv.users as u (u.name)}
              <li>
                <span class="txt">
                  <strong>{u.name}</strong>
                  <span class="faint small">
                    {u.repos.length ? `${u.repos.length} ${u.repos.length === 1 ? "repositorio" : "repositorios"}: ${u.repos.join(", ")}` : "Aún sin repositorios"} · desde
                    {formatRelative(u.created_at)}{u.shared ? " · ofrecido a tus equipos" : ""}
                  </span>
                </span>
                <button class="icon-btn" title="Quitar este equipo (sus copias se quedan en la carpeta)" aria-label="Quitar {u.name}" onclick={() => removeUser(u.name)} disabled={busy}><Trash2 size={14} /></button>
              </li>
            {/each}
          </ul>
        {:else}
          <p class="faint small">Todavía ninguno. Añade uno por cada equipo que vaya a copiar aquí.</p>
        {/if}
        <form class="add" onsubmit={addUser}>
          <input class="input" bind:value={newUser} placeholder="Nombre del equipo (p. ej. Altamar-PC1)" maxlength="32" />
          <button class="btn btn-sm" disabled={busy || !newUser.trim()}><Plus size={13} /> Añadir equipo</button>
        </form>

        {#if created}
          <div class="notice notice-success">
            <Check size={16} />
            <div>
              <p>
                <strong>«{created.user}» puede copiar aquí.</strong>
                {#if srv.linked}
                  En ese equipo: Añadir repositorio → «Un destino compartido por otro equipo tuyo» → Pedir; le llega cifrado.
                {:else}
                  Este equipo no está vinculado con Resguardo Web: configura el otro equipo a mano con estos datos (solo se muestran ahora).
                {/if}
              </p>
              {#if !srv.linked}
                <p class="mono small">{created.location} · usuario {created.user} · contraseña {created.password}</p>
                <button
                  class="btn btn-sm"
                  onclick={async () => {
                    await navigator.clipboard.writeText(`${created?.location}\nusuario: ${created?.user}\ncontraseña: ${created?.password}`).catch(() => {});
                    copied = true;
                  }}>{#if copied}<Check size={13} />{:else}<Copy size={13} />{/if} Copiar</button
                >
              {/if}
            </div>
          </div>
        {/if}

        <details class="remote">
          <summary><Globe size={14} /> Copiar desde otra sede (fuera de esta red)</summary>
          <p>
            Hay que abrir en el router el puerto <strong>{srv.port}</strong> hacia esta IP de la red local y desactivar «Solo equipos de mi red local». Los
            otros equipos usarán tu dirección pública: {#if publicIp}<strong class="mono">https://{publicIp}:{srv.port}</strong>{:else}<button
                class="link"
                onclick={lookupPublic}>ver mi dirección pública</button
              >{/if}.
          </p>
          <p class="faint small">
            Es seguro porque todo va cifrado con TLS, cada equipo tiene su propia contraseña aleatoria y el servidor solo deja añadir. Para más seguridad,
            restringe en el router quién puede llegar a ese puerto. Resguardo no abre puertos solo.
          </p>
        </details>

        <div class="row">
          <button class="btn btn-ghost btn-sm" onclick={startEdit} disabled={busy || !elevated}>Cambiar</button>
          <button class="btn btn-ghost btn-sm danger-text" onclick={() => run(api.serverDisable, "Servidor de copias desactivado")} disabled={busy || !elevated}>Desactivar</button>
        </div>
        <p class="faint small"><Info size={13} /> Las copias guardadas se quedan en la carpeta aunque desactives el servidor. <HelpLink topic="servidor-copias" label="el servidor de copias" /></p>
      </div>
    {/if}
    {#if error}<p class="err" role="alert"><CircleAlert size={14} /> {error}</p>{/if}
  {/snippet}
</SettingRow>

<style>
  .form {
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 12px;
    border-radius: var(--radius);
    background: var(--surface-2);
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .narrow {
    max-width: 160px;
  }
  .with-btn {
    display: flex;
    gap: 6px;
  }
  .with-btn .input {
    flex: 1;
  }
  .check {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: var(--fs-sm);
  }
  .note {
    display: flex;
    gap: 8px;
    margin: 0;
    font-size: var(--fs-sm);
  }
  .note :global(svg) {
    flex: none;
    margin-top: 2px;
  }
  .row {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }
  .facts {
    display: grid;
    gap: 6px;
    margin: 0;
    font-size: var(--fs-sm);
  }
  .facts div {
    display: grid;
    grid-template-columns: 190px minmax(0, 1fr);
    gap: 8px;
  }
  .facts dt {
    color: var(--text-3);
  }
  .facts dd {
    margin: 0;
    overflow-wrap: anywhere;
  }
  h4 {
    margin: 4px 0 0;
    font-size: var(--fs-sm);
  }
  .users {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .users li {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    padding: 6px 0;
  }
  .users li + li {
    border-top: 1px solid var(--border);
  }
  .txt {
    display: flex;
    flex-direction: column;
  }
  .add {
    display: flex;
    gap: 6px;
  }
  .add .input {
    flex: 1;
  }
  .small {
    font-size: var(--fs-sm);
  }
  .remote summary {
    display: flex;
    align-items: center;
    gap: 6px;
    cursor: pointer;
    font-size: var(--fs-sm);
    font-weight: 550;
  }
  .remote p {
    margin: 8px 0 0;
    font-size: var(--fs-sm);
  }
  .err {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 8px 0 0;
    font-size: var(--fs-sm);
    color: var(--bad);
  }
</style>
