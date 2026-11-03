// 16e: los filtros de los equipos que comparten Equipos (lista y mapa), Estado y Avisos.
// Se recuerdan por persona y cliente **en este navegador** (localStorage, con `recordar.ts`):
// el servidor no tiene preferencias por persona salvo las de las notificaciones, y unos
// filtros no justifican un contrato nuevo. Por eso las vistas guardadas tampoco pasan de un
// navegador a otro; para compartir una vista, el enlace (la dirección lleva los filtros).
import { actual, app } from "./estado.svelte";
import { guardar, leerTexto } from "./recordar";
import { conVista, deTexto, aTexto, leerVistas, sinVista, vacios, type ContextoFiltros, type FiltrosEquipos, type NombresFiltros, type VistaGuardada } from "./filtrosEquipos";
import { sedeDeEquipo, sedePorId } from "./sedes";
import { grupoSistema } from "./datosBasicos";
import { grupoTipo } from "./tipoEquipo";
import { equipoEnFiltroInfra, leerFiltroInfra, textoFiltroInfra } from "./infra";

const estado = $state<{ clave: string; f: FiltrosEquipos; vistas: VistaGuardada[] }>({ clave: "", f: vacios(), vistas: [] });

const yo = () => app.cuenta?.id ?? "";
const claveActual = () => (actual.id ? `${yo()}.${actual.id}` : "");

/** Lo recordado (y, si no hay nada, la etiqueta que se elegía antes de la 0.8.3). */
function leerGuardado(clave: string): FiltrosEquipos {
  const t = leerTexto(`filtros.${clave}`);
  if (t) return deTexto(t);
  const vieja = actual.id ? leerTexto(`etiqueta.${actual.id}`) : "";
  return vieja ? { ...vacios(), etiquetas: [vieja] } : vacios();
}

export const filtrosEquipos = {
  get valor(): FiltrosEquipos {
    const k = claveActual();
    if (estado.clave !== k) return k ? leerGuardado(k) : vacios();
    return estado.f;
  },
  get vistas(): VistaGuardada[] {
    const k = claveActual();
    if (estado.clave !== k) return k ? leerVistas(leerTexto(`vistas.${k}`)) : [];
    return estado.vistas;
  },
  poner(f: FiltrosEquipos) {
    const k = claveActual();
    if (estado.clave !== k) estado.vistas = leerVistas(leerTexto(`vistas.${k}`));
    estado.clave = k;
    estado.f = f;
    guardar(`filtros.${k}`, aTexto(f));
    // La etiqueta de antes ya está dentro de los filtros.
    if (actual.id) guardar(`etiqueta.${actual.id}`, "");
  },
  guardarVista(nombre: string) {
    const f = this.valor;
    const k = claveActual();
    estado.vistas = conVista(this.vistas, nombre, f);
    estado.f = f;
    estado.clave = k;
    guardar(`vistas.${k}`, estado.vistas.length ? JSON.stringify(estado.vistas) : "");
  },
  quitarVista(nombre: string) {
    const f = this.valor;
    const k = claveActual();
    estado.vistas = sinVista(this.vistas, nombre);
    estado.f = f;
    estado.clave = k;
    guardar(`vistas.${k}`, estado.vistas.length ? JSON.stringify(estado.vistas) : "");
  },
};

/** Lo que el filtro necesita del cliente abierto: la sede de cada equipo, su sistema, su tipo y la infraestructura. */
export function contextoFiltros(): ContextoFiltros {
  const org = actual.organizacion;
  const infra = actual.infra;
  return {
    sede: (e) => sedeDeEquipo(org, e.id).sede?.id ?? null,
    sistema: grupoSistema,
    tipo: grupoTipo,
    enInfra: (e, v) => {
      const f = leerFiltroInfra(v, infra);
      return !!f && equipoEnFiltroInfra(infra, e.id, f);
    },
  };
}

/** Los nombres de las sedes y de la infraestructura del cliente abierto, para los chips (`chipsDe`). */
export function nombresFiltros(): NombresFiltros {
  const org = actual.organizacion;
  const infra = actual.infra;
  return {
    sede: (id) => sedePorId(org, id)?.nombre ?? null,
    infra: (v) => textoFiltroInfra(leerFiltroInfra(v, infra), infra) || null,
  };
}
