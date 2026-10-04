<script lang="ts">
  import { onMount } from "svelte";
  import { slide } from "svelte/transition";
  import { dur } from "$lib/motion";
  import { Check, ChevronRight, CircleAlert, CircleCheck, Copy, Info, ListX, TriangleAlert, X } from "@lucide/svelte";
  import * as api from "$lib/api";
  import type { Plan, Repo } from "$lib/api";
  import { agent, refreshAgent } from "$lib/agent.svelte";
  import { displayPath, isWithin, toSnapshotPath } from "$lib/paths";
  import { restRepoPath } from "$lib/retention";
  import { withPassword } from "$lib/passwordPrompt.svelte";
  import { toast } from "$lib/toast.svelte";
  import Modal from "./Modal.svelte";
  import HelpLink from "./HelpLink.svelte";

  // «Excluir de las próximas copias»: añade carpetas o archivos de una versión
  // a las exclusiones de las copias (planes) del destino que los incluyen.
  interface Props {
    repo: Repo;
    /** Rutas dentro de la versión (formato de restic: `/C/Users/…`). */
    paths: string[];
    /** Las rutas de la versión son de Windows. */
    windowsStyle: boolean;
    onclose: () => void;
    /** El destino se guardó con las exclusiones nuevas. */
    onsaved: (repo: Repo) => void;
  }
  let { repo, paths, windowsStyle, onclose, onsaved }: Props = $props();

  interface Item {
    /** Ruta dentro de la versión. */
    path: string;
    /** Patrón de exclusión: la ruta absoluta del sistema (`S:\Carpeta` o `/home/…`). */
    pattern: string;
  }

  // svelte-ignore state_referenced_locally
  const items: Item[] = [...new Set(paths)].sort().map((path) => ({ path, pattern: displayPath(path, windowsStyle) }));

  /** Clave para comparar patrones: en Windows sin distinguir mayúsculas ni el separador. */
  // svelte-ignore state_referenced_locally
  const keyOf = (p: string) => (windowsStyle ? p.trim().replace(/\//g, "\\").replace(/\\+$/, "").toLowerCase() : p.trim().replace(/\/+$/, ""));

  /** Copias del destino que copian alguno de los elementos, con cuáles. */
  const matches = $derived(
    repo.plans
      .map((plan) => {
        const roots = plan.paths.map(toSnapshotPath);
        const inside = items.filter((it) => roots.some((r) => isWithin(it.path, r, windowsStyle)));
        // El elemento es una de las carpetas de la copia (o la contiene): se dejaría de copiar entera.
        const whole = inside.filter((it) => roots.some((r) => isWithin(r, it.path, windowsStyle)));
        return { plan, inside, whole };
      })
      .filter((m) => m.inside.length > 0),
  );
  /** Elementos que ninguna copia de este destino copia. */
  const orphans = $derived(items.filter((it) => !matches.some((m) => m.inside.includes(it))));

  // Se preseleccionan todas las copias que los incluyen.
  // svelte-ignore state_referenced_locally
  let chosen = $state<Set<string>>(new Set(matches.map((m) => m.plan.id)));
  const chosenMatches = $derived(matches.filter((m) => chosen.has(m.plan.id)));

  /** Exclusiones resultantes de una copia: las que tenía más las nuevas, sin repetir. */
  function merged(plan: Plan, add: Item[]) {
    const seen = new Set(plan.excludes.map(keyOf));
    const added: string[] = [];
    const already: string[] = [];
    for (const it of add) {
      if (seen.has(keyOf(it.pattern))) already.push(it.pattern);
      else {
        seen.add(keyOf(it.pattern));
        added.push(it.pattern);
      }
    }
    return { existing: plan.excludes, added, already, all: [...plan.excludes, ...added] };
  }

  const newCount = $derived(chosenMatches.reduce((n, m) => n + merged(m.plan, m.inside).added.length, 0));

  function toggle(id: string) {
    const next = new Set(chosen);
    if (next.has(id)) next.delete(id);
    else next.add(id);
    chosen = next;
  }

  onMount(() => {
    refreshAgent();
  });

  // Resultado tras guardar (se conserva aunque `repo` cambie al actualizarse).
  let done = $state<{ planNames: string[]; items: Item[]; agentPending: boolean; agentError: string } | null>(null);
  let busy = $state(false);

  const quote = (s: string) => `"${s.replace(/"/g, '\\"')}"`;
  const names = (list: string[]) =>
    list.length === 1 ? `«${list[0]}»` : `${list.slice(0, -1).map((n) => `«${n}»`).join(", ")} y «${list.at(-1)}»`;

  async function save() {
    const targets = chosenMatches.filter((m) => merged(m.plan, m.inside).added.length > 0);
    if (!targets.length) return;
    const byId = new Map(targets.map((m) => [m.plan.id, merged(m.plan, m.inside).all]));
    const updated = ($state.snapshot(repo.plans) as Plan[]).map((p) => (byId.has(p.id) ? { ...p, excludes: byId.get(p.id)! } : p));

    // Si el agente copia este destino por planes y alguna de estas copias tiene horario, hay que actualizarlo.
    const info = agent.info;
    const inAgent = !!info?.supported && !!info.repos.some((r) => r.id === repo.id && r.schedule.kind === "plans");
    const affectsAgent = inAgent && targets.some((m) => m.plan.schedule);
    const elevated = !!info?.elevated;
    const planNames = targets.map((m) => m.plan.name);
    const usedItems = items.filter((it) => targets.some((m) => m.inside.includes(it)));
    let agentError = "";

    busy = true;
    const ok = await withPassword({
      title: "Excluir de las próximas copias",
      message: `Se añadirán ${newCount === 1 ? "1 exclusión" : `${newCount} exclusiones`} a ${planNames.length === 1 ? "la copia" : "las copias"} ${names(planNames)}.${affectsAgent && elevated ? " Las copias automáticas también las usarán." : ""}`,
      repoName: repo.name,
      confirmLabel: "Guardar exclusiones",
      action: async (password) => {
        onsaved(await api.setPlans(repo.id, updated, password));
        if (affectsAgent && elevated) {
          try {
            agent.info = await api.agentSetSchedule(repo.id, { kind: "plans" }, password);
          } catch (e) {
            // Las exclusiones ya se guardaron: no se repite, solo se avisa.
            agentError = String(e);
          }
        }
      },
    });
    busy = false;
    if (ok) {
      done = { planNames, items: usedItems, agentPending: affectsAgent && !elevated, agentError };
      toast(`Exclusiones guardadas en ${names(planNames)}`);
    }
  }

  // --- Quitar también de las versiones anteriores (solo se explica) ---
  let showRewrite = $state(false);
  let copied = $state(false);

  /** `-r` para los comandos: en un servidor REST, la carpeta del repositorio en el propio servidor. */
  const restPath = $derived(restRepoPath(repo.location));
  const repoArg = $derived(
    restPath !== null ? `/ruta/de/rest-server${restPath ? `/${restPath}` : ""}` : /\s/.test(repo.location) ? quote(repo.location) : repo.location,
  );
  const backslashes = $derived(restPath === null && /Windows/i.test(navigator.userAgent));
  const commands = $derived.by(() => {
    if (!done) return "";
    // En `rewrite` las rutas se escriben como las guarda restic (`/C/Users/…`),
    // con el separador del sistema donde se ejecuta: `/` en el servidor (Linux),
    // `\` si se ejecuta en este equipo Windows (comprobado con restic 0.18).
    const ex = done.items.map((it) => `--exclude ${quote(backslashes ? it.path.replaceAll("/", "\\") : it.path)}`).join(" ");
    return [
      "# 1. Prueba: muestra qué cambiaría, sin tocar nada",
      `restic -r ${repoArg} rewrite --forget --dry-run ${ex}`,
      "# 2. Reescribe las versiones sin esos elementos y olvida las originales (irreversible)",
      `restic -r ${repoArg} rewrite --forget ${ex}`,
      "# 3. Libera el espacio que ya no usa ninguna versión",
      `restic -r ${repoArg} prune`,
    ].join("\n");
  });

  async function copyCommands() {
    try {
      await navigator.clipboard.writeText(commands);
      copied = true;
      setTimeout(() => (copied = false), 1600);
    } catch {
      /* el portapapeles puede no estar disponible */
    }
  }
</script>

<Modal {onclose} labelledby="exclude-title" width={640} dismissible={!busy}>
  <header class="dlg-head">
    <div class="dlg-title">
      <span class="ticon"><ListX size={19} /></span>
      <div>
        <h2 id="exclude-title">Excluir de las próximas copias <HelpLink topic="ocupa-espacio" label="excluir lo que ocupa espacio" /></h2>
        <p class="faint">Repositorio «{repo.name}»</p>
      </div>
    </div>
    <button class="icon-btn" title="Cerrar" aria-label="Cerrar" onclick={onclose}><X size={17} /></button>
  </header>

  {#if done}
    <div class="notice notice-success">
      <CircleCheck size={16} />
      <p>
        <strong>Exclusiones guardadas en {names(done.planNames)}.</strong>
        Las próximas copias ya no incluirán estos elementos. Las versiones que ya existen los siguen guardando hasta que la retención las elimine.
      </p>
    </div>
    {#if done.agentPending}
      <div class="notice notice-warn">
        <TriangleAlert size={16} />
        <p>
          Las copias automáticas usarán las nuevas exclusiones cuando apliques el cambio en <strong>«Copias automáticas»</strong> como administrador.
        </p>
      </div>
    {/if}
    {#if done.agentError}
      <div class="notice notice-danger" role="alert">
        <CircleAlert size={16} />
        <p>No se pudo actualizar el agente: {done.agentError}. Aplica el cambio en «Copias automáticas».</p>
      </div>
    {/if}

    <div class="rewrite">
      <button class="disclosure" aria-expanded={showRewrite} onclick={() => (showRewrite = !showRewrite)}>
        <span class="chev" class:open={showRewrite}><ChevronRight size={15} /></span>
        ¿Quieres quitarlos también de las versiones anteriores?
      </button>
      {#if showRewrite}
        <div class="rewrite-body" transition:slide={{ duration: dur(180) }}>
          <p>
            Resguardo no lo hace por ti. Hay que reescribir las versiones con <code>restic rewrite --exclude … --forget</code> y después liberar el
            espacio con <code>restic prune</code>. Afecta a todas las versiones de «{repo.name}» que contengan esas rutas, sean de la copia que sean.
          </p>
          {#if restPath !== null}
            <p>
              Tiene que ejecutarse donde el repositorio se pueda modificar. En un servidor REST en modo <em>append-only</em> (solo añadir), eso es
              <strong>en el propio servidor</strong>, con la ruta local del repositorio (la carpeta de datos de rest-server).
            </p>
          {:else}
            <p>Tiene que ejecutarse en un equipo con acceso a «{repo.name}» y permiso para modificarlo, con la contraseña del repositorio.</p>
          {/if}
          <div class="notice notice-warn">
            <TriangleAlert size={16} />
            <p>
              <strong>Es irreversible:</strong> lo excluido desaparece de todas las versiones y ya no se podrá restaurar. Ejecuta primero el paso 1 (<code>--dry-run</code>)
              y revisa lo que muestra. Que no haya copias en marcha mientras tanto.
            </p>
          </div>
          <div class="cmd">
            <pre class="mono selectable">{commands}</pre>
            <button class="btn btn-sm copy" onclick={copyCommands} title="Copiar los comandos">
              {#if copied}<Check size={13} /> Copiados{:else}<Copy size={13} /> Copiar{/if}
            </button>
          </div>
          <p class="faint small">
            En <code>rewrite</code> las rutas se escriben como las guarda restic{windowsStyle ? (backslashes ? " (C:\\Users → \\C\\Users)" : " (C:\\Users → /C/Users)") : ""}.
            {#if restPath !== null}Cambia <code>/ruta/de/rest-server</code> por la carpeta real del servidor.{/if}
          </p>
        </div>
      {/if}
    </div>

    <footer>
      <button class="btn btn-primary" onclick={onclose}>Hecho</button>
    </footer>
  {:else}
    <p class="lead">Se excluirán estas rutas en las próximas copias:</p>
    <ul class="items">
      {#each items as it (it.path)}
        <li class="mono" title={it.pattern}>{it.pattern}</li>
      {/each}
    </ul>

    {#if matches.length === 0}
      <div class="notice notice-info">
        <Info size={16} />
        <p>
          Ninguna copia de «{repo.name}» copia {items.length === 1 ? "este elemento" : "estos elementos"}: puede que ya no exista la copia que hizo esta
          versión, o que copie otras carpetas. No hay nada que excluir.
        </p>
      </div>
      <footer><button class="btn btn-primary" onclick={onclose}>Entendido</button></footer>
    {:else}
      <p class="lead">
        {matches.length === 1 ? "Copia que los incluye:" : "Copias que los incluyen (elige en cuáles excluirlos):"}
      </p>
      <div class="plans">
        {#each matches as m (m.plan.id)}
          {@const res = merged(m.plan, m.inside)}
          <div class="plan" class:on={chosen.has(m.plan.id)}>
            <label class="plan-head">
              <input type="checkbox" checked={chosen.has(m.plan.id)} onchange={() => toggle(m.plan.id)} />
              <strong>{m.plan.name}</strong>
              <span class="faint">
                {m.plan.schedule ? "con horario" : "a mano"} ·
                {res.added.length ? `${res.added.length} ${res.added.length === 1 ? "exclusión nueva" : "exclusiones nuevas"}` : "ya las tiene todas"}
              </span>
            </label>
            {#if chosen.has(m.plan.id)}
              <div class="preview" transition:slide={{ duration: dur(150) }}>
                <span class="faint label">Exclusiones de la copia después de guardar:</span>
                <ul>
                  {#each res.existing as ex}<li class="mono">{ex}</li>{/each}
                  {#each res.added as ex}<li class="mono new">{ex}<span class="badge badge-sm tone-accent">nueva</span></li>{/each}
                </ul>
                {#if res.already.length}
                  <span class="faint small">Ya estaba{res.already.length === 1 ? "" : "n"}: {res.already.join(", ")}</span>
                {/if}
                {#if m.whole.length}
                  <div class="notice notice-warn">
                    <TriangleAlert size={16} />
                    <p>
                      {m.whole.map((w) => w.pattern).join(", ")}
                      {m.whole.length === 1 ? "es una de las carpetas" : "son carpetas"} que copia «{m.plan.name}»: dejará{m.whole.length === 1 ? "" : "n"} de copiarse entera{m.whole.length === 1 ? "" : "s"}.
                    </p>
                  </div>
                {/if}
              </div>
            {/if}
          </div>
        {/each}
      </div>

      {#if orphans.length}
        <p class="faint small">
          Ninguna copia de este repositorio copia {orphans.map((o) => o.pattern).join(", ")}: no se {orphans.length === 1 ? "añade" : "añaden"} a ninguna.
        </p>
      {/if}

      <p class="faint small">
        Lo excluido no se copia, así que tampoco se podrá restaurar de las próximas versiones. Las que ya existen no cambian.
      </p>

      <footer>
        <button class="btn btn-ghost" onclick={onclose}>Cancelar</button>
        <button
          class="btn btn-primary"
          onclick={save}
          disabled={busy || newCount === 0}
          title={newCount ? "" : chosen.size ? "Las copias elegidas ya tienen estas exclusiones" : "Elige al menos una copia"}
        >
          <ListX size={14} /> Guardar exclusiones
        </button>
      </footer>
    {/if}
  {/if}
</Modal>

<style>
  .lead {
    margin: 0 0 8px;
    font-size: var(--fs-sm);
    font-weight: 550;
  }
  .items {
    display: flex;
    flex-direction: column;
    gap: 3px;
    max-height: 140px;
    overflow: auto;
    margin: 0 0 14px;
    padding: 8px 12px;
    list-style: none;
    font-size: var(--fs-sm);
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
  }
  .items li {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .plans {
    display: flex;
    flex-direction: column;
    gap: 8px;
    margin-bottom: 12px;
  }
  .plan {
    padding: 10px 12px;
    border: 1.5px solid var(--border);
    border-radius: var(--radius);
    transition: border-color 0.15s;
  }
  .plan.on {
    border-color: var(--accent);
  }
  .plan-head {
    display: flex;
    align-items: center;
    gap: 10px;
    cursor: pointer;
  }
  .plan-head .faint {
    margin-left: auto;
    font-size: var(--fs-sm);
    white-space: nowrap;
  }
  input[type="checkbox"] {
    width: 15px;
    height: 15px;
    accent-color: var(--accent);
    cursor: pointer;
  }
  .preview {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin-top: 10px;
    padding-left: 25px;
  }
  .preview .label {
    font-size: var(--fs-xs);
  }
  .preview ul {
    display: flex;
    flex-direction: column;
    gap: 2px;
    max-height: 160px;
    overflow: auto;
    margin: 0;
    padding: 6px 10px;
    list-style: none;
    font-size: var(--fs-sm);
    background: var(--surface-2);
    border-radius: var(--radius-sm);
  }
  .preview li {
    display: flex;
    align-items: center;
    gap: 8px;
    color: var(--text-2);
    overflow-wrap: anywhere;
  }
  .preview li.new {
    color: var(--text-1);
    font-weight: 600;
  }
  .small {
    margin: 0 0 8px;
    font-size: var(--fs-sm);
    line-height: 1.5;
  }
  .rewrite {
    margin-top: 12px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
  }
  .disclosure {
    display: flex;
    align-items: center;
    gap: 6px;
    width: 100%;
    padding: 10px 12px;
    font: inherit;
    font-size: var(--fs-sm);
    font-weight: 600;
    text-align: left;
    color: var(--text-1);
    background: none;
    border: none;
    cursor: pointer;
  }
  .chev {
    display: grid;
    color: var(--text-3);
    transition: transform 0.15s;
  }
  .chev.open {
    transform: rotate(90deg);
  }
  .rewrite-body {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 0 14px 14px;
    font-size: var(--fs-sm);
    line-height: 1.55;
  }
  .rewrite-body > p {
    margin: 0;
  }
  code {
    padding: 0 4px;
    font-size: 0.92em;
    border-radius: 4px;
    background: var(--surface-3);
  }
  .cmd {
    position: relative;
  }
  pre {
    margin: 0;
    padding: 12px 14px;
    padding-right: 96px;
    overflow: auto;
    font-size: var(--fs-xs);
    line-height: 1.6;
    white-space: pre-wrap;
    overflow-wrap: break-word;
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
  }
  .copy {
    position: absolute;
    top: 8px;
    right: 8px;
  }
  .notice + .notice,
  .notice + .rewrite {
    margin-top: 10px;
  }
  footer {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 16px;
  }
</style>
