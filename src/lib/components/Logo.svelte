<script lang="ts">
  let { size = 32 }: { size?: number } = $props();
</script>

<!-- Escudo con una flecha circular: guardar y poder volver atrás. -->
<svg width={size} height={size} viewBox="0 0 64 64" aria-hidden="true">
  <defs>
    <linearGradient id="logo-bg" x1="0" y1="0" x2="1" y2="1">
      <stop offset="0" style="stop-color: color-mix(in srgb, var(--accent-l) 78%, white)" />
      <stop offset="1" style="stop-color: color-mix(in srgb, var(--accent-l) 82%, black)" />
    </linearGradient>
  </defs>
  <rect width="64" height="64" rx="15" fill="url(#logo-bg)" />
  <path
    d="M32 11.5 16.5 17v13.2c0 10.1 6.5 18.9 15.5 22.3 9-3.4 15.5-12.2 15.5-22.3V17L32 11.5Z"
    fill="rgb(255 255 255 / 0.14)"
    stroke="#fff"
    stroke-width="3.2"
    stroke-linejoin="round"
  />
  <path d="M38.8 27.4a7.6 7.6 0 1 1-2.3-4.6" fill="none" stroke="#fff" stroke-width="3.2" stroke-linecap="round" />
  <path d="M37.6 18.8v5h-5" fill="none" stroke="#fff" stroke-width="3.2" stroke-linecap="round" stroke-linejoin="round" />
</svg>
