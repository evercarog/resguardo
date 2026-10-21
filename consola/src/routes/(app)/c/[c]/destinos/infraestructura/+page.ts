// Plan 0.8.2, A1: la infraestructura pasó a «Ajustes → Infraestructura» (se configura una
// vez, como las sedes; no es un destino). La dirección de la 0.8.1 lleva allí, con lo que
// llevara detrás (`?equipo=…`, `?destino=…`).
import { redirigirA } from "$lib/redirigir";

export const load = redirigirA("/ajustes/infraestructura");
