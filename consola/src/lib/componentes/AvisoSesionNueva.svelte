<script lang="ts">
  // 16k: en las demás sesiones, aviso de que se abrió una nueva en las últimas 24 h
  // («Nueva sesión en Chrome · Android, hace 5 min. ¿Eras tú?»). Se puede descartar
  // (se recuerda en este navegador) o ir a «Mi cuenta» a cerrarla.
  import { ShieldAlert, X } from "@lucide/svelte";
  import { app } from "$lib/estado.svelte";
  import { nuevasSinVer, textoSesionNueva } from "$lib/sesionesCuenta";

  const CLAVE = "resguardo.sesiones-vistas";
  function leerVistas(): string[] {
    try {
      const v = JSON.parse(localStorage.getItem(CLAVE) ?? "[]");
      return Array.isArray(v) ? v.filter((x): x is string => typeof x === "string").slice(-50) : [];
    } catch {
      return [];
    }
  }
  let vistas = $state<string[]>(leerVistas());
  const nuevas = $derived(nuevasSinVer(app.cuenta?.sesiones_nuevas, vistas));
  function descartar(id: string) {
    vistas = [...vistas, id].slice(-50);
    try {
      localStorage.setItem(CLAVE, JSON.stringify(vistas));
    } catch {
      // Sin almacenamiento: se olvida al recargar.
    }
  }
</script>

{#each nuevas as n (n.id)}
  <div class="notice notice-warn aviso" role="status" data-testid="aviso-sesion-nueva">
    <ShieldAlert size={16} aria-hidden="true" />
    <p>{textoSesionNueva(n)}</p>
    <a class="btn btn-sm" href="/ajustes#t-sesiones">Ver mis sesiones</a>
    <button class="btn btn-ghost btn-sm" aria-label="Era yo: no avisar más de esta sesión" onclick={() => descartar(n.id)}><X size={14} />Era yo</button>
  </div>
{/each}

<style>
  .aviso {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
    margin-bottom: var(--sp-3);
  }
  .aviso p {
    flex: 1;
    min-width: 12rem;
    margin: 0;
  }
</style>
