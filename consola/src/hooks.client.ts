// Errores inesperados de la consola (G1, plan 0.8.1). Sin esto, SvelteKit
// enseña «Internal Error» en inglés en `+error.svelte`. El detalle va a la
// consola del navegador (para quien lo depure); a la persona, una frase.
import type { HandleClientError } from "@sveltejs/kit";

export const handleError: HandleClientError = ({ error, status }) => {
  if (status !== 404) console.error(error);
  return {
    message: status === 404 ? "Esta página no existe." : "Algo ha fallado al abrir esta página. Vuelve a intentarlo en un momento.",
  };
};
