<script lang="ts" module>
  /** Las ilustraciones del producto (docs/diseno.md §4, «Ilustraciones»). */
  export type NombreIlustracion =
    | "bienvenida"
    | "sin-equipos"
    | "sin-repos"
    | "todo-en-orden"
    | "sin-versiones"
    | "sin-resultados"
    | "sin-conexion"
    | "no-encontrado"
    | "sin-permiso"
    | "primera-copia"
    | "restaurado";
</script>

<script lang="ts">
  // Ilustraciones de línea, en dos tintas (neutra y acento) sacadas de los
  // tokens: se adaptan solas al claro, al oscuro y al acento elegido. Sin
  // imágenes externas. Son decorativas (aria-hidden): el texto de al lado dice
  // lo mismo. Con «reducir movimiento», quietas; si no, un vaivén muy suave.
  let { nombre, ancho = 144, etiqueta }: { nombre: NombreIlustracion; ancho?: number; etiqueta?: string } = $props();
</script>

<svg
  class="il"
  width={ancho}
  height={Math.round((ancho * 3) / 4)}
  viewBox="0 0 160 120"
  role={etiqueta ? "img" : undefined}
  aria-label={etiqueta}
  aria-hidden={etiqueta ? undefined : "true"}
  focusable="false"
>
  <ellipse class="suelo" cx="80" cy="106" rx="54" ry="5" />
  {#if nombre === "bienvenida"}
    <!-- El escudo de la marca, con su flecha de volver atrás, y unos destellos. -->
    <g class="flota">
      <path class="fa ta" d="M80 20 56 29v20c0 15.5 10 29 24 34.5C94 78 104 64.5 104 49V29L80 20Z" />
      <path class="ta" d="M90.5 47.5a11 11 0 1 1-3.3-6.7" />
      <path class="ta" d="M88.8 34.5v7.3h-7.3" />
    </g>
    <g class="brillo">
      <path class="t" d="M38 30v8M34 34h8" />
      <path class="t" d="M122 22v6M119 25h6" />
      <circle class="fl" cx="126" cy="58" r="2.5" />
      <circle class="fl" cx="36" cy="66" r="2" />
    </g>
  {:else if nombre === "sin-equipos"}
    <!-- Una pantalla vacía y un «+» discontinuo: aquí irá el primero. -->
    <rect class="f t" x="38" y="26" width="72" height="50" rx="6" />
    <rect class="fs" x="45" y="33" width="58" height="36" rx="2" />
    <path class="t" d="M66 76v12M82 76v12M58 90h32" />
    <path class="tt" d="M52 42h22M52 50h32M52 58h14" />
    <g class="flota">
      <circle class="fa ta dash" cx="114" cy="72" r="15" />
      <path class="ta" d="M114 65v14M107 72h14" />
    </g>
  {:else if nombre === "sin-repos"}
    <!-- Un repositorio (cilindro) aún sin datos y un «+». -->
    <path class="f t" d="M48 34v44c0 5 11 9 26 9s26-4 26-9V34" />
    <ellipse class="fs t" cx="74" cy="34" rx="26" ry="9" />
    <path class="tt dash" d="M48 56c0 5 11 9 26 9s26-4 26-9" />
    <g class="flota">
      <circle class="fa ta dash" cx="112" cy="74" r="15" />
      <path class="ta" d="M112 67v14M105 74h14" />
    </g>
  {:else if nombre === "todo-en-orden"}
    <!-- La campana tranquila, con su ✓: nada sin revisar. -->
    <path class="f t" d="M80 22c-13 0-22 9.5-22 23v14l-7 11h58l-7-11V45c0-13.5-9-23-22-23Z" />
    <path class="t" d="M72 76a8 8 0 0 0 16 0" />
    <path class="t" d="M80 16v6" />
    <g class="flota">
      <circle class="fa ta" cx="108" cy="36" r="13" />
      <path class="ta dibuja" d="m102 36 4.2 4.2 7.8-8.2" />
    </g>
    <g class="brillo">
      <path class="tt" d="M38 40h8M40 52h4" />
      <path class="tt" d="M122 62h8M124 72h4" />
    </g>
  {:else if nombre === "sin-versiones"}
    <!-- Un calendario con los días aún por llenar y un reloj: la primera versión llegará. -->
    <rect class="f t" x="40" y="28" width="66" height="60" rx="7" />
    <path class="t" d="M40 42h66M54 22v12M92 22v12" />
    <g class="puntos">
      <circle cx="53" cy="54" r="2.5" /><circle cx="66" cy="54" r="2.5" /><circle cx="79" cy="54" r="2.5" /><circle cx="92" cy="54" r="2.5" />
      <circle cx="53" cy="66" r="2.5" /><circle cx="66" cy="66" r="2.5" /><circle cx="79" cy="66" r="2.5" />
      <circle cx="53" cy="78" r="2.5" />
    </g>
    <g class="flota">
      <circle class="fa ta" cx="108" cy="78" r="15" />
      <path class="ta" d="M108 70v8l5 4" />
    </g>
  {:else if nombre === "sin-resultados"}
    <!-- Una lupa sobre una hoja en blanco. -->
    <rect class="f t" x="42" y="22" width="52" height="66" rx="6" />
    <path class="tt" d="M52 36h30M52 46h22M52 56h26" />
    <g class="flota">
      <circle class="fa ta" cx="96" cy="64" r="16" />
      <path class="ta" d="m108 76 12 12" data-g="4" />
      <path class="ta" d="M91 60.5a5 5 0 1 1 6.5 4.8c-1.3.5-1.5 1.2-1.5 2.4" />
      <circle class="fac" cx="96" cy="72" r="1.6" />
    </g>
  {:else if nombre === "sin-conexion"}
    <!-- El enchufe y su cable, separados. -->
    <path class="t" d="M22 76h20c6 0 10-4 10-10v-2" />
    <rect class="f t" x="44" y="46" width="22" height="18" rx="4" />
    <path class="t" d="M50 46v-9M60 46v-9" />
    <rect class="fa ta" x="94" y="42" width="26" height="26" rx="6" />
    <path class="ta" d="M102 52v6M112 52v6" />
    <path class="t" d="M120 55h20" />
    <g class="brillo">
      <path class="tt" d="M76 40l4 6M84 36v8M90 42l-4 4" />
      <path class="tt" d="M76 72l4-5M84 76v-8M90 70l-4-3" />
    </g>
  {:else if nombre === "no-encontrado"}
    <!-- Una carpeta abierta y vacía, con su «?». -->
    <path class="fs t" d="M38 36c0-3 2-5 5-5h20l6 7h38c3 0 5 2 5 5v40c0 3-2 5-5 5H43c-3 0-5-2-5-5V36Z" />
    <path class="f t" d="M38 50h74v33c0 3-2 5-5 5H43c-3 0-5-2-5-5V50Z" />
    <g class="flota">
      <circle class="fa ta" cx="112" cy="40" r="14" />
      <path class="ta" d="M107 36.5a5 5 0 1 1 6.5 4.8c-1.3.5-1.5 1.2-1.5 2.4" />
      <circle class="fac" cx="112" cy="48.5" r="1.6" />
    </g>
  {:else if nombre === "sin-permiso"}
    <!-- Un candado: esto pide otro papel. -->
    <path class="ta flota" d="M66 50V40a14 14 0 0 1 28 0v10" />
    <rect class="f t" x="54" y="50" width="52" height="40" rx="8" />
    <circle class="fa ta" cx="80" cy="66" r="5" />
    <path class="ta" d="M80 71v7" />
    <g class="brillo">
      <path class="tt" d="M36 56h8M38 68h4" />
      <path class="tt" d="M116 56h8M118 68h4" />
    </g>
  {:else if nombre === "primera-copia"}
    <!-- Las hojas entran en el escudo: ya están a salvo. -->
    <rect class="fs t" x="26" y="36" width="30" height="38" rx="4" transform="rotate(-8 41 55)" />
    <rect class="f t" x="34" y="40" width="30" height="38" rx="4" />
    <path class="tt" d="M41 52h16M41 60h12M41 68h14" />
    <path class="tt dash" d="M70 60h12" />
    <g class="flota">
      <path class="fa ta" d="M108 22 86 30v18c0 14 9 26 22 31 13-5 22-17 22-31V30l-22-8Z" />
      <path class="ta dibuja" d="m98 50 7 7 13-14" data-g="3" />
    </g>
    <g class="brillo">
      <circle class="fl" cx="136" cy="24" r="2.5" />
      <path class="t" d="M80 18v6M77 21h6" />
      <circle class="fl" cx="140" cy="62" r="2" />
    </g>
  {:else if nombre === "restaurado"}
    <!-- La carpeta vuelve (flecha de volver atrás) y queda en su sitio, con su ✓. -->
    <path class="fs t" d="M34 40c0-3 2-5 5-5h18l6 7h36c3 0 5 2 5 5v36c0 3-2 5-5 5H39c-3 0-5-2-5-5V40Z" />
    <path class="f t" d="M34 52h70v31c0 3-2 5-5 5H39c-3 0-5-2-5-5V52Z" />
    <path class="tt" d="M62 70a9 9 0 1 0 3-6.7" />
    <path class="tt" d="M62 58v6h6" />
    <g class="flota">
      <circle class="fa ta" cx="112" cy="40" r="15" />
      <path class="ta dibuja" d="m105 40 5 5 9-9.5" data-g="3" />
    </g>
    <g class="brillo">
      <path class="t" d="M134 64v6M131 67h6" />
      <circle class="fl" cx="26" cy="30" r="2" />
    </g>
  {/if}
</svg>

<style>
  .il {
    --il-linea: var(--text-2);
    --il-tenue: var(--text-3);
    --il-papel: var(--surface);
    --il-suave: var(--surface-2);
    --il-suelo: var(--surface-3);
    --il-acento: var(--accent);
    --il-acento-suave: color-mix(in srgb, var(--accent) 14%, var(--surface));
    display: block;
    flex: none;
    max-width: 100%;
    height: auto;
    overflow: visible;
  }
  /* Un solo grosor de trazo (2 en la caja de 160 × 120), puntas redondas. */
  .il :global(:is(path, rect, circle, ellipse)) {
    stroke-width: 2;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  .il :global([data-g="3"]) {
    stroke-width: 3;
  }
  .il :global([data-g="4"]) {
    stroke-width: 4;
  }
  .suelo {
    fill: var(--il-suelo);
    stroke: none;
    opacity: 0.7;
  }
  .t,
  .tt,
  .ta {
    fill: none;
  }
  .t {
    stroke: var(--il-linea);
  }
  .tt {
    stroke: var(--il-tenue);
    opacity: 0.75;
  }
  .ta {
    stroke: var(--il-acento);
  }
  .f {
    fill: var(--il-papel);
  }
  .fs {
    fill: var(--il-suave);
  }
  .fa {
    fill: var(--il-acento-suave);
  }
  .fl {
    fill: var(--il-tenue);
    stroke: none;
  }
  .fac {
    fill: var(--il-acento);
    stroke: none;
  }
  .dash {
    stroke-dasharray: 4 4;
  }
  .puntos {
    fill: var(--il-suelo);
  }

  /* Movimiento opcional: solo si el sistema no pide reducirlo. */
  @media (prefers-reduced-motion: no-preference) {
    .flota {
      animation: il-flota 5s ease-in-out infinite;
    }
    .brillo {
      animation: il-brillo 3.2s ease-in-out infinite;
    }
    .dibuja {
      stroke-dasharray: 40;
      stroke-dashoffset: 40;
      animation: il-dibuja 0.6s 0.25s var(--ease-out) forwards;
    }
  }
  @keyframes il-flota {
    50% {
      transform: translateY(-2.5px);
    }
  }
  @keyframes il-brillo {
    50% {
      opacity: 0.45;
    }
  }
  @keyframes il-dibuja {
    to {
      stroke-dashoffset: 0;
    }
  }
</style>
