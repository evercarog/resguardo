// Acceso a los archivos de un repositorio desde los paneles de detalle («Qué
// cambió», «Lo que más ocupa», las versiones de un archivo): una sesión
// cifrada `explorar` (api-servidor.md §7) abierta con la contraseña del
// repositorio. Vive mientras la página está abierta (el panel la cierra al
// salir) y la contraseña solo en esta memoria: hace falta para volver a abrir
// la sesión si caduca y para restaurar un archivo. Nada va al servidor en claro.
import { comprobarLlaves } from "./fijadas";
import { ErrorFaltaAdmin, ErrorLlavesCambiadas } from "./ordenar";
import { Sesion, type MensajeEquipo } from "./sesion";
import type { Cliente, Equipo } from "./tipos";

/** Operaciones largas: sin noticias del equipo durante este tiempo, se deja de esperar. */
export const ESPERA_LARGA = 3 * 60_000;

export class AccesoRepo {
  abierta = $state(false);
  abriendo = $state(false);
  paso = $state("");
  error = $state("");
  pideAdmin = $state(false);
  llavesCambiadas = $state(false);
  /** Lo que dice el equipo que admite (null: aún no se sabe). */
  ops = $state<string[] | null>(null);
  private sesion: Sesion | null = null;
  private contrasena = "";
  private resultados = new Map<string, Promise<unknown>>();

  constructor(
    readonly cliente: Cliente,
    readonly equipo: Equipo,
    readonly repo: string,
  ) {}

  /** ¿Hace falta también la clave de administración (primera vez en este navegador)? */
  async comprobar() {
    const x = await comprobarLlaves(this.cliente.id, this.equipo);
    this.llavesCambiadas = x === "cambiada";
    this.pideAdmin = x === "sin_fijar";
  }

  admite(op: string): boolean {
    return this.ops?.includes(op) ?? false;
  }

  /** Abre la sesión con la contraseña (y, la primera vez, la clave de administración). */
  async abrir(contrasena: string, claveAdmin?: string) {
    this.error = "";
    this.abriendo = true;
    try {
      await this.conectar(contrasena, claveAdmin);
      this.contrasena = contrasena;
      this.pideAdmin = false;
    } catch (err) {
      if (err instanceof ErrorLlavesCambiadas) this.llavesCambiadas = true;
      else if (err instanceof ErrorFaltaAdmin) this.pideAdmin = true;
      this.error = err instanceof ErrorLlavesCambiadas ? "" : (err as Error).message;
      throw err;
    } finally {
      this.abriendo = false;
      this.paso = "";
    }
  }

  private async conectar(contrasena: string, claveAdmin?: string) {
    void this.sesion?.cerrar();
    this.sesion = null;
    const s = await Sesion.abrir({
      cliente: this.cliente,
      equipo: this.equipo,
      tipo: "explorar",
      cuerpo: { repo: this.repo },
      secretos: { repo: { repo: this.repo, contrasena }, claveAdmin: claveAdmin || undefined },
      alPaso: (t) => (this.paso = t),
    });
    try {
      this.paso = "Esperando al equipo…";
      await s.lista;
    } catch (e) {
      void s.cerrar();
      throw e;
    }
    this.sesion = s;
    this.ops = s.ops ?? [];
    this.abierta = true;
  }

  /**
   * Pide algo al equipo. Si la sesión caducó (10 min sin uso), se vuelve a
   * abrir con la misma contraseña, una vez. Con `clave`, el resultado se
   * guarda (mientras dure el panel) para no repetirlo.
   */
  pedir<T extends MensajeEquipo>(op: string, args: Record<string, unknown>, clave?: string): Promise<T> {
    if (clave && this.resultados.has(clave)) return this.resultados.get(clave) as Promise<T>;
    const p = this.pedirAhora<T>(op, args);
    if (clave) {
      this.resultados.set(clave, p);
      p.catch(() => this.resultados.delete(clave));
    }
    return p;
  }

  private async pedirAhora<T extends MensajeEquipo>(op: string, args: Record<string, unknown>): Promise<T> {
    if (!this.sesion?.abierta) {
      if (!this.contrasena) throw new Error("Abre el repositorio con su contraseña.");
      this.abriendo = true;
      try {
        await this.conectar(this.contrasena);
      } finally {
        this.abriendo = false;
        this.paso = "";
      }
    }
    try {
      return await this.sesion!.pedir<T>(op, args, ESPERA_LARGA);
    } catch (e) {
      if (!this.sesion?.abierta) this.abierta = false;
      throw e;
    }
  }

  /** La contraseña, para una orden de restaurar (no se guarda en ningún otro sitio). */
  secretoRepo() {
    return this.contrasena ? { repo: this.repo, contrasena: this.contrasena } : null;
  }

  cerrar() {
    void this.sesion?.cerrar();
    this.sesion = null;
    this.contrasena = "";
    this.abierta = false;
    this.resultados.clear();
  }
}
