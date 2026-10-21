// Plan 0.8.1, B1/C4: dirección de antes del menú nuevo; desde la 0.8.2 (A1) lleva a
// «Ajustes → Infraestructura».
import { redirigirA } from "$lib/redirigir";

export const load = redirigirA("/ajustes/infraestructura");
