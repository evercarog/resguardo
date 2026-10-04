<script lang="ts">
  // Alerta de seguridad: las llaves de un equipo que da el servidor no son las
  // que este navegador ya comprobó. No se ha enviado nada.
  import { ShieldAlert } from "@lucide/svelte";
  import type { Equipo } from "$lib/tipos";
  let { equipo, cliente }: { equipo: Equipo; cliente: string } = $props();
</script>

<div class="alerta" role="alert">
  <span class="icono"><ShieldAlert size={22} /></span>
  <div>
    <strong>Alerta de seguridad: las llaves de {equipo.nombre} han cambiado</strong>
    <p>El servidor da para este equipo unas llaves distintas de las que este navegador comprobó antes con la clave de administración. Podría ser un servidor manipulado que intenta quedarse con tus contraseñas. <strong>No se ha enviado nada.</strong></p>
    <p>Qué hacer: no escribas contraseñas para este equipo. Avisa a quien administra el servidor y revisa la actividad. Si sabes que el equipo se reinstaló, en su ficha (Detalles) puedes volver a comprobar sus llaves con la clave de administración. <a href="/ayuda#si-llaves">Más en la ayuda</a>.</p>
    <a href="/c/{cliente}/equipos/{equipo.id}?tab=detalles">Ver el equipo</a>
  </div>
</div>

<style>
  .alerta {
    display: flex;
    gap: 12px;
    padding: 14px 16px;
    color: var(--text-1);
    background: var(--bad-soft);
    border: 1px solid color-mix(in srgb, var(--bad) 35%, transparent);
    border-radius: var(--radius);
  }
  .icono {
    flex: none;
    color: var(--bad);
  }
  strong:first-child {
    display: block;
    margin-bottom: 4px;
    color: var(--bad);
  }
  p {
    margin: 0 0 8px;
    font-size: var(--fs-sm);
    line-height: 1.5;
  }
</style>
