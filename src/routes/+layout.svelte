<script lang="ts">
  // Inter variable, alojada con la app (sin CDN: funciona sin conexión).
  import "@fontsource-variable/inter";
  import "../app.css";
  import { onMount } from "svelte";
  import { appLock, initLock } from "$lib/lock.svelte";
  import LockScreen from "$lib/components/LockScreen.svelte";

  let { children } = $props();

  // En la app instalada, el WebView no debe comportarse como un navegador:
  // sin menú contextual (salvo en campos de texto o texto seleccionado) y sin
  // recargar, imprimir ni buscar. En desarrollo se deja todo para depurar.
  const PROD = !import.meta.env.DEV;

  // Bloqueo con Windows Hello: se consulta antes de montar la app.
  /** Al abrir bloqueada se pide enseguida; tras un rato sin usarla, al pulsar. */
  let firstLock = $state(true);
  onMount(() => {
    void initLock();
  });
  $effect(() => {
    if (appLock.ready && !appLock.locked) firstLock = false;
  });

  function oncontextmenu(e: MouseEvent) {
    if (!PROD) return;
    const t = e.target as HTMLElement | null;
    if (t?.closest("input, textarea, [contenteditable]:not([contenteditable='false']), .selectable")) return;
    if (window.getSelection()?.toString()) return;
    e.preventDefault();
  }

  function onkeydown(e: KeyboardEvent) {
    if (!PROD) return;
    const key = e.key.toLowerCase();
    const ctrl = e.ctrlKey || e.metaKey;
    if (key === "f5" || (ctrl && (key === "r" || key === "p" || key === "f"))) e.preventDefault();
  }
</script>

<svelte:window {oncontextmenu} {onkeydown} />

{#if appLock.locked}
  <!-- Bloqueada: la app no se monta (y el backend rechaza sus comandos). -->
  <LockScreen autoStart={firstLock} />
{:else if appLock.ready}
  {@render children()}
{/if}
