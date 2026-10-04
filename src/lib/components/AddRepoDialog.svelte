<script lang="ts">
  import { tick } from "svelte";
  import { slide } from "svelte/transition";
  import { dur } from "$lib/motion";
  import { open } from "@tauri-apps/plugin-dialog";
  import {
    ArrowLeft,
    ChevronRight,
    CircleAlert,
    Cloud,
    Database,
    Eye,
    EyeOff,
    FileKey,
    FolderOpen,
    Globe,
    HardDrive,
    Info,
    KeyRound,
    LoaderCircle,
    Server,
    ShieldAlert,
    Share2,
    TriangleAlert,
    User,
    X,
  } from "@lucide/svelte";
  import * as api from "$lib/api";
  import type { Repo } from "$lib/api";
  import { repoKind } from "$lib/repoKind";
  import HelpLink from "./HelpLink.svelte";
  import Modal from "./Modal.svelte";
  import Stepper from "./Stepper.svelte";
  import Advanced from "./Advanced.svelte";
  import InfoTip from "./InfoTip.svelte";
  import { refreshAgent } from "$lib/agent.svelte";

  interface Props {
    /** Destinos que ya existen (para reutilizar sus claves de la nube). */
    repos?: Repo[];
    onclose: () => void;
    oncreated: (repo: Repo) => void;
    /** Texto del paso («Paso 1 de 3»), si va dentro del asistente de copia nueva. */
    /** Dentro del asistente «Nueva copia»: sus pasos. */
    steps?: { labels: string[]; current: number };
    /** «Usar un destino compartido» por otro equipo del usuario. */
    onshared?: () => void;
  }
  let { repos = [], onclose, oncreated, steps: wizardSteps, onshared }: Props = $props();

  type Kind = "local" | "rest" | "cloud" | "sftp" | "other";
  const KINDS = [
    { id: "local", label: "Carpeta o disco", sub: "Un disco externo, un USB o una carpeta compartida de la red", icon: HardDrive },
    { id: "rest", label: "Servidor REST", sub: "Un rest-server en tu red o en Internet", icon: Globe },
    { id: "cloud", label: "Nube compatible con S3", sub: "Backblaze B2, Wasabi, Cloudflare R2, Amazon S3…", icon: Cloud },
    { id: "sftp", label: "SFTP", sub: "Un servidor al que entras por SSH", icon: Server },
    { id: "other", label: "Otro", sub: "Cualquier ubicación de restic: rclone, Azure, B2 nativo…", icon: Database },
  ] as const;

  // Proveedores compatibles con S3 (mismos que la copia externa).
  const PROVIDERS = [
    { id: "b2", label: "Backblaze B2", region: "us-west-004", endpoint: (r: string) => `s3.${r}.backblazeb2.com` },
    { id: "wasabi", label: "Wasabi", region: "us-east-1", endpoint: (r: string) => `s3.${r}.wasabisys.com` },
    { id: "r2", label: "Cloudflare R2", region: "auto", endpoint: (a: string) => `${a}.r2.cloudflarestorage.com` },
    { id: "aws", label: "Amazon S3", region: "us-east-1", endpoint: (r: string) => `s3.${r}.amazonaws.com` },
    { id: "s3", label: "Otro S3", region: "", endpoint: (e: string) => e },
  ] as const;
  type ProviderId = (typeof PROVIDERS)[number]["id"];

  let step = $state<"type" | "form">("type");
  let mode = $state<"existing" | "new">("new");
  let kind = $state<Kind>("local");
  let name = $state("");
  let path = $state(""); // local, sftp y otro
  let restUrl = $state("");
  let restUser = $state("");
  let restPassword = $state("");
  let cacert = $state("");
  // Nube
  let provider = $state<ProviderId>("b2");
  let region = $state<string>("us-west-004");
  let account = $state(""); // R2: id de cuenta · S3 genérico: servidor
  let bucket = $state("");
  let folder = $state("");
  let keysFrom = $state(""); // id del repositorio cuyas claves se reutilizan ("" = claves nuevas)
  let keyId = $state("");
  let keySecret = $state("");
  let password = $state("");
  let confirm = $state("");
  let showPasswords = $state(false);
  let saving = $state(false);
  let slow = $state(false);
  let checkId = "";
  let error = $state("");
  let nameInput = $state<HTMLInputElement>();

  // Segundo paso (si el equipo está vinculado a la web): vigilar el destino.
  let created = $state<Repo | null>(null);
  let watchElevated = $state(false);
  let watchEvery = $state(24);
  let watching = $state(false);
  let watchError = $state("");

  const mismatch = $derived(mode === "new" && confirm.length > 0 && confirm !== password);
  const kindInfo = $derived(KINDS.find((k) => k.id === kind)!);
  const prov = $derived(PROVIDERS.find((p) => p.id === provider)!);

  /** Con --private-repos la URL necesita la carpeta del usuario: avisar si solo hay host y puerto. */
  const restMissingPath = $derived.by(() => {
    try {
      const url = new URL(restUrl.trim().replace(/^rest:/, ""));
      return kind === "rest" && !!restUser && (url.pathname === "/" || url.pathname === "");
    } catch {
      return false;
    }
  });

  /** HTTP sin TLS hacia otra máquina: las credenciales del servidor viajarían en claro. */
  const insecureHttp = $derived.by(() => {
    try {
      const url = new URL(restUrl.trim().replace(/^rest:/, ""));
      return url.protocol === "http:" && !["localhost", "127.0.0.1", "[::1]"].includes(url.hostname);
    } catch {
      return false;
    }
  });

  const slug = (s: string) =>
    s
      .normalize("NFD")
      .replace(/[\u0300-\u036f]/g, "")
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, "-")
      .replace(/^-|-$/g, "");

  const cloudLocation = $derived.by(() => {
    const host = (provider === "r2" || provider === "s3" ? prov.endpoint(account.trim()) : prov.endpoint(region.trim())).replace(/^https?:\/\//, "");
    const b = bucket.trim().replace(/^\/+|\/+$/g, "");
    const f = (folder.trim() || slug(name)).replace(/^\/+|\/+$/g, "");
    return host && b ? `s3:https://${host}/${[b, f].filter(Boolean).join("/")}` : "";
  });

  const location = $derived.by(() => {
    if (kind === "rest") return `rest:${restUrl.trim().replace(/^rest:/, "")}`;
    if (kind === "cloud") return cloudLocation;
    if (kind === "sftp" && path.trim() && !path.trim().startsWith("sftp:")) return `sftp:${path.trim()}`;
    return path.trim();
  });

  /** «Otro» con una ubicación que usa claves (S3, B2, Azure): se piden, opcionales. */
  const otherNeedsKeys = $derived(kind === "other" && /^(s3|b2|azure):/i.test(path.trim()));
  const usesKeys = $derived(kind === "cloud" || otherNeedsKeys);

  /** Destinos con claves guardadas del mismo proveedor (misma familia de ubicación). */
  const keySources = $derived.by(() => {
    if (!usesKeys) return [];
    // Las claves de B2 valen tanto para «b2:» como para su API compatible con S3: basta con que coincida el proveedor.
    const label = kind === "cloud" ? (provider === "s3" ? repoKind(`s3:https://${account.trim() || "otro.s3"}/b`).label : prov.label) : repoKind(path).label;
    return repos.filter((r) => r.cloud_key_id && repoKind(r.location).label === label);
  });
  // Si el destino elegido para las claves deja de valer (otro proveedor), se vuelve a claves nuevas.
  $effect(() => {
    if (keysFrom && !keySources.some((r) => r.id === keysFrom)) keysFrom = "";
  });

  const keyLabels = $derived(
    /^azure:/i.test(path.trim()) && kind === "other"
      ? { id: "Nombre de la cuenta de almacenamiento", secret: "Clave de la cuenta" }
      : /^b2:/i.test(path.trim()) && kind === "other"
        ? { id: "ID de la clave de aplicación (keyID)", secret: "Clave de aplicación (applicationKey)" }
        : { id: "ID de la clave (Access key)", secret: "Clave secreta (Secret key)" },
  );

  async function choose(k: Kind) {
    kind = k;
    step = "form";
    error = "";
    await tick();
    nameInput?.focus();
  }

  function pickProvider(id: ProviderId) {
    provider = id;
    const p = PROVIDERS.find((x) => x.id === id)!;
    region = p.region;
    account = "";
  }

  /** Si pegan la URL con usuario y contraseña, se pasan a sus campos y se quitan de la URL. */
  function extractCredentials() {
    const raw = restUrl.trim().replace(/^rest:/, "");
    try {
      const url = new URL(raw);
      if (!url.username && !url.password) return;
      if (url.username) restUser = decodeURIComponent(url.username);
      if (url.password) restPassword = decodeURIComponent(url.password);
      url.username = "";
      url.password = "";
      restUrl = url.toString();
    } catch {
      /* URL incompleta: se valida al guardar */
    }
  }

  async function browseFolder() {
    const picked = await open({ directory: true, title: "Carpeta del repositorio" });
    if (typeof picked === "string") path = picked;
  }

  async function browseCert() {
    const picked = await open({
      title: "Certificado de la CA",
      filters: [{ name: "Certificados", extensions: ["pem", "crt", "cer"] }],
    });
    if (typeof picked === "string") cacert = picked;
  }

  async function submit(event: SubmitEvent) {
    event.preventDefault();
    error = "";
    if (mode === "new" && password !== confirm) {
      error = "Las contraseñas no coinciden.";
      return;
    }
    if (kind === "rest") {
      extractCredentials();
      if (!/^https?:\/\//i.test(restUrl.trim().replace(/^rest:/, ""))) {
        error = "La URL del servidor debe empezar por http:// o https://";
        return;
      }
      if (restUser && !restPassword) {
        error = "Falta la contraseña del servidor.";
        return;
      }
    }
    if (kind === "cloud") {
      if (!cloudLocation) {
        error = provider === "s3" ? "Escribe el servidor (endpoint) y el bucket." : provider === "r2" ? "Escribe el ID de cuenta y el bucket." : "Escribe el bucket.";
        return;
      }
      if (!keysFrom && (!keyId.trim() || !keySecret.trim())) {
        error = "Escribe el ID y la clave secreta de acceso a la nube.";
        return;
      }
    }
    if (otherNeedsKeys && !keysFrom && !!keyId.trim() !== !!keySecret.trim()) {
      error = "Escribe las dos claves (el ID y la secreta), o ninguna.";
      return;
    }
    saving = true;
    slow = false;
    checkId = crypto.randomUUID();
    const mine = checkId;
    const slowTimer = setTimeout(() => (slow = true), 8000);
    const effectiveRegion = kind === "cloud" ? (provider === "r2" ? "auto" : region.trim() || null) : null;
    try {
      const repo = await api.addRepo({
        checkId: mine,
        name,
        location,
        password,
        create: mode === "new",
        restUsername: kind === "rest" ? restUser || null : null,
        restPassword: kind === "rest" && restUser ? restPassword : null,
        cacert: kind === "rest" ? cacert || null : null,
        cloudFrom: usesKeys && keysFrom ? keysFrom : null,
        cloudKeyId: usesKeys && !keysFrom ? keyId.trim() || null : null,
        cloudKeySecret: usesKeys && !keysFrom ? keySecret.trim() || null : null,
        cloudRegion: usesKeys && !keysFrom ? effectiveRegion : null,
      });
      if (mine === checkId) {
        let linked = false;
        try {
          const web = await api.webInfo();
          linked = !!web.link && !web.link.revoked;
          watchElevated = web.elevated;
        } catch {
          /* sin datos de la web: se omite el segundo paso */
        }
        if (linked) created = repo;
        else oncreated(repo);
      }
    } catch (e) {
      if (mine === checkId) error = String(e);
    } finally {
      clearTimeout(slowTimer);
      if (mine === checkId) {
        saving = false;
        slow = false;
      }
    }
  }

  /** Detiene la comprobación en curso sin cerrar el diálogo, para poder corregir los datos. */
  async function stopChecking() {
    const id = checkId;
    checkId = "";
    saving = false;
    slow = false;
    error = "";
    await api.cancelAddRepo(id).catch(() => {});
  }

  function close() {
    if (created) return oncreated(created); // el repositorio ya está añadido
    if (saving) stopChecking();
    onclose();
  }

  function back() {
    if (saving) stopChecking();
    error = "";
    step = "type";
  }

  /** Vigilar desde la web: el agente revisa sus snapshots cada hora y los informa. */
  async function watch() {
    if (!created) return;
    watching = true;
    watchError = "";
    try {
      // La contraseña se acaba de escribir en este diálogo: no se vuelve a pedir.
      await api.agentSetSchedule(created.id, { kind: "monitor", every: Math.max(1, Math.floor(watchEvery) || 24) }, password);
      await refreshAgent();
      oncreated(created);
    } catch (e) {
      watchError = String(e);
    } finally {
      watching = false;
    }
  }
</script>

<Modal onclose={close} labelledby="add-title" width={step === "type" && !created ? 600 : 560} dismissible={step === "type" && !created}>
  {#if created}
  <div class="watch">
    <span class="watch-ic"><Globe size={22} /></span>
    <h2 id="add-title">¿Vigilar «{created.name}» desde la web?</h2>
    <p class="muted">
      Este equipo está vinculado a Resguardo Web. Si lo vigilas, el agente revisará sus versiones cada hora y enviará su estado a
      la web (fechas, tamaños y duración; nunca contraseñas ni archivos). Útil si las copias de este repositorio las hace otro programa.
    </p>
    <label class="row-field">
      Se esperan copias cada
      <input class="input num" type="number" min="1" max="744" bind:value={watchEvery} />
      horas
    </label>
    {#if !watchElevated}
      <div class="notice notice-info">
        <Info size={16} />
        <p>
          Hace falta abrir Resguardo como administrador. Después, en el repositorio, usa <strong>Copias automáticas →
          Programar → Solo vigilar</strong>.
        </p>
      </div>
    {/if}
    {#if watchError}<div class="notice notice-danger" role="alert"><CircleAlert size={16} /><p>{watchError}</p></div>{/if}
    <footer>
      <button class="btn btn-ghost" onclick={() => oncreated(created!)} disabled={watching}>Ahora no</button>
      {#if watchElevated}
        <button class="btn btn-primary" onclick={watch} disabled={watching}>
          {#if watching}<span class="spin" style="display:grid"><LoaderCircle size={15} /></span>{:else}<Globe size={15} />{/if}
          Vigilar en la web
        </button>
      {:else}
        <button class="btn btn-primary" onclick={() => api.relaunchAsAdmin().catch((e) => (watchError = String(e)))}>
          <ShieldAlert size={15} /> Abrir como administrador
        </button>
      {/if}
    </footer>
  </div>
  {:else if step === "type"}
  <header class="dlg-head">
    <div>
      <h2 id="add-title">Añadir repositorio</h2>
      <p class="faint lead">¿Dónde quieres guardar las copias?</p>
    </div>
    <button class="icon-btn" title="Cerrar" aria-label="Cerrar" onclick={close}><X size={17} /></button>
  </header>
  {#if wizardSteps}<div class="wiz-steps"><Stepper labels={wizardSteps.labels} current={wizardSteps.current} /></div>{/if}
  <div class="types">
    {#each KINDS as k}
      <button type="button" class="type" onclick={() => choose(k.id)}>
        <span class="type-icon"><k.icon size={20} /></span>
        <span class="type-text">
          <strong>{k.label}</strong>
          <span class="faint">{k.sub}</span>
        </span>
        <span class="type-go"><ChevronRight size={16} /></span>
      </button>
    {/each}
    {#if onshared}
      <button type="button" class="type shared" onclick={onshared}>
        <span class="type-icon"><Share2 size={20} /></span>
        <span class="type-text">
          <strong>Un destino compartido por otro equipo tuyo</strong>
          <span class="faint">La cuenta de la nube o el servidor que ya usa otro de tus equipos, sin escribir sus claves</span>
        </span>
        <span class="type-go"><ChevronRight size={16} /></span>
      </button>
    {/if}
  </div>
  <p class="faint foot-note">
    Elige dónde (el destino: un disco, un servidor o la nube) y crea ahí un repositorio, una caja cifrada con su propia contraseña, o conecta uno que ya tengas. <InfoTip id="repositorio" />
  </p>
  {:else}
  <header class="dlg-head">
    <div class="dlg-title">
      <button class="icon-btn" title="Elegir otro tipo" aria-label="Volver a elegir el tipo de repositorio" onclick={back}><ArrowLeft size={17} /></button>
      <span class="ticon"><kindInfo.icon size={19} /></span>
      <div>
        <h2 id="add-title">Añadir repositorio</h2>
        <p class="faint lead">{kindInfo.label}</p>
      </div>
    </div>
    <button class="icon-btn" title="Cerrar" aria-label="Cerrar" onclick={close}><X size={17} /></button>
  </header>
  {#if wizardSteps}<div class="wiz-steps"><Stepper labels={wizardSteps.labels} current={wizardSteps.current} /></div>{/if}

  <div class="segmented mode" role="group" aria-label="Crear o conectar">
    <button type="button" aria-pressed={mode === "new"} class:on={mode === "new"} onclick={() => (mode = "new")}>
      Crear uno nuevo
    </button>
    <button type="button" aria-pressed={mode === "existing"} class:on={mode === "existing"} onclick={() => (mode = "existing")}>
      Conectar uno que ya tengo
    </button>
  </div>

  <form onsubmit={submit}>
    <label class="field">
      <span class="field-label">Nombre</span>
      <input
        class="input"
        bind:this={nameInput}
        bind:value={name}
        placeholder={kind === "rest" ? "Servidor de casa" : kind === "cloud" ? prov.label : kind === "sftp" ? "NAS por SFTP" : "Disco externo"}
        required
      />
      <span class="field-hint">Solo para reconocerlo en Resguardo.</span>
    </label>

    {#if kind === "local"}
      <div class="field">
        <label class="field-label" for="loc">Carpeta del repositorio</label>
        <div class="with-button">
          <input id="loc" class="input mono" bind:value={path} placeholder="D:\Copias\restic" spellcheck="false" required />
          <button type="button" class="btn" onclick={browseFolder}><FolderOpen size={15} /> Examinar</button>
        </div>
        {#if mode === "new"}<span class="field-hint">Mejor una carpeta vacía en otro disco: si el disco del equipo falla, las copias siguen a salvo.</span>{/if}
      </div>
    {:else if kind === "rest"}
      <div class="field">
        <label class="field-label" for="url">URL del servidor</label>
        <input
          id="url"
          class="input mono"
          bind:value={restUrl}
          onblur={extractCredentials}
          placeholder="https://servidor:8000/mi-equipo/"
          spellcheck="false"
          required
        />
        <span class="field-hint">
          Con <code>--private-repos</code> la ruta empieza por tu usuario: <code>http://servidor:8000/usuario/</code>
        </span>
      </div>
      <div class="grid2">
        <label class="field">
          <span class="field-label">Usuario del servidor <span class="faint">(opcional)</span></span>
          <span class="with-icon">
            <User size={15} />
            <input class="input" bind:value={restUser} autocomplete="off" spellcheck="false" />
          </span>
        </label>
        <label class="field">
          <span class="field-label">Contraseña del servidor</span>
          <span class="with-icon">
            <KeyRound size={15} />
            <input class="input" type={showPasswords ? "text" : "password"} bind:value={restPassword} autocomplete="off" disabled={!restUser} />
          </span>
        </label>
      </div>
      {#if restMissingPath}
        <div class="notice notice-info" transition:slide={{ duration: dur(150) }}>
          <Info size={16} />
          <p>La URL no tiene ruta. Si el servidor usa <code>--private-repos</code>, añade tu usuario al final: <code>…/{restUser}/</code></p>
        </div>
      {/if}
      {#if insecureHttp}
        <div class="notice notice-warn" transition:slide={{ duration: dur(150) }}>
          <ShieldAlert size={16} />
          <p>La conexión usa <strong>HTTP sin cifrar</strong>: el usuario y la contraseña del servidor viajan en claro por la red. Las copias en sí siguen cifradas por restic. Si puedes, usa HTTPS.</p>
        </div>
      {/if}
      <Advanced id="destino-rest" hint={cacert.trim() ? `Certificado de CA: ${cacert.trim()}` : "Certificado de CA propio (solo con HTTPS autofirmado)"} custom={!!cacert.trim()}>
        <div class="field">
          <label class="field-label" for="ca">Certificado de CA propio <span class="faint">(HTTPS con certificado autofirmado)</span></label>
          <div class="with-button">
            <span class="with-icon grow">
              <FileKey size={15} />
              <input id="ca" class="input mono" bind:value={cacert} placeholder="C:\certs\mi-ca.pem" spellcheck="false" />
            </span>
            <button type="button" class="btn" onclick={browseCert}><FolderOpen size={15} /> Examinar</button>
          </div>
        </div>
      </Advanced>
    {:else if kind === "cloud"}
      <div class="field">
        <span class="field-label" id="prov-label">Proveedor</span>
        <div class="chips" role="group" aria-labelledby="prov-label">
          {#each PROVIDERS as p}
            <button type="button" class="chip" class:on={provider === p.id} aria-pressed={provider === p.id} onclick={() => pickProvider(p.id)}>{p.label}</button>
          {/each}
        </div>
      </div>
      <div class="grid2">
        {#if provider === "r2"}
          <label class="field"><span class="field-label">ID de cuenta de Cloudflare</span><input class="input mono" bind:value={account} placeholder="0123abcd…" spellcheck="false" /></label>
        {:else if provider === "s3"}
          <label class="field"><span class="field-label">Servidor (endpoint)</span><input class="input mono" bind:value={account} placeholder="s3.ejemplo.com" spellcheck="false" /></label>
          <label class="field"><span class="field-label">Región <span class="faint">(si la pide)</span></span><input class="input mono" bind:value={region} placeholder="opcional" spellcheck="false" /></label>
        {:else}
          <label class="field"><span class="field-label">Región</span><input class="input mono" bind:value={region} placeholder={prov.region} spellcheck="false" /></label>
        {/if}
        <label class="field"><span class="field-label">Bucket</span><input class="input mono" bind:value={bucket} placeholder="mis-copias" spellcheck="false" /></label>
        <label class="field">
          <span class="field-label">Carpeta dentro del bucket</span>
          <input class="input mono" bind:value={folder} placeholder={slug(name) || "equipo"} spellcheck="false" />
        </label>
      </div>
      {#if cloudLocation}<p class="faint dest mono" title={cloudLocation}>{cloudLocation}</p>{/if}
    {:else if kind === "sftp"}
      <div class="field">
        <label class="field-label" for="sftp">Repositorio SFTP</label>
        <input id="sftp" class="input mono" bind:value={path} placeholder="usuario@servidor:/ruta/al/repo" spellcheck="false" required />
        <span class="field-hint">Usa autenticación con clave SSH (agente o <code>~/.ssh/config</code>): restic no puede pedir contraseñas de SSH desde aquí.</span>
      </div>
    {:else}
      <div class="field">
        <label class="field-label" for="other">Ubicación de restic</label>
        <input id="other" class="input mono" bind:value={path} placeholder="rclone:remoto:ruta  ·  b2:bucket:carpeta  ·  azure:contenedor:/carpeta" spellcheck="false" required />
        <span class="field-hint">Tal como la escribirías en <code>restic -r</code>.</span>
      </div>
    {/if}

    {#if usesKeys}
      <fieldset class="keys">
        <legend class="field-label">Claves de acceso {#if otherNeedsKeys}<span class="faint">(si hacen falta)</span>{/if}</legend>
        {#if keySources.length}
          <div class="segmented" role="group" aria-label="Origen de las claves">
            <button type="button" aria-pressed={!keysFrom} class:on={!keysFrom} onclick={() => (keysFrom = "")}>Claves nuevas</button>
            <button type="button" aria-pressed={!!keysFrom} class:on={!!keysFrom} onclick={() => (keysFrom = keySources[0].id)}>
              Las de otro repositorio
            </button>
          </div>
        {/if}
        {#if keysFrom}
          <label class="field">
            <span class="sr-only">Repositorio cuyas claves se usan</span>
            <select class="input" bind:value={keysFrom}>
              {#each keySources as r (r.id)}<option value={r.id}>Usar las claves de «{r.name}»</option>{/each}
            </select>
            <span class="field-hint">Se copian las claves guardadas de ese repositorio (también su región). No hace falta volver a escribirlas.</span>
          </label>
        {:else}
          <div class="grid2">
            <label class="field">
              <span class="field-label sub">{keyLabels.id}</span>
              <input class="input mono" bind:value={keyId} autocomplete="off" spellcheck="false" />
            </label>
            <label class="field">
              <span class="field-label sub">{keyLabels.secret}</span>
              <input class="input mono" type={showPasswords ? "text" : "password"} bind:value={keySecret} autocomplete="off" spellcheck="false" />
            </label>
          </div>
          {#if kind === "cloud"}
            <span class="field-hint">Mejor una clave solo para este bucket, que no pueda borrar versiones antiguas. Se guarda en el almacén de credenciales del sistema.</span>
          {/if}
        {/if}
      </fieldset>
    {/if}

    <div class="divider"></div>

    <div class="field">
      <div class="label-row">
        <label class="field-label" for="password">{mode === "new" ? "Contraseña para cifrar las copias" : "Contraseña del repositorio"}</label>
        <HelpLink topic="destinos-contrasena" label="la contraseña del repositorio" />
      </div>
      <span class="with-icon">
        <KeyRound size={15} />
        <input id="password" class="input" type={showPasswords ? "text" : "password"} bind:value={password} autocomplete="off" required />
        <button type="button" class="icon-btn eye" onclick={() => (showPasswords = !showPasswords)} title={showPasswords ? "Ocultar" : "Mostrar"} aria-label={showPasswords ? "Ocultar contraseñas" : "Mostrar contraseñas"}>
          {#if showPasswords}<EyeOff size={15} />{:else}<Eye size={15} />{/if}
        </button>
      </span>
      {#if mode === "new"}
        <input
          class="input"
          class:bad={mismatch}
          type={showPasswords ? "text" : "password"}
          bind:value={confirm}
          placeholder="Repite la contraseña"
          aria-label="Repite la contraseña"
          autocomplete="off"
          required
        />
      {/if}
      <span class="field-hint">
        {kind === "rest" ? "Es la que cifra las copias, distinta de la del servidor. " : kind === "cloud" ? "Es la que cifra las copias, distinta de las claves de la nube. " : ""}Todas las contraseñas se guardan en el almacén de credenciales del sistema.
      </span>
    </div>

    {#if mode === "new"}
      <div class="notice notice-warn">
        <TriangleAlert size={16} />
        <p>Guarda esta contraseña en un lugar seguro. Si la pierdes, <strong>no hay forma de recuperar las copias</strong>.</p>
      </div>
    {/if}

    {#if slow}
      <div class="notice notice-info" transition:slide={{ duration: dur(150) }}>
        <LoaderCircle size={16} class="spin" />
        <p>Está tardando más de lo normal. Si la dirección o la ruta no son correctas, restic reintenta durante un rato; puedes detenerlo y revisarlas.</p>
      </div>
    {/if}

    {#if error}
      <div class="notice notice-danger" role="alert"><CircleAlert size={16} /><p class="pre">{error}</p></div>
    {/if}

    <footer>
      {#if saving}
        <button type="button" class="btn btn-ghost" onclick={stopChecking}>Detener</button>
      {:else}
        <button type="button" class="btn btn-ghost" onclick={close}>Cancelar</button>
      {/if}
      <button class="btn btn-primary" type="submit" disabled={saving || mismatch}>
        {#if saving}<span class="spin" style="display:grid"><LoaderCircle size={15} /></span>
          {mode === "new" ? "Creando…" : "Conectando…"}
        {:else}
          {mode === "new" ? "Crear repositorio" : "Conectar"}
        {/if}
      </button>
    </footer>
  </form>
  {/if}
</Modal>

<style>
  .wiz-steps {
    margin: 0 0 var(--sp-5);
  }
  .lead {
    margin: 2px 0 0;
    font-size: var(--fs-sm);
  }
  .dlg-title .icon-btn {
    margin-left: -8px;
  }
  .types {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .type {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 12px 14px;
    font: inherit;
    text-align: left;
    color: var(--text-1);
    background: var(--surface);
    border: 1.5px solid var(--border);
    border-radius: var(--radius);
    cursor: pointer;
    transition:
      border-color 0.15s,
      background 0.15s;
  }
  .type:hover,
  .type:focus-visible {
    border-color: var(--accent);
    background: var(--accent-soft);
  }
  .type-icon {
    display: grid;
    place-items: center;
    flex: none;
    width: 40px;
    height: 40px;
    border-radius: var(--radius);
    color: var(--accent-text);
    background: var(--accent-soft);
  }
  .type:hover .type-icon {
    color: var(--accent-contrast);
    background: var(--accent);
  }
  .type-text {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-width: 0;
  }
  .type-text .faint {
    font-size: var(--fs-sm);
  }
  .type-go {
    display: grid;
    color: var(--text-3);
  }
  .foot-note {
    margin: 14px 0 0;
    font-size: var(--fs-sm);
    line-height: 1.5;
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .chip {
    padding: 0 12px;
    font: inherit;
    font-size: var(--fs-sm);
    font-weight: 550;
    line-height: 30px;
    color: var(--text-2);
    background: var(--surface);
    border: 1.5px solid var(--border);
    border-radius: 999px;
    cursor: pointer;
  }
  .chip:hover {
    border-color: var(--border-strong);
  }
  .chip.on {
    color: var(--accent-text);
    border-color: var(--accent);
    background: var(--accent-soft);
  }
  .dest {
    margin: -6px 0 0;
    font-size: var(--fs-xs);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .keys {
    display: flex;
    flex-direction: column;
    gap: 10px;
    margin: 0;
    padding: 0;
    border: none;
  }
  .keys legend {
    padding: 0;
    margin-bottom: 8px;
  }
  .field-label.sub {
    font-weight: 500;
    color: var(--text-2);
  }
  .segmented.mode {
    margin-bottom: 18px;
  }
  form {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }
  .grid2 {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 12px;
  }
  .with-button {
    display: flex;
    gap: 8px;
  }
  .with-button .btn {
    height: 36px;
  }
  .grow {
    flex: 1;
  }
  .with-icon {
    position: relative;
    display: flex;
    align-items: center;
  }
  .with-icon > :global(svg) {
    position: absolute;
    left: 11px;
    color: var(--text-3);
    pointer-events: none;
  }
  .with-icon .input {
    padding-left: 33px;
  }
  .with-icon .eye {
    position: absolute;
    right: 4px;
  }
  .with-icon:has(.eye) .input {
    padding-right: 38px;
  }
  .input:disabled {
    opacity: 0.5;
  }
  .watch {
    display: flex;
    flex-direction: column;
    gap: 14px;
  }
  .watch-ic {
    display: grid;
    place-items: center;
    width: 44px;
    height: 44px;
    border-radius: var(--radius-lg);
    color: var(--accent-text);
    background: var(--accent-soft);
  }
  .watch p {
    margin: 0;
    font-size: var(--fs-sm);
    line-height: 1.55;
  }
  .row-field {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: var(--fs-sm);
  }
  .num {
    width: 80px;
    height: 34px;
  }
  .divider {
    height: 1px;
    background: var(--border);
    margin: 2px 0;
  }
  .bad {
    border-color: var(--bad);
  }
  .pre {
    white-space: pre-line;
  }
  footer {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    padding-top: 4px;
  }
</style>
