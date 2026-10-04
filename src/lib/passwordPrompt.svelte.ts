// Confirmación con la contraseña del repositorio para acciones destructivas o
// que cambian la configuración. La interfaz solo la pide: quien la comprueba es
// el backend (store::verify_password), así que no se puede saltar desde aquí.

export interface PasswordRequest {
  title: string;
  message: string;
  /** Nombre del repositorio, para que quede claro qué contraseña se pide. */
  repoName: string;
  confirmLabel: string;
  danger?: boolean;
  /** Acción que recibe la contraseña. Si falla, el diálogo muestra el error y sigue abierto. */
  action: (password: string) => Promise<unknown>;
}

interface Pending extends PasswordRequest {
  resolve: (password: string | null) => void;
}

export const passwordPrompt = $state<{ current: Pending | null }>({ current: null });

/**
 * Pide la contraseña y ejecuta la acción. Resuelve con la contraseña si se
 * completó (para reutilizarla en una acción larga que empieza después) o
 * `null` si se canceló.
 */
export function withPassword(request: PasswordRequest): Promise<string | null> {
  passwordPrompt.current?.resolve(null);
  return new Promise((resolve) => {
    passwordPrompt.current = { ...request, resolve };
  });
}
