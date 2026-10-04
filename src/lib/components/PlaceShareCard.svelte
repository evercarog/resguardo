<script lang="ts">
  import { CircleAlert, Info, KeyRound, Share2, ShieldCheck } from "@lucide/svelte";
  import * as api from "$lib/api";
  import type { Repo, ShareStatus } from "$lib/api";
  import { formatDate, formatRelative } from "$lib/format";
  import { withPassword } from "$lib/passwordPrompt.svelte";
  import { openSettings } from "$lib/settingsNav.svelte";
  import { toast } from "$lib/toast.svelte";
  import HelpLink from "./HelpLink.svelte";

  // «Compartir este destino con mis equipos» (fase 3, docs/compartir.md): la
  // cuenta de la nube o el rest-server llega cifrada a los equipos del
  // usuario que lo pidan. Nunca las contraseñas de los repositorios.
  interface Props {
    placeId: string;
    placeName: string;
    /** Repositorios del destino (uno de ellos prueba la autoridad sobre el lugar). */
    repos: Repo[];
  }
  let { placeId, placeName, repos }: Props = $props();

  let status = $state<ShareStatus | null>(null);
  let busy = $state(false);
  let error = $state("");
  let confirmOff = $state(false);

  $effect(() => {
    void placeId;
    api.placeShareStatus(placeId).then(
      (s) => (status = s),
      (e) => (error = String(e)),
    );
  });

  async function set(on: boolean) {
    error = "";
    const repo = repos[0];
    if (!repo) return;
    busy = true;
    await withPassword({
      title: on ? "Compartir el destino" : "Dejar de compartir",
      message: on
        ? `Tus otros equipos podrán pedir la cuenta de «${placeName}» (la clave o el usuario del servidor) y la recibirán cifrada solo para ellos. Las contraseñas de los repositorios no se comparten nunca. Confirma con la contraseña de «${repo.name}».`
        : `Nadie más podrá pedir la cuenta de «${placeName}». Los equipos que ya la recibieron la conservan: para revocarla del todo, rota la clave en el proveedor. Confirma con la contraseña de «${repo.name}».`,
      repoName: repo.name,
      confirmLabel: on ? "Compartir" : "Dejar de compartir",
      danger: !on,
      action: async (password) => {
        status = await api.placeShareSet(placeId, on, repo.id, password);
        toast(on ? `«${placeName}» se comparte con tus equipos` : `«${placeName}» ya no se comparte`);
      },
    });
    busy = false;
    confirmOff = false;
  }

  async function elevate() {
    try {
      await api.relaunchAsAdmin();
    } catch (e) {
      error = String(e);
    }
  }
</script>

{#if status?.shareable}
  <section class="card share">
    <header>
      <span class="ic"><Share2 size={17} /></span>
      <div class="txt">
        <h2 class="section-title">Compartir con mis equipos <HelpLink topic="compartir-destino" label="compartir un destino" /></h2>
        <p class="faint">
          Tus otros equipos pueden pedir esta cuenta para crear en ella su propio repositorio, con su propia contraseña. Llega cifrada solo para el equipo
          que la pide: la web no la ve.
        </p>
      </div>
      <input
        type="checkbox"
        class="switch"
        role="switch"
        aria-label="Compartir este destino con mis equipos"
        checked={status.shared}
        disabled={busy || !status.linked || !status.elevated}
        onclick={(e) => {
          e.preventDefault();
          if (status?.shared) confirmOff = true;
          else void set(true);
        }}
      />
    </header>

    {#if !status.linked}
      <div class="notice notice-info">
        <Info size={16} />
        <p>
          Para compartir, vincula antes este equipo con Resguardo Web.
          <button class="link notice-action" onclick={() => openSettings("equipo")}>Ir a Ajustes → Este equipo</button>
        </p>
      </div>
    {:else if !status.elevated}
      <div class="notice notice-info">
        <ShieldCheck size={16} />
        <p>
          Compartir guarda la cuenta para el agente de este equipo, así que hace falta abrir Resguardo como administrador.
          <button class="link notice-action" onclick={elevate}>Abrir como administrador</button>
        </p>
      </div>
    {:else if status.shared}
      <p class="state">
        Se comparte desde <span title={status.since ? formatDate(status.since) : ""}>{status.since ? formatRelative(status.since) : "hace poco"}</span>.
        {#if status.delivered.length}
          La han recibido: {status.delivered.map(([d, at]) => `«${d}» (${formatRelative(at)})`).join(", ")}.
        {:else}
          Aún no la ha pedido ningún equipo.
        {/if}
      </p>
      <p class="faint hint">
        <Info size={14} />
        <span>Recomendado: un bucket y una clave por cliente (o un usuario del servidor por cliente). Así un equipo de un cliente nunca ve la cuenta de otro.</span>
      </p>
    {/if}

    {#if confirmOff}
      <div class="notice notice-warn">
        <KeyRound size={16} />
        <div>
          <p>
            <strong>Los equipos que ya la recibieron la conservan.</strong> Al dejar de compartir nadie más podrá pedirla, pero para revocarla del todo
            tienes que <strong>rotar la clave</strong>: crea una nueva en la consola del proveedor (o cambia la contraseña del usuario del servidor),
            ponla en los repositorios de este destino y borra la antigua.
          </p>
          <div class="row">
            <button class="btn btn-sm" onclick={() => (confirmOff = false)}>Seguir compartiendo</button>
            <button class="btn btn-sm btn-danger" onclick={() => set(false)} disabled={busy}>Dejar de compartir</button>
          </div>
        </div>
      </div>
    {/if}
    {#if error}<div class="notice notice-danger" role="alert"><CircleAlert size={16} /><p>{error}</p></div>{/if}
  </section>
{/if}

<style>
  .share {
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 16px 20px;
  }
  header {
    display: flex;
    align-items: flex-start;
    gap: 12px;
  }
  .ic {
    display: grid;
    place-items: center;
    width: 32px;
    height: 32px;
    flex: none;
    border-radius: 8px;
    color: var(--accent-text);
    background: var(--accent-soft);
  }
  .txt {
    flex: 1;
    min-width: 0;
  }
  .txt p {
    margin: 4px 0 0;
    font-size: var(--fs-sm);
  }
  .state {
    margin: 0;
    font-size: var(--fs-sm);
  }
  .hint {
    display: flex;
    gap: 8px;
    margin: 0;
    font-size: var(--fs-sm);
  }
  .hint :global(svg) {
    flex: none;
    margin-top: 2px;
  }
  .row {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 8px;
  }
</style>
