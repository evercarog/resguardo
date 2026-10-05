// El historial que guarda el propio equipo (v1.23) para «Historial y
// versiones»: lo más reciente al abrir, lo nuevo arriba en vivo (sin perder
// lo ya cargado) y «Cargar más» por páginas (v1.26). Lo usan la página del
// repositorio, la de la copia y la del equipo. Se llama al crear el
// componente (usa `$effect`).
import { untrack } from "svelte";
import * as api from "./api";
import { avisar } from "./avisos.svelte";
import { seguirCambios, tocaEquipo } from "./vivo.svelte";
import type { EntradaHistorial } from "./tipos";

export function usarHistorialEquipo(cliente: () => string, equipo: () => string) {
  const h = $state({ entradas: [] as EntradaHistorial[], hayMas: false, cargando: false });

  $effect(() => {
    const [c, e] = [cliente(), equipo()];
    h.entradas = [];
    h.hayMas = false;
    if (!c || !e) return;
    api
      .historialEquipo(c, e)
      .then((x) => {
        if (c !== cliente() || e !== equipo()) return;
        h.entradas = x;
        h.hayMas = x.length >= api.HISTORIAL_POR_PAGINA;
      })
      .catch(() => {});
  });

  // Lo nuevo, arriba (sin perder las páginas ya cargadas con «Cargar más»).
  $effect(() => {
    const [c, e] = [cliente(), equipo()];
    if (!c || !e) return;
    return untrack(() =>
      seguirCambios(
        () =>
          api.historialEquipo(c, e).then((x) => {
            if (c !== cliente() || e !== equipo()) return;
            const ya = new Set(h.entradas.map((y) => y.id));
            const nuevas = x.filter((y) => !ya.has(y.id));
            if (nuevas.length) h.entradas = [...nuevas, ...h.entradas].sort((a, b) => Date.parse(b.hora) - Date.parse(a.hora));
          }, () => {}),
        { ms: 0, toca: (x) => x.t === "historial" && tocaEquipo(x, e) },
      ),
    );
  });

  async function cargarMas() {
    const [c, e, ultima] = [cliente(), equipo(), h.entradas.at(-1)];
    if (!ultima) return;
    h.cargando = true;
    try {
      const x = await api.historialEquipo(c, e, { antes: ultima.id });
      if (c !== cliente() || e !== equipo()) return;
      // Sin repetir (un servidor anterior a v1.26 no entiende «antes» y da otra vez lo mismo).
      const ya = new Set(h.entradas.map((y) => y.id));
      const nuevas = x.filter((y) => !ya.has(y.id));
      h.entradas = [...h.entradas, ...nuevas];
      h.hayMas = nuevas.length > 0 && x.length >= api.HISTORIAL_POR_PAGINA;
    } catch (x) {
      avisar((x as Error).message, "bad");
    } finally {
      h.cargando = false;
    }
  }

  return {
    get entradas() {
      return h.entradas;
    },
    get hayMas() {
      return h.hayMas;
    },
    get cargando() {
      return h.cargando;
    },
    cargarMas,
  };
}
