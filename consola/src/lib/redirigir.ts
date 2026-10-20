// Plan 0.8.1, B1 (15bb): las direcciones de antes del menú nuevo siguen
// funcionando y llevan a su pestaña nueva, con lo que llevaran detrás
// (`?equipo=…&repo=…`). Solo en el navegador (la consola es una SPA).
import { redirect } from "@sveltejs/kit";

/** Las direcciones viejas de una sección del cliente y su sitio nuevo (sin `/c/<id>`). */
export const DIRECCIONES_VIEJAS: Record<string, string> = {
  "/repositorios": "/destinos",
  "/restaurar": "/recuperar",
  "/buscar": "/recuperar/buscar",
  "/restauraciones": "/recuperar/restauraciones",
  "/pruebas": "/recuperar/pruebas",
  "/ordenes": "/actividad",
  "/auditoria": "/actividad/registro",
  "/versiones": "/ajustes/versiones",
  "/servidor": "/ajustes/servidor",
};

/** El `load` de una dirección vieja: redirige a la nueva del mismo cliente. */
export const redirigirA =
  (ruta: string) =>
  ({ params, url }: { params: Record<string, string>; url: URL }) => {
    redirect(308, `/c/${encodeURIComponent(params.c ?? "")}${ruta}${url.search}`);
  };
