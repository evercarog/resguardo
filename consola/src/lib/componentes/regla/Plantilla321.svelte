<script lang="ts">
  // Plantilla «3-2-1 recomendada» (tarea 8d, docs/regla-3-2-1.md): la copia al
  // almacén (zona D) → espejo a otro disco (zona E) → repositorio a partir del
  // anterior en la nube, cada uno «después de la anterior», con verificación
  // automática y prueba de restauración mensual.
  //
  // Hoy se hace lo que se puede (la copia, la verificación y la prueba, que se
  // envían con «Enviar al equipo») y se dice dónde se hace el resto: el espejo
  // del almacén (con «después de cada copia nueva») y la copia externa. Con
  // agentes que admiten las cadenas (tarea 7, parte B), los dos últimos pasos se
  // crean desde la ficha del equipo: el paso «espejo» (lo hace el almacén) y la
  // copia derivada a la nube (lo hace el equipo, con la contraseña del repositorio).
  import { Check, CircleDashed, Hourglass, LayoutTemplate, X } from "@lucide/svelte";
  import { espejoDelRepo, nombreEnAlmacen } from "$lib/espejo";
  import { destinoDe } from "$lib/repo";
  import { unidadDe, zonaDeDestino, zonasDe } from "$lib/destinos";
  import type { Equipo } from "$lib/tipos";
  import { ADMITE, admite, repoEnAlmacen } from "$lib/cadenas";

  let {
    equipo,
    repo,
    cliente,
    equipos,
    verificacion,
    prueba,
    admitePrueba,
    onquitar,
  }: { equipo: Equipo; repo: string; cliente: string; equipos: Equipo[]; verificacion: boolean; prueba: boolean; admitePrueba: boolean; onquitar: () => void } = $props();

  const r = $derived(equipo.resumen?.repositorios?.find((x) => x.id === repo) ?? null);
  const d = $derived(destinoDe(equipo.resumen?.destinos, r));
  const zona = $derived(zonaDeDestino(d, equipos));
  const almacen = $derived(zona?.almacen ?? null);
  // El espejo del almacén que incluye este repositorio, en otro disco y «después de cada copia nueva».
  const espejo = $derived(zona?.principal && r ? espejoDelRepo(almacen?.resumen?.guarda_copias?.espejo ?? null, nombreEnAlmacen(d?.donde, r)) : null);
  const otroDisco = $derived(
    (espejo?.destinos ?? []).find((x) => x.tipo === "carpeta" && unidadDe(x.carpeta) && unidadDe(x.carpeta) !== unidadDe(zona?.carpeta ?? "")) ?? null,
  );
  const otraZona = $derived(almacen ? zonasDe(almacen).find((z) => !z.principal) : null);
  // Tarea 7d.2: un paso «espejo» de este repositorio (desde su zona, a otra zona o a otro disco).
  const enAlmacen = $derived(r ? repoEnAlmacen(equipo, r, equipos) : null);
  const pasoEspejo = $derived(
    enAlmacen
      ? ((almacen?.resumen?.guarda_copias?.espejo?.destinos ?? []).find(
          (x) => x.tras_copia && (x.zona ?? "principal") === enAlmacen.zona && Array.isArray(x.repos) && x.repos.includes(enAlmacen.nombre) && (x.tipo === "zona" || x.tipo === "carpeta"),
        ) ?? null)
      : null,
  );
  const conPasoEspejo = $derived(!!almacen && admite(almacen, ADMITE.espejoZonas));
  const conDerivadas = $derived(admite(equipo, ADMITE.derivadas));
  const derivada = $derived(r?.derivadas?.[0] ?? null);
  const ficha = $derived(`/c/${cliente}/equipos/${equipo.id}`);

  type Estado = "hecho" | "al_enviar" | "pendiente" | "falta";
  interface Paso {
    titulo: string;
    estado: Estado;
    texto: string;
    enlace?: { texto: string; href: string };
    parteB?: boolean;
  }
  const pasos = $derived<Paso[]>([
    zona
      ? { titulo: "Copia al almacén", estado: "hecho", texto: `En «${almacen?.nombre}${zona.nombre ? ` · ${zona.nombre}` : ""}», de solo añadir.` }
      : { titulo: "Copia al almacén", estado: "falta", texto: "El repositorio elegido no está en un almacén: crea uno en un almacén de la oficina y elígelo arriba.", enlace: { texto: "Nuevo repositorio", href: `/c/${cliente}/repositorios` } },
    { titulo: "Verificación automática cada semana (10 %)", estado: verificacion ? "al_enviar" : "falta", texto: verificacion ? "Se envía con la copia." : "Actualiza el agente para programarla desde aquí." },
    {
      titulo: "Prueba de restauración cada mes",
      estado: prueba ? "al_enviar" : "falta",
      texto: prueba ? "Se envía con la copia." : admitePrueba ? "Actívala abajo, en «Prueba de restauración automática»." : `Actualiza el agente para programarla; mientras, «Probar la restauración» en ${equipo.nombre} cada mes.`,
    },
    pasoEspejo
      ? { titulo: "Espejo a otro disco, después de la anterior", estado: "hecho", texto: "El almacén ya lo copia a otro disco después de cada copia nueva (un paso de su cadena)." }
      : otroDisco && otroDisco.tras_copia
      ? { titulo: "Espejo a otro disco, después de la anterior", estado: "hecho", texto: `El almacén ya lo copia a ${unidadDe(otroDisco.carpeta)} después de cada copia nueva.` }
      : conPasoEspejo
      ? {
          titulo: "Espejo a otro disco, después de la anterior",
          estado: "pendiente",
          texto: `Después de enviar: en la ficha de ${equipo.nombre}, el paso «espejo» del repositorio${otraZona ? ` (por ejemplo, a ${otraZona.nombre ?? "otra zona"})` : ""}. Lo hace ${almacen?.nombre}, sin contraseñas.`,
          enlace: { texto: "Añadir el paso «espejo»", href: `${ficha}?paso_espejo=${encodeURIComponent(repo)}` },
        }
      : {
          titulo: "Espejo a otro disco, después de la anterior",
          estado: "pendiente",
          texto: almacen
            ? `Hoy: en ${almacen.nombre}, «Espejo» → un destino de carpeta en otro disco${otraZona?.carpeta ? ` (por ejemplo, ${unidadDe(otraZona.carpeta) ?? "otra zona"})` : ""} con «después de cada copia nueva».`
            : "Cuando la copia esté en un almacén: su espejo a otro disco, después de cada copia nueva.",
          enlace: almacen ? { texto: "Espejo del almacén", href: `/c/${cliente}/equipos/${almacen.id}` } : undefined,
          parteB: true,
        },
    r?.externa
      ? { titulo: "En la nube, después de la anterior", estado: "hecho", texto: `Copia externa a «${r.externa.destino}»${r.externa.bloqueo_dias ? ` con bloqueo de ${r.externa.bloqueo_dias} días` : ""}.` }
      : derivada
      ? { titulo: "En la nube, después de la anterior", estado: "hecho", texto: `Copia derivada a «${derivada.destino ?? "otro destino"}»${derivada.bloqueo_dias ? ` con bloqueo de ${derivada.bloqueo_dias} días` : ""}.` }
      : conDerivadas
      ? {
          titulo: "En la nube, después de la anterior",
          estado: "pendiente",
          texto: "Después de enviar: una copia derivada a Backblaze B2 con bloqueo de objetos (o a Dropbox, sin bloqueo), «después de cada copia», con otra contraseña.",
          enlace: { texto: "Copia derivada", href: `${ficha}?derivada=${encodeURIComponent(repo)}` },
        }
      : {
          titulo: "En la nube, después de la anterior",
          estado: "pendiente",
          texto: "Hoy: la copia externa del repositorio a Backblaze B2 con bloqueo de objetos (o a Dropbox, sin bloqueo).",
          enlace: { texto: "Copia externa", href: `/c/${cliente}/equipos/${equipo.id}?externa=${encodeURIComponent(repo)}` },
          parteB: true,
        },
  ]);
  const ICONO = { hecho: Check, al_enviar: Hourglass, pendiente: CircleDashed, falta: X };
  const TEXTO = { hecho: "Ya está", al_enviar: "Al enviar", pendiente: "Hazlo después", falta: "Falta" };
  const TONO = { hecho: "ok", al_enviar: "info", pendiente: "neutral", falta: "warn" };
</script>

<div class="notice notice-info plantilla" role="status">
  <LayoutTemplate size={16} />
  <div>
    <p><strong>Plantilla «3-2-1 recomendada».</strong> Revisa las carpetas y pulsa «Enviar al equipo». Lo demás se hace donde dice cada paso:</p>
    <ol class="pasos">
      {#each pasos as p, i (i)}
        {@const Icono = ICONO[p.estado]}
        <li>
          <span class="badge badge-sm tone-{TONO[p.estado]}"><Icono size={11} />{TEXTO[p.estado]}</span>
          <span class="t"><strong>{p.titulo}.</strong> {p.texto}{#if p.enlace}{" "}<a class="link" href={p.enlace.href}>{p.enlace.texto} →</a>{/if}{#if p.parteB}{" "}<span class="faint parte-b">Cuando lleguen las copias en cadena, este paso se creará aquí mismo.</span>{/if}</span>
        </li>
      {/each}
    </ol>
    <button type="button" class="btn btn-sm btn-ghost" onclick={onquitar}>Ocultar</button>
  </div>
</div>

<style>
  .plantilla > div {
    display: grid;
    gap: var(--sp-2);
    min-width: 0;
  }
  .plantilla p {
    margin: 0;
  }
  .pasos {
    margin: 0;
    padding: 0;
    list-style: none;
    display: grid;
    gap: 6px;
  }
  .pasos li {
    display: flex;
    gap: var(--sp-2);
    align-items: baseline;
    font-size: var(--fs-sm);
  }
  .pasos .badge {
    flex: none;
    min-width: 7.5em;
  }
  .parte-b {
    font-size: var(--fs-xs);
  }
  @media (max-width: 480px) {
    .pasos li {
      flex-direction: column;
      gap: 2px;
    }
  }
</style>
